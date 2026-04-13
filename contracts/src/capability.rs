//! The Capability system — the heart of AETHEL.
//!
//! Every function, service, agent, ML model, and UI component in AETHEL
//! implements the `Capability` trait. This enables composability, discoverability,
//! auditability, and budgeting.

use crate::{
    AethelError, CapabilityId, Claim, OmegaSpectrum24, BioSignal,
    RoutingDecision, VerificationResult, ThoughtEfficiency, RiskLevel,
};
use serde::{Deserialize, Serialize};

/// Categories of capabilities in the AETHEL system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityCategory {
    /// Input capabilities: Bio sensors, Voice VAD, Vision input
    Sensing,
    /// Data transformation: NLP, Spectrum computation, Feature extraction
    Processing,
    /// Decision-making: Claim engine, Contemplation, Verification
    Reasoning,
    /// Execution: LLM inference, Agent spawning, TTS output
    Acting,
    /// Oversight: Audit logging, Risk assessment, Gate checking
    Governing,
    /// Output: UI rendering, Export, Visualization
    Presenting,
}

/// Typed input/output values for capabilities.
///
/// Every capability takes a `CapValue` and returns a `CapValue`.
/// This enum is the lingua franca of the pipeline system.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CapValue {
    /// Plain text (prompts, responses, transcriptions)
    Text(String),
    /// A single claim with epistemic metadata
    Claim(Box<Claim>),
    /// Multiple claims (from ensemble/swarm)
    Claims(Vec<Claim>),
    /// 32-dimensional ontological vector
    Spectrum(OmegaSpectrum24),
    /// Operator biological state
    BioSignal(BioSignal),
    /// A routing decision from Triplex Via
    Routing(Box<RoutingDecision>),
    /// Result of claim verification
    Verification(Box<VerificationResult>),
    /// Efficiency metrics of a completed execution
    Efficiency(ThoughtEfficiency),
    /// Arbitrary JSON (for flexibility, use sparingly)
    Json(serde_json::Value),
    /// Raw bytes (audio, images, binary data)
    Bytes(Vec<u8>),
    /// No value (for capabilities that are side-effect only)
    Nothing,
}

impl CapValue {
    /// Returns the type name of this value. Used for pipeline type checking.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Text(_) => "Text",
            Self::Claim(_) => "Claim",
            Self::Claims(_) => "Claims",
            Self::Spectrum(_) => "Spectrum",
            Self::BioSignal(_) => "BioSignal",
            Self::Routing(_) => "Routing",
            Self::Verification(_) => "Verification",
            Self::Efficiency(_) => "Efficiency",
            Self::Json(_) => "Json",
            Self::Bytes(_) => "Bytes",
            Self::Nothing => "Nothing",
        }
    }

    /// Returns true if this value is Nothing.
    pub fn is_nothing(&self) -> bool {
        matches!(self, Self::Nothing)
    }

    /// Try to extract as Text.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(t) => Some(t),
            _ => None,
        }
    }

    /// Try to extract as Claim.
    pub fn as_claim(&self) -> Option<&Claim> {
        match self {
            Self::Claim(c) => Some(c),
            _ => None,
        }
    }
}

/// Metadata describing a capability's identity, types, and cost.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    /// Unique identifier
    pub id: CapabilityId,
    /// Human-readable name
    pub name: String,
    /// Category for filtering and grouping
    pub category: CapabilityCategory,
    /// Type name of expected input (e.g., "Text", "Bytes", "Any")
    pub input_type_name: String,
    /// Type name of produced output
    pub output_type_name: String,
    /// Estimated cost per execution in cents
    pub estimated_cost_cents: f32,
    /// Estimated latency per execution in milliseconds
    pub estimated_latency_ms: u32,
    /// Risk level for EU AI Act compliance
    pub risk_level: RiskLevel,
}

