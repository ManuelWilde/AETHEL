//! Pipeline — sequential composition of capabilities.
//!
//! A pipeline is an ordered list of capabilities where the output of
//! step N becomes the input of step N+1.

use crate::{
    AethelError, Capability, CapValue, CapabilityDescriptor,
    PipelineId, CapabilityId,
};
use std::sync::Arc;

/// A single step in a pipeline.
#[derive(Clone)]
pub struct PipelineStep {
    /// The capability to execute at this step.
    pub capability: Arc<dyn Capability>,
    /// Human-readable label for this step.
    pub label: String,
}

impl PipelineStep {
    /// Create a new pipeline step.
    pub fn new(capability: Arc<dyn Capability>, label: impl Into<String>) -> Self {
        Self {
            capability,
            label: label.into(),
        }
    }

    /// Get the descriptor of the underlying capability.
    pub fn descriptor(&self) -> &CapabilityDescriptor {
        self.capability.descriptor()
    }
}

/// Metadata about a completed pipeline step execution.
#[derive(Clone, Debug)]
pub struct StepResult {
    /// Which step index (0-based).
    pub step_index: usize,
    /// The capability ID that ran.
    pub capability_id: CapabilityId,
    /// The label of this step.
    pub label: String,
    /// The output type name produced.
    pub output_type_name: String,
}

/// A pipeline — ordered sequence of capabilities.
pub struct Pipeline {
    /// Unique identifier.
    pub id: PipelineId,
    /// Human-readable name.
    pub name: String,
    /// The ordered steps.
    pub steps: Vec<PipelineStep>,
}

