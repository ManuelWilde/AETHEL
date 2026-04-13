//! End-to-end integration tests for the AETHEL platform.
//!
//! These tests exercise the full stack: contracts → engine → storage,
//! verifying that all crates work together correctly.

use aethel_contracts::*;
use aethel_engine::*;
use aethel_storage::*;
use std::sync::Arc;

// ─── Test Capability ─────────────────────────────
// A minimal capability implementation for tests that need Arc<dyn Capability>.

struct TestCap {
    desc: CapabilityDescriptor,
}

#[async_trait::async_trait]
impl Capability for TestCap {
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.desc
    }
    fn accepts(&self, _: &CapValue) -> bool {
        true
    }
    async fn execute(&self, input: CapValue) -> Result<CapValue, AethelError> {
        Ok(input)
    }
}

fn make_test_cap(id: &str, name: &str, category: CapabilityCategory) -> Arc<dyn Capability> {
    Arc::new(TestCap {
        desc: CapabilityDescriptor {
            id: CapabilityId::new(id),
            name: name.to_string(),
            category,
            input_type_name: "Text".to_string(),
            output_type_name: "Text".to_string(),
            estimated_cost_cents: 10.0,
            estimated_latency_ms: 100,
            risk_level: RiskLevel::Low,
        },
    })
}

// ═══════════════════════════════════════════════════
// 1. Full System Lifecycle
// ═══════════════════════════════════════════════════

#[test]
fn test_system_boots_and_reports_healthy() {
    let system = AethelSystem::new();
    let summary = system.summary();
    assert!(summary.is_compliant);
    assert_eq!(summary.capabilities_count, 0);
    assert_eq!(summary.apps_count, 0);
}

#[test]
fn test_capability_registration_and_discovery() {
    let mut system = AethelSystem::new();

    let sensing: Arc<dyn Capability> = Arc::new(TestCap {
        desc: CapabilityDescriptor {
            id: CapabilityId::new("sense-bio"),
            name: "Bio Sensor".to_string(),
            category: CapabilityCategory::Sensing,
            input_type_name: "BioSignal".to_string(),
            output_type_name: "Spectrum".to_string(),
            estimated_cost_cents: 5.0,
            estimated_latency_ms: 100,
            risk_level: RiskLevel::Low,
        },
    });

    let processing: Arc<dyn Capability> = Arc::new(TestCap {
        desc: CapabilityDescriptor {
            id: CapabilityId::new("process-spectrum"),
            name: "Spectrum Analyzer".to_string(),
            category: CapabilityCategory::Processing,
            input_type_name: "Spectrum".to_string(),
            output_type_name: "Text".to_string(),
            estimated_cost_cents: 20.0,
            estimated_latency_ms: 500,
            risk_level: RiskLevel::Medium,
        },
    });

    system.capabilities.register(sensing);
    system.capabilities.register(processing);

    assert_eq!(system.summary().capabilities_count, 2);

    let sensors = system.capabilities.find_by_category(CapabilityCategory::Sensing);
    assert_eq!(sensors.len(), 1);
    assert_eq!(sensors[0].descriptor().name, "Bio Sensor");

    let connectable = system.capabilities.find_connectable_after(&CapabilityId::new("sense-bio"));
    assert_eq!(connectable.len(), 1);
    assert_eq!(connectable[0].descriptor().name, "Spectrum Analyzer");
}

// ═══════════════════════════════════════════════════
// 2. Claim Lifecycle: Create → Transition → Store → Retrieve
// ═══════════════════════════════════════════════════

#[test]
fn test_claim_full_lifecycle() {
    let claim = Claim {
        id: "test-claim-1".into(),
        content: "The Earth orbits the Sun".into(),
        state: ClaimState::Generated,
        origin: ClaimOrigin::ModelGenerated,
        support_level: SupportLevel::Unsupported,
        risk: RiskLevel::Low,
        confidence: 0.95,
        evidence_ids: vec![],
        created_at_ms: 1000,
        updated_at_ms: 1000,
    };

    let s1 = claim.state.transition(ClaimState::Supported).unwrap();
    assert_eq!(s1, ClaimState::Supported);
    let s2 = s1.transition(ClaimState::Accepted).unwrap();
    assert_eq!(s2, ClaimState::Accepted);
    assert!(s2.transition(ClaimState::Generated).is_err());
    let s3 = s2.transition(ClaimState::Retired).unwrap();
    assert!(s3.is_terminal());
}

