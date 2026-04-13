//! AETHEL System Facade — ties all components together.
//!
//! This is the entry point for the entire system. It holds references
//! to all subsystems and provides a unified API for:
//! - Processing tasks through the full pipeline
//! - Routing via Triplex Via
//! - Budget management
//! - Compliance and audit

use crate::{
    AethelError, CapabilityRegistry, AuditChain, ComplianceManifest,
    EuAiActRiskLevel, BioGate, BioSignal, BioGateState,
    ThoughtCompressor, CompressionLevel, CompressionResult,
    ThoughtPressure, RiskLevel, AppRegistry,
};

/// The AETHEL system — top-level orchestrator.
pub struct AethelSystem {
    /// Capability registry.
    pub capabilities: CapabilityRegistry,
    /// App registry.
    pub apps: AppRegistry,
    /// Audit chain.
    pub audit: AuditChain,
    /// Compliance manifest.
    pub manifest: ComplianceManifest,
    /// Bio-gate.
    pub bio_gate: BioGate,
    /// Thought compressor.
    pub compressor: ThoughtCompressor,
    /// Current timestamp provider (ms).
    timestamp_ms: u64,
}

impl AethelSystem {
    /// Create a new AETHEL system with default settings.
    pub fn new() -> Self {
        Self {
            capabilities: CapabilityRegistry::new(),
            apps: AppRegistry::new(),
            audit: AuditChain::new(),
            manifest: ComplianceManifest::aethel_default(),
            bio_gate: BioGate::new(),
            compressor: ThoughtCompressor::new(),
            timestamp_ms: 0,
        }
    }

    /// Set current timestamp (for testing / deterministic behavior).
    pub fn set_timestamp(&mut self, ms: u64) {
        self.timestamp_ms = ms;
    }

    /// Advance timestamp by delta.
    pub fn advance_time(&mut self, delta_ms: u64) {
        self.timestamp_ms += delta_ms;
    }

    /// Get current timestamp.
    pub fn now(&self) -> u64 {
        self.timestamp_ms
    }

    /// Check system compliance.
    pub fn is_compliant(&self) -> bool {
        self.manifest.is_compliant()
    }

    /// Process a bio signal: update the bio-gate and audit.
    pub fn process_bio_signal(&mut self, signal: &BioSignal) -> BioGateState {
        let state = self.bio_gate.update(signal);
        self.audit.append(
            "bio_signal_processed",
            "bio_gate",
            &format!("state={:?}, hrv={:.2}", state, signal.hrv_coherence),
            EuAiActRiskLevel::Minimal,
            self.timestamp_ms,
        );
        state
    }

    /// Determine compression for a task.
    pub fn compress_for_task(
        &self,
        pressure: &ThoughtPressure,
        risk: RiskLevel,
    ) -> CompressionResult {
        self.compressor.compress(pressure, risk)
    }

    /// Log an auditable decision.
    pub fn audit_decision(
        &mut self,
        action: &str,
        actor: &str,
        decision: &str,
        risk: RiskLevel,
    ) {
        let eu_risk = EuAiActRiskLevel::from(risk);
        self.audit.append(action, actor, decision, eu_risk, self.timestamp_ms);
    }

    /// Verify the entire audit chain integrity.
    pub fn verify_audit_integrity(&self) -> Result<(), AethelError> {
        self.audit.verify_integrity()
    }

    /// Get system summary.
    pub fn summary(&self) -> SystemSummary {
        SystemSummary {
            capabilities_count: self.capabilities.len(),
            apps_count: self.apps.len(),
            audit_blocks: self.audit.len(),
            is_compliant: self.is_compliant(),
            bio_gate_state: self.bio_gate.state(),
            timestamp_ms: self.timestamp_ms,
        }
    }
}

impl Default for AethelSystem {
    fn default() -> Self { Self::new() }
}