impl Pipeline {
    /// Create a new empty pipeline.
    pub fn new(id: PipelineId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            steps: Vec::new(),
        }
    }

    /// Add a step to the end of the pipeline.
    pub fn add_step(&mut self, step: PipelineStep) {
        self.steps.push(step);
    }

    /// Number of steps.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Is the pipeline empty?
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Validate the pipeline: check that each step's output type
    /// matches the next step's expected input type.
    pub fn validate(&self) -> Result<(), AethelError> {
        if self.steps.is_empty() {
            return Ok(());
        }
        for i in 0..self.steps.len() - 1 {
            let current = self.steps[i].descriptor();
            let next = self.steps[i + 1].descriptor();
            if next.input_type_name != "Any" && current.output_type_name != next.input_type_name {
                return Err(AethelError::TypeMismatch {
                    expected: next.input_type_name.clone(),
                    got: current.output_type_name.clone(),
                });
            }
        }
        Ok(())
    }

    /// Execute the pipeline with an initial input value.
    pub async fn execute(
        &self,
        input: CapValue,
    ) -> Result<(CapValue, Vec<StepResult>), AethelError> {
        let mut current_value = input;
        let mut trace = Vec::with_capacity(self.steps.len());

        for (i, step) in self.steps.iter().enumerate() {
            if !step.capability.accepts(&current_value) {
                return Err(AethelError::PipelineStepFailed {
                    step_index: i,
                    reason: format!(
                        "Step '{}' does not accept input type '{}'",
                        step.label,
                        current_value.type_name()
                    ),
                });
            }

            current_value = step.capability.execute(current_value).await.map_err(|e| {
                AethelError::PipelineStepFailed {
                    step_index: i,
                    reason: format!("Step '{}' failed: {}", step.label, e),
                }
            })?;

            trace.push(StepResult {
                step_index: i,
                capability_id: step.descriptor().id.clone(),
                label: step.label.clone(),
                output_type_name: current_value.type_name().to_string(),
            });
        }

        Ok((current_value, trace))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CapabilityCategory, RiskLevel};

    struct UpperCaseCap {
        desc: CapabilityDescriptor,
    }

    impl UpperCaseCap {
        fn new() -> Self {
            Self {
                desc: CapabilityDescriptor {
                    id: CapabilityId::new("upper"),
                    name: "UpperCase".into(),
                    category: CapabilityCategory::Processing,
                    input_type_name: "Text".into(),
                    output_type_name: "Text".into(),
                    estimated_cost_cents: 0.0,
                    estimated_latency_ms: 0,
                    risk_level: RiskLevel::Low,
                },
            }
        }
    }

    #[async_trait::async_trait]
    impl Capability for UpperCaseCap {
        fn descriptor(&self) -> &CapabilityDescriptor { &self.desc }
        fn accepts(&self, input: &CapValue) -> bool { matches!(input, CapValue::Text(_)) }
        async fn execute(&self, input: CapValue) -> Result<CapValue, AethelError> {
            match input {
                CapValue::Text(t) => Ok(CapValue::Text(t.to_uppercase())),
                _ => Err(AethelError::TypeMismatch {
                    expected: "Text".into(),
                    got: input.type_name().into(),
                }),
            }
        }
    }

    struct PrefixCap {
        desc: CapabilityDescriptor,
        prefix: String,
    }

    impl PrefixCap {
        fn new(prefix: &str) -> Self {
            Self {
                desc: CapabilityDescriptor {
                    id: CapabilityId::new("prefix"),
                    name: "Prefix".into(),
                    category: CapabilityCategory::Processing,
                    input_type_name: "Text".into(),
                    output_type_name: "Text".into(),
                    estimated_cost_cents: 0.0,
                    estimated_latency_ms: 0,
                    risk_level: RiskLevel::Low,
                },
                prefix: prefix.into(),
            }
        }
    }

    #[async_trait::async_trait]
    impl Capability for PrefixCap {
        fn descriptor(&self) -> &CapabilityDescriptor { &self.desc }
        fn accepts(&self, input: &CapValue) -> bool { matches!(input, CapValue::Text(_)) }
        async fn execute(&self, input: CapValue) -> Result<CapValue, AethelError> {
            match input {
                CapValue::Text(t) => Ok(CapValue::Text(format!("{}{}", self.prefix, t))),
                _ => Err(AethelError::TypeMismatch {
                    expected: "Text".into(),
                    got: input.type_name().into(),
                }),
            }
        }
    }

    struct FailCap {
        desc: CapabilityDescriptor,
    }

    impl FailCap {
        fn new() -> Self {
            Self {
                desc: CapabilityDescriptor {
                    id: CapabilityId::new("fail"),
                    name: "AlwaysFails".into(),
                    category: CapabilityCategory::Processing,
                    input_type_name: "Text".into(),
                    output_type_name: "Text".into(),
                    estimated_cost_cents: 0.0,
                    estimated_latency_ms: 0,
                    risk_level: RiskLevel::Low,
                },
            }
        }
    }

    #[async_trait::async_trait]
    impl Capability for FailCap {
        fn descriptor(&self) -> &CapabilityDescriptor { &self.desc }
        fn accepts(&self, input: &CapValue) -> bool { matches!(input, CapValue::Text(_)) }
        async fn execute(&self, _input: CapValue) -> Result<CapValue, AethelError> {
            Err(AethelError::Other("intentional failure".into()))
        }
    }

    #[test]
    fn test_empty_pipeline_is_valid() {
        let p = Pipeline::new(PipelineId::new("empty"), "Empty");
        assert!(p.validate().is_ok());
        assert!(p.is_empty());
    }

    #[test]
    fn test_single_step_pipeline_valid() {
        let mut p = Pipeline::new(PipelineId::new("single"), "Single");
        p.add_step(PipelineStep::new(Arc::new(UpperCaseCap::new()), "upper"));
        assert!(p.validate().is_ok());
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn test_matching_types_valid() {
        let mut p = Pipeline::new(PipelineId::new("chain"), "Chain");
        p.add_step(PipelineStep::new(Arc::new(UpperCaseCap::new()), "upper"));
        p.add_step(PipelineStep::new(Arc::new(PrefixCap::new(">> ")), "prefix"));
        assert!(p.validate().is_ok());
    }

    #[tokio::test]
    async fn test_execute_single_step() {
        let mut p = Pipeline::new(PipelineId::new("p1"), "P1");
        p.add_step(PipelineStep::new(Arc::new(UpperCaseCap::new()), "upper"));
        let (result, trace) = p.execute(CapValue::Text("hello".into())).await.unwrap();
        assert_eq!(result.as_text(), Some("HELLO"));
        assert_eq!(trace.len(), 1);
    }

    #[tokio::test]
    async fn test_execute_two_steps() {
        let mut p = Pipeline::new(PipelineId::new("p2"), "P2");
        p.add_step(PipelineStep::new(Arc::new(UpperCaseCap::new()), "upper"));
        p.add_step(PipelineStep::new(Arc::new(PrefixCap::new(">> ")), "prefix"));
        let (result, _) = p.execute(CapValue::Text("hello".into())).await.unwrap();
        assert_eq!(result.as_text(), Some(">> HELLO"));
    }

    #[tokio::test]
    async fn test_execute_empty_pipeline() {
        let p = Pipeline::new(PipelineId::new("empty"), "Empty");
        let (result, trace) = p.execute(CapValue::Text("pass-through".into())).await.unwrap();
        assert_eq!(result.as_text(), Some("pass-through"));
        assert!(trace.is_empty());
    }

    #[tokio::test]
    async fn test_execute_wrong_input_type() {
        let mut p = Pipeline::new(PipelineId::new("p3"), "P3");
        p.add_step(PipelineStep::new(Arc::new(UpperCaseCap::new()), "upper"));
        let result = p.execute(CapValue::Nothing).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_step_failure_propagates() {
        let mut p = Pipeline::new(PipelineId::new("p4"), "P4");
        p.add_step(PipelineStep::new(Arc::new(UpperCaseCap::new()), "upper"));
        p.add_step(PipelineStep::new(Arc::new(FailCap::new()), "fail"));
        let result = p.execute(CapValue::Text("hello".into())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_trace_records_all_steps() {
        let mut p = Pipeline::new(PipelineId::new("p5"), "P5");
        p.add_step(PipelineStep::new(Arc::new(PrefixCap::new("a-")), "prefix-a"));
        p.add_step(PipelineStep::new(Arc::new(PrefixCap::new("b-")), "prefix-b"));
        p.add_step(PipelineStep::new(Arc::new(PrefixCap::new("c-")), "prefix-c"));
        let (result, trace) = p.execute(CapValue::Text("x".into())).await.unwrap();
        assert_eq!(result.as_text(), Some("c-b-a-x"));
        assert_eq!(trace.len(), 3);
    }
}