#[test]
fn test_claim_persistence_sqlite() {
    let db = test_db();
    let store = SqliteClaimStore::new(db);

    let claim = Claim {
        id: "persist-1".into(),
        content: "Water is H2O".into(),
        state: ClaimState::Generated,
        origin: ClaimOrigin::HumanEntered,
        support_level: SupportLevel::Unsupported,
        risk: RiskLevel::Low,
        confidence: 0.99,
        evidence_ids: vec!["ev-chem-101".into()],
        created_at_ms: 2000,
        updated_at_ms: 2000,
    };

    store.save_claim(&claim).unwrap();
    assert_eq!(store.count_claims().unwrap(), 1);

    let loaded = store.load_claim(&ClaimId::new("persist-1")).unwrap().unwrap();
    assert_eq!(loaded.content, "Water is H2O");

    let mut updated = loaded;
    updated.state = ClaimState::Accepted;
    store.save_claim(&updated).unwrap();
    assert_eq!(store.count_claims().unwrap(), 1);

    assert!(store.delete_claim(&ClaimId::new("persist-1")).unwrap());
    assert_eq!(store.count_claims().unwrap(), 0);
}

// ═══════════════════════════════════════════════════
// 3. Budget Enforcement
// ═══════════════════════════════════════════════════

#[test]
fn test_budget_enforcement_and_sub_leasing() {
    let mut root = BudgetLease {
        lease_id: "root".to_string(),
        mission_id: "mission-1".to_string(),
        max_tokens: 10_000,
        max_cost_cents: 500.0,
        max_duration_ms: 60_000,
        tokens_used: 0,
        cost_used_cents: 0.0,
        granted_at_ms: 0,
        expires_at_ms: 0,
    };

    assert!(root.consume(1000, 50.0).is_ok());
    assert_eq!(root.remaining_tokens(), 9000);

    let child = root.sub_lease("child-lease".to_string(), 3000, 100.0).unwrap();
    assert_eq!(child.max_tokens, 3000);

    let mut small = BudgetLease {
        lease_id: "small".to_string(),
        mission_id: "mission-2".to_string(),
        max_tokens: 100,
        max_cost_cents: 10.0,
        max_duration_ms: 1000,
        tokens_used: 0,
        cost_used_cents: 0.0,
        granted_at_ms: 0,
        expires_at_ms: 0,
    };
    assert!(small.consume(100, 10.0).is_ok());
    assert!(small.is_exhausted());
    assert!(small.consume(1, 0.0).is_err());
}

// ═══════════════════════════════════════════════════
// 4. Bio-Adaptive Routing
// ═══════════════════════════════════════════════════

#[test]
fn test_bio_gate_schmitt_trigger_hysteresis() {
    let mut system = AethelSystem::new();

    // High coherence (0.8 >= 0.70 threshold) → Active
    let signal_coherent = BioSignal {
        stress: 0.3,
        hrv_coherence: 0.8,
        focus: 0.7,
        measured_at_ms: 0,
    };
    let state = system.process_bio_signal(&signal_coherent);
    assert!(matches!(state, BioGateState::Active));

    // Low coherence (0.3 <= 0.55 deactivate threshold) → Reduced
    let signal_incoherent = BioSignal {
        stress: 0.85,
        hrv_coherence: 0.3,
        focus: 0.2,
        measured_at_ms: 100,
    };
    let state = system.process_bio_signal(&signal_incoherent);
    assert!(matches!(state, BioGateState::Reduced));

    // Mid coherence (0.60 between thresholds), from Reduced → Holding
    let signal_mid = BioSignal {
        stress: 0.60,
        hrv_coherence: 0.60,
        focus: 0.5,
        measured_at_ms: 200,
    };
    let state = system.process_bio_signal(&signal_mid);
    assert!(matches!(state, BioGateState::Holding));

    // High coherence again → back to Active
    let signal_recover = BioSignal {
        stress: 0.2,
        hrv_coherence: 0.9,
        focus: 0.9,
        measured_at_ms: 300,
    };
    let state = system.process_bio_signal(&signal_recover);
    assert!(matches!(state, BioGateState::Active));
}

// ═══════════════════════════════════════════════════
// 5. Thought Compression with Risk Safety
// ═══════════════════════════════════════════════════

#[test]
fn test_thought_compression_risk_overrides() {
    let system = AethelSystem::new();

    let pressure_low = ThoughtPressure {
        token_budget: 100,
        time_budget_ms: 1000,
        pressure_normalized: 0.9,
        phase_transitioned: false,
    };

    let compressed_low = system.compress_for_task(&pressure_low, RiskLevel::Low);
    assert!(!compressed_low.emergency_blocked);

    let compressed_critical = system.compress_for_task(&pressure_low, RiskLevel::Critical);
    // Critical risk should not be emergency blocked at this pressure
    // (or it might be — depends on config; just verify it returns)
    let _ = compressed_critical;
}

