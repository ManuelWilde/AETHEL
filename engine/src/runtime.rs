//! AETHEL Runtime — top-level system orchestrator.
//!
//! Ties together the AethelSystem (from contracts), the FIMAS executor,
//! the agent runner, and the task queue into a single runtime entry point.

use crate::agent_runner::{AgentBackend, AgentRunner, EchoBackend};
use crate::fimas_executor::{FimasConfig, FimasExecutor, FimasResult};
use crate::task_queue::TaskQueue;
use aethel_contracts::{
    AethelSystem, AppDefinition, BudgetLease, CapValue, CapabilityDescriptor,
    CompressionConfig, ComplianceManifest, DecompositionPlan, LeaseId,
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
    compliance: Option<ComplianceManifest>,
    compression: Option<CompressionConfig>,
}

impl RuntimeBuilder {
    /// Start building a new runtime.
    pub fn new() -> Self {
        Self {
            backend: None,
            fimas_config: FimasConfig::default(),
            compliance: None,
            compression: None,
        }
    }

    /// Set the agent backend (LLM connector).
    pub fn with_backend(mut self, backend: Arc<dyn AgentBackend>) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Set FIMAS execution config.
    pub fn with_fimas_config(mut self, config: FimasConfig) -> Self {
        self.fimas_config = config;
        self
    }

    /// Set compliance manifest.
    pub fn with_compliance(mut self, manifest: ComplianceManifest) -> Self {
        self.compliance = Some(manifest);
        self
    }

    /// Set thought compression config.
    pub fn with_compression(mut self, config: CompressionConfig) -> Self {
        self.compression = Some(config);
        self
    }