/// Summary of the system state.
#[derive(Clone, Debug)]
pub struct SystemSummary {
    /// Number of registered capabilities.
    pub capabilities_count: usize,
    /// Number of registered apps.
    pub apps_count: usize,
    /// Number of audit blocks.
    pub audit_blocks: usize,
    /// Whether the system is compliant.
    pub is_compliant: bool,
    /// Bio-gate state.
    pub bio_gate_state: BioGateState,
    /// Current timestamp.
    pub timestamp_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Capability, CapValue, CapabilityDescriptor, CapabilityCategory,
        CapabilityId,
    };
    use std::sync::Arc;

    struct DummyCap {
        desc: CapabilityDescriptor,
    }

    impl DummyCap {
        fn new(id: &str) -> Self {
            Self {
                desc: CapabilityDescriptor {
                    id: CapabilityId::new(id),
                    name: format!("Dummy {}", id),
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
    impl Capability for DummyCap {
        fn descriptor(&self) -> &CapabilityDescriptor { &self.desc }
        fn accepts(&self, _: &CapValue) -> bool { true }
        async fn execute(&self, input: CapValue) -> Result<CapValue, AethelError> { Ok(input) }
    }

    #[test]
    fn test_new_system() {
        let sys = AethelSystem::new();
        assert!(sys.is_compliant());
        assert_eq!(sys.capabilities.len(), 0);
        assert_eq!(sys.audit.len(), 0);
    }

    #[test]
    fn test_register_capability() {
        let mut sys = AethelSystem::new();
        sys.capabilities.register(Arc::new(DummyCap::new("cap1")));
        assert_eq!(sys.capabilities.len(), 1);
    }

    #[test]
    fn test_bio_signal_processing() {
        let mut sys = AethelSystem::new();
        sys.set_timestamp(1000);
        let signal = BioSignal {
            stress: 0.2,
            focus: 0.8,
            hrv_coherence: 0.75,
            measured_at_ms: 1000,
        };
        let state = sys.process_bio_signal(&signal);
        assert_eq!(state, BioGateState::Active);
        assert_eq!(sys.audit.len(), 1);
    }

    #[test]
    fn test_audit_decision() {
        let mut sys = AethelSystem::new();
        sys.set_timestamp(500);
        sys.audit_decision("create_claim", "agent-1", "approved", RiskLevel::Low);
        sys.audit_decision("verify_claim", "membrane", "passed", RiskLevel::Medium);
        assert_eq!(sys.audit.len(), 2);
        assert!(sys.verify_audit_integrity().is_ok());
    }

    #[test]
    fn test_compression_integration() {
        let sys = AethelSystem::new();
        let pressure = ThoughtPressure {
            token_budget: 500,
            time_budget_ms: 2000,
            pressure_normalized: 0.7,
            phase_transitioned: false,
        };
        let result = sys.compress_for_task(&pressure, RiskLevel::Low);
        assert_eq!(result.level, CompressionLevel::Aggressive);
    }

    #[test]
    fn test_system_summary() {
        let mut sys = AethelSystem::new();
        sys.capabilities.register(Arc::new(DummyCap::new("c1")));
        sys.capabilities.register(Arc::new(DummyCap::new("c2")));
        sys.set_timestamp(999);
        let summary = sys.summary();
        assert_eq!(summary.capabilities_count, 2);
        assert!(summary.is_compliant);
        assert_eq!(summary.timestamp_ms, 999);
    }

    #[test]
    fn test_time_advance() {
        let mut sys = AethelSystem::new();
        sys.set_timestamp(1000);
        sys.advance_time(500);
        assert_eq!(sys.now(), 1500);
    }

    #[test]
    fn test_full_system_flow() {
        let mut sys = AethelSystem::new();
        sys.set_timestamp(0);

        // Register capabilities
        sys.capabilities.register(Arc::new(DummyCap::new("sensing")));
        sys.capabilities.register(Arc::new(DummyCap::new("reasoning")));

        // Process bio signal
        sys.advance_time(100);
        let signal = BioSignal { stress: 0.1, focus: 0.9, hrv_coherence: 0.80, measured_at_ms: 100 };
        assert_eq!(sys.process_bio_signal(&signal), BioGateState::Active);

        // Audit a decision
        sys.advance_time(50);
        sys.audit_decision("route_task", "triplex_via", "selected local MLX", RiskLevel::Low);

        // Compress for task
        let pressure = ThoughtPressure {
            token_budget: 2000,
            time_budget_ms: 5000,
            pressure_normalized: 0.2,
            phase_transitioned: false,
        };
        let compression = sys.compress_for_task(&pressure, RiskLevel::Low);
        assert_eq!(compression.level, CompressionLevel::Full);

        // Verify audit integrity
        assert!(sys.verify_audit_integrity().is_ok());
        assert_eq!(sys.audit.len(), 2); // bio + route

        // Summary
        let s = sys.summary();
        assert_eq!(s.capabilities_count, 2);
        assert_eq!(s.bio_gate_state, BioGateState::Active);
        assert!(s.is_compliant);
    }
}