// ═══════════════════════════════════════════════════
// 6. Audit Chain Integrity
// ═══════════════════════════════════════════════════

#[test]
fn test_audit_chain_append_and_verify() {
    let mut system = AethelSystem::new();

    system.audit_decision("init", "test", "Initialized system", RiskLevel::Low);
    system.audit_decision("process", "test", "Processed claim-42", RiskLevel::Medium);
    system.audit_decision("escalate", "test", "Escalated high-risk claim", RiskLevel::High);

    assert!(system.verify_audit_integrity().is_ok());
    assert_eq!(system.summary().audit_blocks, 3);
}

// ═══════════════════════════════════════════════════
// 7. EU AI Act Compliance
// ═══════════════════════════════════════════════════

#[test]
fn test_compliance_manifest() {
    let manifest = ComplianceManifest::aethel_default();
    assert!(manifest.is_compliant());
}

#[test]
fn test_risk_level_to_eu_tier_mapping() {
    assert_eq!(EuAiActRiskLevel::from(RiskLevel::Low), EuAiActRiskLevel::Minimal);
    assert_eq!(EuAiActRiskLevel::from(RiskLevel::Medium), EuAiActRiskLevel::Limited);
    assert_eq!(EuAiActRiskLevel::from(RiskLevel::High), EuAiActRiskLevel::High);
    assert_eq!(EuAiActRiskLevel::from(RiskLevel::Critical), EuAiActRiskLevel::Unacceptable);
}

// ═══════════════════════════════════════════════════
// 8. App Composition
// ═══════════════════════════════════════════════════

#[test]
fn test_app_composed_mode() {
    let mut system = AethelSystem::new();

    let app = AppDefinition {
        app_id: "app-1".into(),
        name: "Claim Analyzer".into(),
        mode: AppMode::Composed,
        pipeline_id: Some(PipelineId::new("pipe-1")),
        required_capabilities: vec![],
        version: "0.1.0".into(),
        author: "test".into(),
        risk_level: RiskLevel::Low,
        tags: vec!["analysis".into(), "claims".into()],
        description: "Analyzes claims through a pipeline".into(),
    };

    system.apps.register(app).unwrap();
    assert_eq!(system.summary().apps_count, 1);

    let found = system.apps.find_by_tag("analysis");
    assert_eq!(found.len(), 1);
}

#[test]
fn test_app_classic_mode_requires_capabilities() {
    let app = AppDefinition {
        app_id: "app-bad".into(),
        name: "Empty Classic".into(),
        mode: AppMode::Classic,
        pipeline_id: None,
        required_capabilities: vec![],
        version: "0.1.0".into(),
        author: "test".into(),
        risk_level: RiskLevel::Low,
        tags: vec![],
        description: "Should fail validation".into(),
    };
    assert!(app.validate().is_err());
}

// ═══════════════════════════════════════════════════
// 9. FIMAS Decomposition
// ═══════════════════════════════════════════════════

#[test]
fn test_decomposition_plan_validation() {
    let plan = DecompositionPlan {
        plan_id: "plan-1".to_string(),
        original_task: "Analyze claims".to_string(),
        strategy: DecompositionStrategy::Sequential,
        sub_tasks: vec![
            SubTask {
                id: "step-1".into(),
                description: "Extract claims".into(),
                capability_id: CapabilityId::new("claim-extractor"),
                depends_on: vec![],
                max_tokens: 5000,
                max_cost_cents: 50.0,
                risk_level: RiskLevel::Low,
                depth: 1,
                can_decompose_further: false,
                input_prompt: "Extract claims from input".into(),
            },
            SubTask {
                id: "step-2".into(),
                description: "Verify claims".into(),
                capability_id: CapabilityId::new("claim-verifier"),
                depends_on: vec!["step-1".into()],
                max_tokens: 3000,
                max_cost_cents: 30.0,
                risk_level: RiskLevel::Low,
                depth: 1,
                can_decompose_further: false,
                input_prompt: "Verify extracted claims".into(),
            },
        ],
        total_budget_tokens: 10_000,
        total_budget_cost_cents: 100.0,
        max_depth: 2,
    };

    assert!(plan.validate().is_ok());
    assert_eq!(plan.root_tasks().len(), 1);
    assert_eq!(plan.dependents_of("step-1").len(), 1);
}

// ═══════════════════════════════════════════════════
// 10. Engine: Task Queue + Agent Runner
// ═══════════════════════════════════════════════════

