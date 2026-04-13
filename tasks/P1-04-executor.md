# P1-04: Capability Executor with Budget Integration

## Prerequisites

P1-03 must be merged to main.

## Context

You are working on the AETHEL project — a Rust workspace.
P0-04 added: BudgetLease enforcement (consume, sub_lease).
P1-01-03 added: Capability, Pipeline, Registry.
Now we build the Executor — it wraps capability execution with budget tracking, tracing, and error handling.

## Git Branch

```bash
git checkout main && git pull
git checkout -b P1-04-executor
```

## Your Task

1. Create `contracts/src/executor.rs` with:
   - `ExecutionContext` struct (budget, trace info)
   - `CapabilityExecutor` struct
   - `execute_with_budget()` — runs a capability, deducting from BudgetLease
   - `execute_pipeline_with_budget()` — runs a pipeline with per-step budget tracking
2. Add `pub mod executor; pub use executor::*;` to `contracts/src/lib.rs`

## Exact Code

### contracts/src/executor.rs:
```rust
//! Capability Executor — runs capabilities with budget tracking and tracing.
//!
//! The executor wraps capability execution with:
//! - Budget enforcement (tokens + cost deducted per execution)
//! - Execution tracing (timing, step results)
//! - Error handling with context
//!
//! Every capability execution in the AETHEL system goes through the executor.
//! Direct capability.execute() should only be used in tests.

use crate::{
    AethelError, BudgetLease, CapValue, Capability,
    Pipeline, StepResult,
};
use std::sync::Arc;
use std::time::Instant;

/// Context for a capability execution — carries budget and trace info.
pub struct ExecutionContext {
    /// The budget lease to consume from.
    pub budget: BudgetLease,
    /// Mission ID for tracing.
    pub mission_id: String,
    /// Accumulated cost in cents across all executions in this context.
    pub total_cost_cents: f32,
    /// Accumulated tokens across all executions.
    pub total_tokens_used: u64,
    /// Number of capabilities executed.
    pub executions_count: u32,
}

impl ExecutionContext {
    /// Create a new execution context with a budget.
    pub fn new(budget: BudgetLease, mission_id: impl Into<String>) -> Self {
        Self {
            budget,
            mission_id: mission_id.into(),
            total_cost_cents: 0.0,
            total_tokens_used: 0,
            executions_count: 0,
        }
    }

    /// Check if the budget is exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.budget.is_exhausted()
    }

    /// Get budget utilization.
    pub fn utilization(&self) -> f32 {
        self.budget.utilization()
    }
}

/// Result of a single capability execution with timing.
#[derive(Clone, Debug)]
pub struct ExecutionResult {
    /// The output value.
    pub output: CapValue,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Tokens consumed (estimated from capability descriptor).
    pub tokens_consumed: u64,
    /// Cost consumed in cents (from capability descriptor).
    pub cost_consumed_cents: f32,
}

/// Result of a pipeline execution with budget tracking.
#[derive(Clone, Debug)]
pub struct PipelineExecutionResult {
    /// The final output value.
    pub output: CapValue,
    /// Per-step results.
    pub step_results: Vec<StepResult>,
    /// Total wall-clock duration in milliseconds.
    pub total_duration_ms: u64,
    /// Total tokens consumed.
    pub total_tokens: u64,
    /// Total cost in cents.
    pub total_cost_cents: f32,
}

/// The capability executor — wraps execution with budget and tracing.
pub struct CapabilityExecutor;

impl CapabilityExecutor {
    /// Execute a single capability with budget tracking.
    ///
    /// Deducts estimated cost from the budget BEFORE execution.
    /// If budget is insufficient, returns BudgetExceeded error.
    pub async fn execute_with_budget(
        ctx: &mut ExecutionContext,
        capability: &Arc<dyn Capability>,
        input: CapValue,
    ) -> Result<ExecutionResult, AethelError> {
        let desc = capability.descriptor();

        // Pre-check: estimate cost
        let est_tokens = desc.estimated_latency_ms as u64; // rough proxy
        let est_cost = desc.estimated_cost_cents;

        // Deduct from budget
        ctx.budget.consume(est_tokens, est_cost)?;

        // Execute with timing
        let start = Instant::now();
        let output = capability.execute(input).await?;
        let duration = start.elapsed();
        let duration_ms = duration.as_millis() as u64;

        // Update context
        ctx.total_cost_cents += est_cost;
        ctx.total_tokens_used += est_tokens;
        ctx.executions_count += 1;

        Ok(ExecutionResult {
            output,
            duration_ms,
            tokens_consumed: est_tokens,
            cost_consumed_cents: est_cost,
        })
    }

    /// Execute a pipeline with budget tracking.
    ///
    /// Validates the pipeline first, then executes each step with budget deduction.
    pub async fn execute_pipeline_with_budget(
        ctx: &mut ExecutionContext,
        pipeline: &Pipeline,
        input: CapValue,
    ) -> Result<PipelineExecutionResult, AethelError> {
        // Validate pipeline types
        pipeline.validate()?;

        let start = Instant::now();
        let mut current_value = input;
        let mut step_results = Vec::with_capacity(pipeline.len());
        let mut total_tokens = 0u64;
        let mut total_cost = 0.0f32;

        for (i, step) in pipeline.steps.iter().enumerate() {
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

            let result = Self::execute_with_budget(ctx, &step.capability, current_value).await
                .map_err(|e| AethelError::PipelineStepFailed {
                    step_index: i,
                    reason: format!("Step '{}': {}", step.label, e),
                })?;

            total_tokens += result.tokens_consumed;
            total_cost += result.cost_consumed_cents;

            step_results.push(StepResult {
                step_index: i,
                capability_id: step.descriptor().id.clone(),
                label: step.label.clone(),
                output_type_name: result.output.type_name().to_string(),
            });

            current_value = result.output;
        }

        let total_duration_ms = start.elapsed().as_millis() as u64;

        Ok(PipelineExecutionResult {
            output: current_value,
            step_results,
            total_duration_ms,
            total_tokens,
            total_cost_cents: total_cost,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapabilityDescriptor, CapabilityCategory, CapabilityId,
        PipelineId, PipelineStep, RiskLevel,
    };

    fn make_budget(max_tokens: u64, max_cost: f32) -> BudgetLease {
        BudgetLease {
            lease_id: "test-lease".into(),
            mission_id: "test-mission".into(),
            max_tokens,
            max_cost_cents: max_cost,
            max_duration_ms: 60_000,
            tokens_used: 0,
            cost_used_cents: 0.0,
            granted_at_ms: 0,
            expires_at_ms: 0,
        }
    }

    struct EchoCap {
        desc: CapabilityDescriptor,
    }

    impl EchoCap {
        fn new(cost: f32, latency: u32) -> Self {
            Self {
                desc: CapabilityDescriptor {
                    id: CapabilityId::new("echo"),
                    name: "Echo".into(),
                    category: CapabilityCategory::Processing,
                    input_type_name: "Text".into(),
                    output_type_name: "Text".into(),
                    estimated_cost_cents: cost,
                    estimated_latency_ms: latency,
                    risk_level: RiskLevel::Low,
                },
            }
        }
    }

    #[async_trait::async_trait]
    impl Capability for EchoCap {
        fn descriptor(&self) -> &CapabilityDescriptor { &self.desc }
        fn accepts(&self, input: &CapValue) -> bool { matches!(input, CapValue::Text(_)) }
        async fn execute(&self, input: CapValue) -> Result<CapValue, AethelError> {
            Ok(input)
        }
    }

    #[tokio::test]
    async fn test_execute_with_budget_success() {
        let budget = make_budget(10000, 100.0);
        let mut ctx = ExecutionContext::new(budget, "mission-1");
        let cap = Arc::new(EchoCap::new(1.0, 10));
        let result = CapabilityExecutor::execute_with_budget(
            &mut ctx, &cap, CapValue::Text("hello".into())
        ).await.unwrap();
        assert_eq!(result.output.as_text(), Some("hello"));
        assert_eq!(ctx.executions_count, 1);
    }

    #[tokio::test]
    async fn test_execute_with_budget_exhausted() {
        let budget = make_budget(5, 0.5); // very small budget
        let mut ctx = ExecutionContext::new(budget, "mission-1");
        let cap = Arc::new(EchoCap::new(1.0, 10)); // costs 10 tokens, 1.0 cents
        let result = CapabilityExecutor::execute_with_budget(
            &mut ctx, &cap, CapValue::Text("hello".into())
        ).await;
        assert!(result.is_err()); // budget exceeded
    }

    #[tokio::test]
    async fn test_execute_tracks_cost() {
        let budget = make_budget(10000, 100.0);
        let mut ctx = ExecutionContext::new(budget, "mission-1");
        let cap = Arc::new(EchoCap::new(2.5, 10));

        CapabilityExecutor::execute_with_budget(
            &mut ctx, &cap, CapValue::Text("a".into())
        ).await.unwrap();
        CapabilityExecutor::execute_with_budget(
            &mut ctx, &cap, CapValue::Text("b".into())
        ).await.unwrap();

        assert_eq!(ctx.executions_count, 2);
        assert!((ctx.total_cost_cents - 5.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_context_utilization() {
        let budget = make_budget(100, 10.0);
        let mut ctx = ExecutionContext::new(budget, "mission-1");
        let cap = Arc::new(EchoCap::new(5.0, 50)); // 50 tokens, 5.0 cents

        CapabilityExecutor::execute_with_budget(
            &mut ctx, &cap, CapValue::Text("x".into())
        ).await.unwrap();

        assert!(ctx.utilization() >= 0.49); // at least 50% used
    }

    #[tokio::test]
    async fn test_pipeline_execution_with_budget() {
        let budget = make_budget(10000, 100.0);
        let mut ctx = ExecutionContext::new(budget, "mission-1");

        let mut pipeline = Pipeline::new(PipelineId::new("p1"), "Test Pipeline");
        pipeline.add_step(PipelineStep::new(
            Arc::new(EchoCap::new(1.0, 10)), "step1"
        ));
        pipeline.add_step(PipelineStep::new(
            Arc::new(EchoCap::new(2.0, 20)), "step2"
        ));

        let result = CapabilityExecutor::execute_pipeline_with_budget(
            &mut ctx, &pipeline, CapValue::Text("hello".into())
        ).await.unwrap();

        assert_eq!(result.output.as_text(), Some("hello"));
        assert_eq!(result.step_results.len(), 2);
        assert!((result.total_cost_cents - 3.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_pipeline_budget_exceeded_mid_execution() {
        let budget = make_budget(15, 100.0); // only 15 tokens
        let mut ctx = ExecutionContext::new(budget, "mission-1");

        let mut pipeline = Pipeline::new(PipelineId::new("p2"), "Budget Test");
        pipeline.add_step(PipelineStep::new(
            Arc::new(EchoCap::new(0.0, 10)), "step1" // 10 tokens
        ));
        pipeline.add_step(PipelineStep::new(
            Arc::new(EchoCap::new(0.0, 10)), "step2" // 10 tokens — total 20 > 15
        ));

        let result = CapabilityExecutor::execute_pipeline_with_budget(
            &mut ctx, &pipeline, CapValue::Text("hello".into())
        ).await;
        assert!(result.is_err()); // second step should exceed budget
    }

    #[tokio::test]
    async fn test_empty_pipeline_with_budget() {
        let budget = make_budget(10000, 100.0);
        let mut ctx = ExecutionContext::new(budget, "mission-1");
        let pipeline = Pipeline::new(PipelineId::new("empty"), "Empty");

        let result = CapabilityExecutor::execute_pipeline_with_budget(
            &mut ctx, &pipeline, CapValue::Text("pass".into())
        ).await.unwrap();

        assert_eq!(result.output.as_text(), Some("pass"));
        assert_eq!(result.step_results.len(), 0);
        assert_eq!(ctx.executions_count, 0);
    }

    #[test]
    fn test_execution_context_new() {
        let budget = make_budget(1000, 10.0);
        let ctx = ExecutionContext::new(budget, "m1");
        assert_eq!(ctx.mission_id, "m1");
        assert!(!ctx.is_exhausted());
        assert!((ctx.utilization() - 0.0).abs() < 0.01);
    }
}
```

### contracts/src/lib.rs — add module declaration:
```rust
pub mod executor;
pub use executor::*;
```

## Validation

```bash
cd contracts && cargo test --workspace 2>&1
```

Expected: All tests pass (P0 + P1), zero warnings.

## Done Criteria

- [ ] `contracts/src/executor.rs` exists
- [ ] `ExecutionContext` wraps BudgetLease with tracking
- [ ] `CapabilityExecutor::execute_with_budget()` deducts from budget
- [ ] `CapabilityExecutor::execute_pipeline_with_budget()` runs pipeline with per-step budget
- [ ] Budget exhaustion mid-pipeline returns error
- [ ] 8+ tests pass
- [ ] All previous tests still pass

## Git

```bash
git add -A
git commit -m "P1-04: Capability Executor with budget integration

- ExecutionContext: wraps BudgetLease with cost/token tracking
- execute_with_budget(): single capability with budget deduction
- execute_pipeline_with_budget(): full pipeline with per-step budget
- 8+ tests including mid-pipeline budget exhaustion"
git push -u origin P1-04-executor
gh pr create --title "P1-04: Capability Executor" --body "$(cat tasks/P1-04-executor.md)"
```