    /// Build the runtime. Uses EchoBackend if no backend is provided.
    pub fn build(self) -> AethelRuntime {
        let backend: Arc<dyn AgentBackend> = self
            .backend
            .unwrap_or_else(|| Arc::new(EchoBackend::new()));

        let system = AethelSystem::new(
            self.compliance.unwrap_or_else(ComplianceManifest::aethel_default),
            self.compression.unwrap_or_default(),
        );

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
    /// Create a new runtime with default configuration.
    pub fn new_default() -> Self {
        RuntimeBuilder::new().build()
    }

    /// Create a builder for custom configuration.
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    /// Execute a FIMAS decomposition plan with the given budget.
    pub async fn execute_plan(
        &self,
        plan: &DecompositionPlan,
        budget: &BudgetLease,
        input: CapValue,
    ) -> FimasResult {
        // Audit the decision to execute
        let audit_msg = format!(
            "Executing plan for mission {} with {} sub-tasks",
            plan.mission_id,
            plan.sub_tasks.len()
        );
        self.system.audit_decision(&audit_msg, aethel_contracts::RiskLevel::Medium);

        // Execute via FIMAS
        let result = self.fimas.execute_plan(plan, budget, input).await;

        // Audit the result
        let result_msg = format!(
            "Plan completed: {} success, {} tokens, {} cents",
            result.success, result.total_tokens, result.total_cost_cents,
        );
        self.system.audit_decision(&result_msg, aethel_contracts::RiskLevel::Low);

        result
    }

    /// Process a bio-signal through the system's BioGate.
    pub fn process_bio_signal(&self, stress: f64, coherence: f64, focus: f64) -> bool {
        self.system.process_bio_signal(stress, coherence, focus)
    }

    /// Compress thought for a given task context.
    pub fn compress(&self, thought: &str, task_risk: aethel_contracts::RiskLevel) -> String {
        self.system.compress_for_task(thought, 0.5, task_risk)
    }

    /// Verify the integrity of the audit chain.
    pub fn verify_integrity(&self) -> bool {
        self.system.verify_audit_integrity()
    }

    /// Get a summary of the system state.
    pub fn summary(&self) -> aethel_contracts::SystemSummary {
        self.system.summary()
    }

    /// Register a capability in the system.
    pub fn register_capability(&self, descriptor: CapabilityDescriptor) {
        self.system.capabilities.register(descriptor);
    }

    /// Register an app definition in the system.
    pub fn register_app(&self, app: AppDefinition) -> Result<(), aethel_contracts::AethelError> {
        self.system.apps.register(app)
    }

    /// Create a root budget lease for a new mission.
    pub fn create_root_budget(
        &self,
        mission_id: &str,
        max_tokens: u64,
        max_cost_cents: u64,
    ) -> BudgetLease {
        BudgetLease {
            lease_id: LeaseId::new(format!("root-{}", mission_id)),
            parent_lease: None,
            max_tokens,
            max_cost_cents,
            used_tokens: 0,
            used_cost_cents: 0,
            max_depth: 10,
            current_depth: 0,
            max_duration_ms: 600_000, // 10 minutes
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aethel_contracts::{
        DecompositionStrategy, MissionId, SubTask, RiskLevel,
        CapabilityCategory, CapabilityId,
    };

    fn make_simple_plan() -> DecompositionPlan {
        DecompositionPlan {
            mission_id: MissionId::new("test"),
            strategy: DecompositionStrategy::Sequential,
            sub_tasks: vec![SubTask {
                id: "only-task".to_string(),
                description: "Do the thing".to_string(),
                capability_name: "echo".to_string(),
                depends_on: vec![],
                estimated_tokens: 100,
                estimated_cost_cents: 5,
                depth: 1,
            }],
        }
    }

    #[tokio::test]
    async fn test_runtime_default_executes() {
        let runtime = AethelRuntime::new_default();
        let budget = runtime.create_root_budget("test", 10_000, 500);
        let result = runtime
            .execute_plan(&make_simple_plan(), &budget, CapValue::Text("go".into()))
            .await;

        assert!(result.success);
        assert_eq!(result.agent_results.len(), 1);
    }

    #[tokio::test]
    async fn test_runtime_builder() {
        let runtime = AethelRuntime::builder()
            .with_fimas_config(FimasConfig::new("custom").with_concurrency(2))
            .build();

        let budget = runtime.create_root_budget("test", 50_000, 1000);
        let result = runtime
            .execute_plan(&make_simple_plan(), &budget, CapValue::Text("go".into()))
            .await;

        assert!(result.success);
    }

    #[test]
    fn test_bio_signal_routing() {
        let runtime = AethelRuntime::new_default();
        // High stress should activate bio-gate
        let activated = runtime.process_bio_signal(0.8, 0.3, 0.4);
        assert!(activated);
    }

    #[test]
    fn test_thought_compression() {
        let runtime = AethelRuntime::new_default();
        let long_thought = "This is a detailed analysis of the epistemic landscape \
            that requires careful consideration of multiple factors including \
            ontological grounding, bio-adaptive routing, and compliance requirements. \
            The system must balance all of these concerns.";
        let compressed = runtime.compress(long_thought, RiskLevel::Low);
        // Compression should produce something (may or may not be shorter depending on config)
        assert!(!compressed.is_empty());
    }

    #[test]
    fn test_audit_integrity() {
        let runtime = AethelRuntime::new_default();
        runtime.system.audit_decision("test decision", RiskLevel::Low);
        assert!(runtime.verify_integrity());
    }

    #[test]
    fn test_capability_registration() {
        let runtime = AethelRuntime::new_default();
        let desc = CapabilityDescriptor {
            id: CapabilityId::new("cap-1"),
            name: "Test Capability".to_string(),
            category: CapabilityCategory::Processing,
            input_type: "Text".to_string(),
            output_type: "Text".to_string(),
            estimated_cost_per_call: 10,
            estimated_latency_ms: 500,
            risk_level: RiskLevel::Low,
        };
        runtime.register_capability(desc);
        let summary = runtime.summary();
        assert_eq!(summary.registered_capabilities, 1);
    }

    #[test]
    fn test_root_budget_creation() {
        let runtime = AethelRuntime::new_default();
        let budget = runtime.create_root_budget("mission-42", 50_000, 2000);
        assert_eq!(budget.max_tokens, 50_000);
        assert_eq!(budget.max_cost_cents, 2000);
        assert_eq!(budget.used_tokens, 0);
        assert!(budget.parent_lease.is_none());
    }
}