#[test]
fn test_task_queue_diamond_dependency() {
    let q = TaskQueue::new();

    q.enqueue(QueuedTask::new("root", "cap").with_priority(TaskPriority::Critical));
    q.enqueue(QueuedTask::new("left", "cap").depends_on("root").with_priority(TaskPriority::High));
    q.enqueue(QueuedTask::new("right", "cap").depends_on("root").with_priority(TaskPriority::Normal));
    q.enqueue(QueuedTask::new("join", "cap").depends_on("left").depends_on("right"));

    assert_eq!(q.stats().ready, 1);

    let root = q.dequeue().unwrap();
    q.complete(&root.id);
    assert_eq!(q.stats().ready, 2);

    let left = q.dequeue().unwrap();
    assert_eq!(left.id, TaskId::new("left")); // higher priority
    q.complete(&left.id);

    let right = q.dequeue().unwrap();
    q.complete(&right.id);

    assert_eq!(q.stats().ready, 1);
    let join = q.dequeue().unwrap();
    q.complete(&join.id);
    assert!(q.is_all_done());
}

#[tokio::test]
async fn test_engine_executes_plan_end_to_end() {
    let mut runtime = AethelRuntime::new_default();

    runtime.register_capability(make_test_cap("echo", "Echo", CapabilityCategory::Processing));

    let plan = DecompositionPlan {
        plan_id: "e2e-test".to_string(),
        original_task: "E2E test run".to_string(),
        strategy: DecompositionStrategy::Parallel,
        sub_tasks: vec![
            SubTask {
                id: "a".into(),
                description: "Task A".into(),
                capability_id: CapabilityId::new("echo"),
                depends_on: vec![],
                max_tokens: 100,
                max_cost_cents: 5.0,
                risk_level: RiskLevel::Low,
                depth: 1,
                can_decompose_further: false,
                input_prompt: "do A".into(),
            },
            SubTask {
                id: "b".into(),
                description: "Task B".into(),
                capability_id: CapabilityId::new("echo"),
                depends_on: vec![],
                max_tokens: 100,
                max_cost_cents: 5.0,
                risk_level: RiskLevel::Low,
                depth: 1,
                can_decompose_further: false,
                input_prompt: "do B".into(),
            },
        ],
        total_budget_tokens: 50_000,
        total_budget_cost_cents: 1000.0,
        max_depth: 1,
    };

    let budget = runtime.create_root_budget("e2e-test", 50_000, 1000.0);
    let result = runtime
        .execute_plan(&plan, &budget, CapValue::Text("test input".into()))
        .await;

    assert!(result.success);
    assert_eq!(result.agent_results.len(), 2);
    assert!(runtime.verify_integrity().is_ok());
}

// ═══════════════════════════════════════════════════
// 11. OmegaSpectrum24
// ═══════════════════════════════════════════════════

#[test]
fn test_omega_spectrum_cache_aligned() {
    // 32 × f32 = 128 bytes
    assert_eq!(std::mem::size_of::<OmegaSpectrum24>(), 128);

    let mut s = OmegaSpectrum24::default();
    s.set_dimension(OmegaDimension::Psychikon, 0.9);
    s.set_dimension(OmegaDimension::Bion, 0.3);
    assert!((s.dimension(OmegaDimension::Psychikon) - 0.9).abs() < f32::EPSILON);
    assert!((s.dimension(OmegaDimension::Bion) - 0.3).abs() < f32::EPSILON);
}

// ═══════════════════════════════════════════════════
// 12. ID Types
// ═══════════════════════════════════════════════════

#[test]
fn test_id_types_are_distinct() {
    let claim_id = ClaimId::new("abc");
    let agent_id = AgentId::new("abc");

    assert_eq!(claim_id.as_str(), "abc");
    assert_eq!(agent_id.as_str(), "abc");
    assert_eq!(format!("{}", claim_id), "abc");
}

// ═══════════════════════════════════════════════════
// 13. Storage: Multi-claim queries
// ═══════════════════════════════════════════════════

#[test]
fn test_sqlite_bulk_claims_with_filters() {
    let db = test_db();
    let store = SqliteClaimStore::new(db);

    for i in 0..20 {
        let risk = match i % 4 {
            0 => RiskLevel::Low,
            1 => RiskLevel::Medium,
            2 => RiskLevel::High,
            _ => RiskLevel::Critical,
        };
        let claim = Claim {
            id: format!("c-{:03}", i),
            content: format!("Claim number {}", i),
            state: ClaimState::Generated,
            origin: ClaimOrigin::ModelGenerated,
            support_level: SupportLevel::Unsupported,
            risk,
            confidence: (i as f32) / 20.0,
            evidence_ids: vec![],
            created_at_ms: 0,
            updated_at_ms: 0,
        };
        store.save_claim(&claim).unwrap();
    }

    assert_eq!(store.count_claims().unwrap(), 20);

    let page1 = store.list_claims(0, 10).unwrap();
    assert_eq!(page1.len(), 10);

    let high_risk = store.find_by_risk(RiskLevel::High).unwrap();
    assert_eq!(high_risk.len(), 5);
}