/// The central trait of the AETHEL system.
///
/// Every function, service, agent, model, and UI component implements this.
#[async_trait::async_trait]
pub trait Capability: Send + Sync {
    /// Returns the immutable descriptor of this capability.
    fn descriptor(&self) -> &CapabilityDescriptor;
    /// Returns true if this capability can process the given input.
    fn accepts(&self, input: &CapValue) -> bool;
    /// Execute this capability with the given input.
    async fn execute(&self, input: CapValue) -> Result<CapValue, AethelError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cap_value_type_names_unique() {
        let values = vec![
            CapValue::Text("hello".into()),
            CapValue::Nothing,
            CapValue::Json(serde_json::Value::Null),
            CapValue::Bytes(vec![1, 2, 3]),
        ];
        let names: Vec<_> = values.iter().map(|v| v.type_name()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names.len(), sorted.len());
    }

    #[test]
    fn test_cap_value_as_text() {
        let v = CapValue::Text("hello".into());
        assert_eq!(v.as_text(), Some("hello"));
        assert!(v.as_claim().is_none());
    }

    #[test]
    fn test_cap_value_is_nothing() {
        assert!(CapValue::Nothing.is_nothing());
        assert!(!CapValue::Text("x".into()).is_nothing());
    }

    #[test]
    fn test_cap_value_serde_text() {
        let original = CapValue::Text("hello world".into());
        let json = serde_json::to_string(&original).unwrap();
        let restored: CapValue = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.as_text(), Some("hello world"));
    }

    #[test]
    fn test_cap_value_serde_nothing() {
        let original = CapValue::Nothing;
        let json = serde_json::to_string(&original).unwrap();
        let restored: CapValue = serde_json::from_str(&json).unwrap();
        assert!(restored.is_nothing());
    }

    #[test]
    fn test_descriptor_serde_roundtrip() {
        let desc = CapabilityDescriptor {
            id: CapabilityId::new("test-cap"),
            name: "Test Capability".into(),
            category: CapabilityCategory::Processing,
            input_type_name: "Text".into(),
            output_type_name: "Claim".into(),
            estimated_cost_cents: 0.5,
            estimated_latency_ms: 100,
            risk_level: RiskLevel::Low,
        };
        let json = serde_json::to_string(&desc).unwrap();
        let restored: CapabilityDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, desc.id);
    }

    #[test]
    fn test_category_serde() {
        let cat = CapabilityCategory::Reasoning;
        let json = serde_json::to_string(&cat).unwrap();
        let restored: CapabilityCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, CapabilityCategory::Reasoning);
    }

    struct MockCap {
        desc: CapabilityDescriptor,
    }

    impl MockCap {
        fn new() -> Self {
            Self {
                desc: CapabilityDescriptor {
                    id: CapabilityId::new("mock"),
                    name: "Mock".into(),
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
    impl Capability for MockCap {
        fn descriptor(&self) -> &CapabilityDescriptor { &self.desc }
        fn accepts(&self, input: &CapValue) -> bool { matches!(input, CapValue::Text(_)) }
        async fn execute(&self, input: CapValue) -> Result<CapValue, AethelError> {
            match input {
                CapValue::Text(t) => Ok(CapValue::Text(format!("processed: {}", t))),
                _ => Err(AethelError::TypeMismatch {
                    expected: "Text".into(),
                    got: input.type_name().into(),
                }),
            }
        }
    }

    #[tokio::test]
    async fn test_mock_capability_execute() {
        let cap = MockCap::new();
        let result = cap.execute(CapValue::Text("hello".into())).await.unwrap();
        assert_eq!(result.as_text(), Some("processed: hello"));
    }

    #[tokio::test]
    async fn test_mock_capability_rejects_wrong_type() {
        let cap = MockCap::new();
        let result = cap.execute(CapValue::Nothing).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_capability_accepts() {
        let cap = MockCap::new();
        assert!(cap.accepts(&CapValue::Text("x".into())));
        assert!(!cap.accepts(&CapValue::Nothing));
    }

    #[test]
    fn test_capability_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockCap>();
    }
}
