//! AETHEL Runtime — top-level system orchestrator.
//!
//! Ties together the AethelSystem (from contracts), the FIMAS executor,
//! the agent runner, and the task queue into a single runtime entry point.

use crate::agent_runner::{AgentBackend, EchoBackend};
use crate::fimas_executor::{FimasConfig, FimasExecutor, FimasResult};
use crate::task_queue::TaskQueue;
use aethel_contracts::{
    AethelSystem, BioSignal, BioGateState, BudgetLease, CapValue,
    Capability, DecompositionPlan, RiskLevel, ThoughtPressure,
    CompressionResult, EuAiActRiskLevel, SystemSummary,
};
use std::sync::Arc;

/// The AETHEL runtime — coordinates all subsystems.
pub struct AethelRuntime {
    pub system: AethelSystem,
    pub fimas: FimasExecutor,
    pub task_queue: TaskQueue,
}

/// Builder for constructing an AethelRuntime.
pub struct RuntimeBuilder {
    backend: Option<Arc<dyn AgentBackend>>,
    fimas_config: FimasConfig,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self {
            backend: None,
            fimas_config: FimasConfig::default(),
        }
    }

    pub fn with_backend(mut self, backend: Arc<dyn AgentBackend>) -> Self {
        self.backend = Some(backend);
        self
    }

    pub fn with_fimas_config(mut self, config: FimasConfig) -> Self {
        self.fimas_config = config;
        self
    }

    pub fn build(self) -> AethelRuntime {
        let backend: Arc<dyn AgentBackend> = self
            .backend
            .unwrap_or_else(|| Arc::new(EchoBackend::new()));

        let system = AethelSystem::new();
        let fimas = FimasExecutor::new(backend, self.fimas_config);
        let task_queue = TaskQueue::new();

        AethelRuntime {
            system,
            fimas,
            task_queue,
        }
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AethelRuntime {
    pub fn new_default() -> Self {
        RuntimeBuilder::new().build()
    }

    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    /// Execute a FIMAS decomposition plan with the given budget.
    pub async fn execute_plan(
        &mut self,
        plan: &DecompositionPlan,
        budget: &BudgetLease,
        input: CapValue,
    ) -> FimasResult {
        let audit_msg = format!(
            "Executing plan {} with {} sub-tasks",
            plan.plan_id,
            plan.sub_tasks.len()
        );
        self.system.audit_decision(
            "execute_plan",
            "runtime",
            &audit_msg,
            RiskLevel::Medium,
        );

        let result = self.fimas.execute_plan(plan, budget, input).await;

        let result_msg = format!(
            "Plan completed: success={}, {} tokens, {} cents",
            result.success, result.total_tokens, result.total_cost_cents,
        );
        self.system.audit_decision(
            "plan_completed",
            "runtime",
            &result_msg,
            RiskLevel::Low,
        );

        result
    }

    /// Process a bio-signal through the system's BioGate.
    pub fn process_bio_signal(&mut self, signal: &BioSignal) -> BioGateState {
        self.system.process_bio_signal(signal)
    }

    /// Compress thought for a given task context.
    pub fn compress(&self, pressure: &ThoughtPressure, task_risk: RiskLevel) -> CompressionResult {
        self.system.compress_for_task(pressure, task_risk)
    }

    /// Verify the integrity of the audit chain.
    pub fn verify_integrity(&self) -> Result<(), aethel_contracts::AethelError> {
        self.system.verify_audit_integrity()
    }

    /// Get a summary of the system state.
    pub fn summary(&self) -> SystemSummary {
        self.system.summary()
    }

    /// Register a capability in the system.
    pub fn register_capability(&mut self, capability: Arc<dyn Capability>) {
        self.system.capabilities.register(capability);
    }

    /// Register an app definition in the system.
    pub fn register_app(&mut self, app: aethel_contracts::AppDefinition) -> Result<(), aethel_contracts::AethelError> {
        self.system.apps.register(app)
    }

    /// Create a root budget lease for a new mission.
    pub fn create_root_budget(
        &self,
        mission_id: &str,
        max_tokens: u64,
        max_cost_cents: f32,
    ) -> BudgetLease {
        BudgetLease {
            lease_id: format!("root-{}", mission_id),
            mission_id: mission_id.to_string(),
            max_tokens,
            max_cost_cents,
            max_duration_ms: 600_000,
            tokens_used: 0,
            cost_used_cents: 0.0,
            granted_at_ms: 0,
            expires_at_ms: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aethel_contracts::{
        DecompositionStrategy, SubTask, CapabilityId, CapabilityDescriptor,
        CapabilityCategory, CapValue, AethelError,
    };

    // Minimal capability for testing
    struct TestCap {
        desc: CapabilityDescriptor,
    }

    #[async_trait::async_trait]
    impl Capability for TestCap {
        fn descriptor(&self) -> &CapabilityDescriptor { &self.desc }
        fn accepts(&self, _: &CapValue) -> bool { true }
        async fn execute(&self, input: CapValue) -> Result<CapValue, AethelError> { Ok(input) }
    }

    fn make_simple_plan() -> DecompositionPlan {
        DecompositionPlan {
            plan_id: "test-plan".to_string(),
            original_task: "Test task".to_string(),
            strategy: DecompositionStrategy::Sequential,
            sub_tasks: vec![SubTask {
                id: "only-task".to_string(),
                description: "Do the thing".to_string(),
                capability_id: CapabilityId::new("echo"),
                depends_on: vec![],
                max_tokens: 100,
                max_cost_cents: 5.0,
                risk_level: RiskLevel::Low,
                depth: 1,
                can_decompose_further: false,
                input_prompt: "go".to_string(),
            }],
            total_budget_tokens: 100,
            total_budget_cost_cents: 5.0,
            max_depth: 1,
        }
    }

    #[tokio::test]
    async fn test_runtime_default_executes() {
        let mut runtime = AethelRuntime::new_default();
        let budget = runtime.create_root_budget("test", 10_000, 500.0);
        let result = runtime
            .execute_plan(&make_simple_plan(), &budget, CapValue::Text("go".into()))
            .await;
        assert!(result.success);
        assert_eq!(result.agent_results.len(), 1);
    }

    #[tokio::test]
    async fn test_runtime_builder() {
        let mut runtime = AethelRuntime::builder()
            .with_fimas_config(FimasConfig::new("custom").with_concurrency(2))
            .build();
        let budget = runtime.create_root_budget("test", 50_000, 1000.0);
        let result = runtime
            .execute_plan(&make_simple_plan(), &budget, CapValue::Text("go".into()))
            .await;
        assert!(result.success);
    }

    #[test]
    fn test_bio_signal_routing() {
        let mut runtime = AethelRuntime::new_default();
        let signal = BioSignal {
            stress: 0.85,
            focus: 0.3,
            hrv_coherence: 0.2,
            measured_at_ms: 0,
        };
        let state = runtime.process_bio_signal(&signal);
        // High stress should activate
        assert!(matches!(state, BioGateState::Active | BioGateState::Reduced));
    }

    #[test]
    fn test_thought_compression() {
        let runtime = AethelRuntime::new_default();
        let pressure = ThoughtPressure {
            token_budget: 100,
            time_budget_ms: 1000,
            pressure_normalized: 0.9,
            phase_transitioned: false,
        };
        let result = runtime.compress(&pressure, RiskLevel::Low);
        // Should produce a valid compression result
        assert!(!result.emergency_blocked);
    }

    #[test]
    fn test_audit_integrity() {
        let mut runtime = AethelRuntime::new_default();
        runtime.system.audit_decision("test", "runtime", "testing", RiskLevel::Low);
        assert!(runtime.verify_integrity().is_ok());
    }

    #[test]
    fn test_capability_registration() {
        let mut runtime = AethelRuntime::new_default();
        let cap = Arc::new(TestCap {
            desc: CapabilityDescriptor {
                id: CapabilityId::new("cap-1"),
                name: "Test".to_string(),
                category: CapabilityCategory::Processing,
                input_type_name: "Text".to_string(),
                output_type_name: "Text".to_string(),
                estimated_cost_cents: 10.0,
                estimated_latency_ms: 500,
                risk_level: RiskLevel::Low,
            },
        });
        runtime.register_capability(cap);
        assert_eq!(runtime.summary().capabilities_count, 1);
    }

    #[test]
    fn test_root_budget_creation() {
        let runtime = AethelRuntime::new_default();
        let budget = runtime.create_root_budget("mission-42", 50_000, 2000.0);
        assert_eq!(budget.max_tokens, 50_000);
        assert!((budget.max_cost_cents - 2000.0).abs() < 0.01);
        assert_eq!(budget.tokens_used, 0);
    }
}