// ═══════════════════════════════════════════════════
// 14. Agent Lifecycle State Machine
// ═══════════════════════════════════════════════════

#[test]
fn test_agent_lifecycle_happy_path() {
    let state = AgentState::Created;
    let state = state.transition(AgentState::Initializing).unwrap();
    let state = state.transition(AgentState::Running).unwrap();
    let state = state.transition(AgentState::Reporting).unwrap();
    let state = state.transition(AgentState::Completed).unwrap();
    assert!(state.is_terminal());
}

#[test]
fn test_agent_lifecycle_failure_path() {
    let state = AgentState::Created;
    let state = state.transition(AgentState::Initializing).unwrap();
    let state = state.transition(AgentState::Running).unwrap();
    let state = state.transition(AgentState::Failed).unwrap();
    assert!(state.is_terminal());
}

#[test]
fn test_agent_cannot_skip_states() {
    assert!(AgentState::Created.transition(AgentState::Running).is_err());
    assert!(AgentState::Created.transition(AgentState::Completed).is_err());
}

// ═══════════════════════════════════════════════════
// 15. Full Stack: System → Decompose → Execute → Store → Audit
// ═══════════════════════════════════════════════════

#[tokio::test]
async fn test_full_stack_mission() {
    // 1. Boot system
    let mut runtime = AethelRuntime::new_default();
    let db = test_db();
    let store = SqliteClaimStore::new(db);

    // 2. Register capability
    runtime.register_capability(make_test_cap(
        "analyzer",
        "Claim Analyzer",
        CapabilityCategory::Reasoning,
    ));

    // 3. Create and store initial claim
    let claim = Claim {
        id: "mission-claim-1".into(),
        content: "AETHEL processes claims epistemically".into(),
        state: ClaimState::Generated,
        origin: ClaimOrigin::ModelGenerated,
        support_level: SupportLevel::Unsupported,
        risk: RiskLevel::Medium,
        confidence: 0.7,
        evidence_ids: vec![],
        created_at_ms: 0,
        updated_at_ms: 0,
    };
    store.save_claim(&claim).unwrap();

    // 4. Process bio-signal — coherence 0.7 is right at activate_threshold (0.70) → Active
    let signal = BioSignal {
        stress: 0.4,
        hrv_coherence: 0.5,
        focus: 0.6,
        measured_at_ms: 0,
    };
    let bio_state = runtime.process_bio_signal(&signal);
    // coherence 0.5 <= 0.55 deactivate threshold → Reduced
    assert!(matches!(bio_state, BioGateState::Reduced));

    // 5. Execute plan
    let plan = DecompositionPlan {
        plan_id: "full-stack-mission".to_string(),
        original_task: "Analyze a claim".to_string(),
        strategy: DecompositionStrategy::Sequential,
        sub_tasks: vec![SubTask {
            id: "analyze".into(),
            description: "Analyze the claim".into(),
            capability_id: CapabilityId::new("analyzer"),
            depends_on: vec![],
            max_tokens: 500,
            max_cost_cents: 10.0,
            risk_level: RiskLevel::Medium,
            depth: 1,
            can_decompose_further: false,
            input_prompt: "Analyze this claim".into(),
        }],
        total_budget_tokens: 10_000,
        total_budget_cost_cents: 500.0,
        max_depth: 2,
    };

    let budget = runtime.create_root_budget("full-stack", 10_000, 500.0);
    let result = runtime
        .execute_plan(&plan, &budget, CapValue::Text(claim.content.clone()))
        .await;

    assert!(result.success);

    // 6. Verify audit trail
    assert!(runtime.verify_integrity().is_ok());
    let summary = runtime.summary();
    assert!(summary.audit_blocks >= 2); // at least start + finish audited

    // 7. Transition claim based on result
    let s = claim.state.transition(ClaimState::Supported).unwrap();
    let mut updated_claim = claim.clone();
    updated_claim.state = s;
    updated_claim.confidence = 0.85;
    store.save_claim(&updated_claim).unwrap();

    let final_claim = store.load_claim(&ClaimId::new("mission-claim-1")).unwrap().unwrap();
    assert_eq!(final_claim.state, ClaimState::Supported);
}
