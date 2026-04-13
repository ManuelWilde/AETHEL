//! End-to-end integration tests for the AETHEL platform.
//!
//! These tests exercise the full stack: contracts → engine → storage,
//! verifying that all crates work together correctly.

use aethel_contracts::*;
use aethel_engine::*;
use aethel_storage::*;
use std::sync::Arc;

// ═══════════════════════════════════════════════════
// 1. Full System Lifecycle
// ═══════════════════════════════════════════════════

#[test]
fn test_system_boots_and_reports_healthy() {
    let system = AethelSystem::new(
        ComplianceManifest::aethel_default(),
        CompressionConfig::default(),
    );
    let summary = system.summary();
    assert!(summary.compliant);
    assert!(summary.audit_integrity);
    assert!(!summary.bio_gate_active);
    assert_eq!(summary.registered_capabilities, 0);
    assert_eq!(summary.registered_apps, 0);
}

#[test]
fn test_capability_registration_and_discovery() {
    let system = AethelSystem::new(
        ComplianceManifest::aethel_default(),
        CompressionConfig::default(),
    );

    let sensing = CapabilityDescriptor {
        id: CapabilityId::new("sense-bio"),
        name: "Bio Sensor".into(),
        category: CapabilityCategory::Sensing,
        input_type: "BioSignal".into(),
        output_type: "Spectrum".into(),
        estimated_cost_per_call: 5,
        estimated_latency_ms: 100,
        risk_level: RiskLevel::Low,
    };

    let processing = CapabilityDescriptor {
        id: CapabilityId::new("process-spectrum"),
        name: "Spectrum Analyzer".into(),
        category: CapabilityCategory::Processing,
        input_type: "Spectrum".into(),
        output_type: "Text".into(),
        estimated_cost_per_call: 20,
        estimated_latency_ms: 500,
        risk_level: RiskLevel::Medium,
    };

    system.capabilities.register(sensing);
    system.capabilities.register(processing);

    assert_eq!(system.summary().registered_capabilities, 2);

    let sensors = system.capabilities.find_by_category(CapabilityCategory::Sensing);
    assert_eq!(sensors.len(), 1);
    assert_eq!(sensors[0].name, "Bio Sensor");

    let connectable = system.capabilities.find_connectable_after(&CapabilityId::new("sense-bio"));
    assert_eq!(connectable.len(), 1);
    assert_eq!(connectable[0].name, "Spectrum Analyzer");
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
        origin: ClaimOrigin::UserSupplied,
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
        lease_id: LeaseId::new("root"),
        parent_lease: None,
        max_tokens: 10_000,
        max_cost_cents: 500,
        used_tokens: 0,
        used_cost_cents: 0,
        max_depth: 5,
        current_depth: 0,
        max_duration_ms: 60_000,
    };

    assert!(root.consume(1000, 50).is_ok());
    assert_eq!(root.remaining_tokens(), 9000);

    let child = root.sub_lease(3000, 100).unwrap();
    assert_eq!(child.max_tokens, 3000);
    assert_eq!(child.current_depth, 1);

    let mut small = BudgetLease {
        lease_id: LeaseId::new("small"),
        parent_lease: None,
        max_tokens: 100,
        max_cost_cents: 10,
        used_tokens: 0,
        used_cost_cents: 0,
        max_depth: 1,
        current_depth: 0,
        max_duration_ms: 1000,
    };
    assert!(small.consume(100, 10).is_ok());
    assert!(small.is_exhausted());
    assert!(small.consume(1, 0).is_err());
}

// ═══════════════════════════════════════════════════
// 4. Bio-Adaptive Routing
// ═══════════════════════════════════════════════════

#[test]
fn test_bio_gate_schmitt_trigger_hysteresis() {
    let system = AethelSystem::new(
        ComplianceManifest::aethel_default(),
        CompressionConfig::default(),
    );

    let activated = system.process_bio_signal(0.3, 0.8, 0.7);
    assert!(!activated);

    let activated = system.process_bio_signal(0.85, 0.3, 0.2);
    assert!(activated);

    let activated = system.process_bio_signal(0.60, 0.5, 0.5);
    assert!(activated); // hysteresis keeps it active

    let activated = system.process_bio_signal(0.2, 0.9, 0.9);
    assert!(!activated);
}

// ═══════════════════════════════════════════════════
// 5. Thought Compression with Risk Safety
// ═══════════════════════════════════════════════════

#[test]
fn test_thought_compression_risk_overrides() {
    let system = AethelSystem::new(
        ComplianceManifest::aethel_default(),
        CompressionConfig::default(),
    );

    let long_thought = "This is a very detailed and thorough analysis of the \
        epistemic foundations that requires careful consideration.";

    let compressed_low = system.compress_for_task(long_thought, 0.9, RiskLevel::Low);
    assert!(!compressed_low.is_empty());

    let compressed_critical = system.compress_for_task(long_thought, 0.9, RiskLevel::Critical);
    assert!(!compressed_critical.is_empty());
    assert!(compressed_critical.len() >= compressed_low.len());
}

// ═══════════════════════════════════════════════════
// 6. Audit Chain Integrity
// ═══════════════════════════════════════════════════

#[test]
fn test_audit_chain_append_and_verify() {
    let system = AethelSystem::new(
        ComplianceManifest::aethel_default(),
        CompressionConfig::default(),
    );

    system.audit_decision("Initialized system", RiskLevel::Low);
    system.audit_decision("Processed claim-42", RiskLevel::Medium);
    system.audit_decision("Escalated high-risk claim", RiskLevel::High);

    assert!(system.verify_audit_integrity());
    assert_eq!(system.summary().audit_blocks, 3);
}

// ═══════════════════════════════════════════════════
// 7. EU AI Act Compliance
// ═══════════════════════════════════════════════════

#[test]
fn test_compliance_manifest() {
    let manifest = ComplianceManifest::aethel_default();
    assert!(manifest.is_compliant());
    assert_eq!(manifest.risk_tier, EuAiActRiskLevel::High);
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
    let system = AethelSystem::new(
        ComplianceManifest::aethel_default(),
        CompressionConfig::default(),
    );

    let app = AppDefinition {
        id: "app-1".into(),
        name: "Claim Analyzer".into(),
        mode: AppMode::Composed,
        pipeline_id: Some(PipelineId::new("pipe-1")),
        capabilities: vec![],
        tags: vec!["analysis".into(), "claims".into()],
        description: "Analyzes claims through a pipeline".into(),
    };

    system.apps.register(app).unwrap();
    assert_eq!(system.summary().registered_apps, 1);

    let found = system.apps.find_by_tag("analysis");
    assert_eq!(found.len(), 1);
}

#[test]
fn test_app_classic_mode_requires_capabilities() {
    let app = AppDefinition {
        id: "app-bad".into(),
        name: "Empty Classic".into(),
        mode: AppMode::Classic,
        pipeline_id: None,
        capabilities: vec![],
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
        mission_id: MissionId::new("mission-1"),
        strategy: DecompositionStrategy::Sequential,
        sub_tasks: vec![
            SubTask {
                id: "step-1".into(),
                description: "Extract claims".into(),
                capability_name: "claim-extractor".into(),
                depends_on: vec![],
                estimated_tokens: 5000,
                estimated_cost_cents: 50,
                depth: 1,
            },
            SubTask {
                id: "step-2".into(),
                description: "Verify claims".into(),
                capability_name: "claim-verifier".into(),
                depends_on: vec!["step-1".into()],
                estimated_tokens: 3000,
                estimated_cost_cents: 30,
                depth: 1,
            },
        ],
    };

    assert!(plan.validate(10_000, 100).is_ok());
    assert!(plan.validate(4_000, 100).is_err());
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
    let runtime = AethelRuntime::new_default();

    runtime.register_capability(CapabilityDescriptor {
        id: CapabilityId::new("echo"),
        name: "Echo".into(),
        category: CapabilityCategory::Processing,
        input_type: "Text".into(),
        output_type: "Text".into(),
        estimated_cost_per_call: 5,
        estimated_latency_ms: 10,
        risk_level: RiskLevel::Low,
    });

    let plan = DecompositionPlan {
        mission_id: MissionId::new("e2e-test"),
        strategy: DecompositionStrategy::Parallel,
        sub_tasks: vec![
            SubTask {
                id: "a".into(),
                description: "Task A".into(),
                capability_name: "echo".into(),
                depends_on: vec![],
                estimated_tokens: 100,
                estimated_cost_cents: 5,
                depth: 1,
            },
            SubTask {
                id: "b".into(),
                description: "Task B".into(),
                capability_name: "echo".into(),
                depends_on: vec![],
                estimated_tokens: 100,
                estimated_cost_cents: 5,
                depth: 1,
            },
        ],
    };

    let budget = runtime.create_root_budget("e2e-test", 50_000, 1000);
    let result = runtime
        .execute_plan(&plan, &budget, CapValue::Text("test input".into()))
        .await;

    assert!(result.success);
    assert_eq!(result.agent_results.len(), 2);
    assert!(runtime.verify_integrity());
}

// ═══════════════════════════════════════════════════
// 11. OmegaSpectrum24
// ═══════════════════════════════════════════════════

#[test]
fn test_omega_spectrum_cache_aligned() {
    assert_eq!(std::mem::size_of::<OmegaSpectrum24>(), 128);

    let mut s = OmegaSpectrum24::default();
    s.values[OmegaDimension::Psychikon as usize] = 0.9;
    s.values[OmegaDimension::Bion as usize] = 0.3;
    assert_eq!(s.dominant_dimension(), OmegaDimension::Psychikon);
}

// ═══════════════════════════════════════════════════
// 12. ID Types
// ═══════════════════════════════════════════════════

#[test]
fn test_id_types_are_distinct() {
    let claim_id = ClaimId::new("abc");
    let mission_id = MissionId::new("abc");
    let agent_id = AgentId::new("abc");

    assert_eq!(claim_id.as_str(), "abc");
    assert_eq!(mission_id.as_str(), "abc");
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
    let mut state = AgentState::Created;
    state = state.transition(AgentState::Initializing).unwrap();
    state = state.transition(AgentState::Running).unwrap();
    state = state.transition(AgentState::Reporting).unwrap();
    state = state.transition(AgentState::Completed).unwrap();
    assert!(state.is_terminal());
}

#[test]
fn test_agent_lifecycle_failure_path() {
    let mut state = AgentState::Created;
    state = state.transition(AgentState::Initializing).unwrap();
    state = state.transition(AgentState::Running).unwrap();
    state = state.transition(AgentState::Failed).unwrap();
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
    let runtime = AethelRuntime::new_default();
    let db = test_db();
    let store = SqliteClaimStore::new(db);

    // 2. Register capability
    runtime.register_capability(CapabilityDescriptor {
        id: CapabilityId::new("analyzer"),
        name: "Claim Analyzer".into(),
        category: CapabilityCategory::Reasoning,
        input_type: "Text".into(),
        output_type: "Text".into(),
        estimated_cost_per_call: 10,
        estimated_latency_ms: 200,
        risk_level: RiskLevel::Medium,
    });

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

    // 4. Process bio-signal
    let bio_active = runtime.process_bio_signal(0.4, 0.7, 0.6);
    assert!(!bio_active); // normal conditions

    // 5. Execute plan
    let plan = DecompositionPlan {
        mission_id: MissionId::new("full-stack-mission"),
        strategy: DecompositionStrategy::Sequential,
        sub_tasks: vec![SubTask {
            id: "analyze".into(),
            description: "Analyze the claim".into(),
            capability_name: "analyzer".into(),
            depends_on: vec![],
            estimated_tokens: 500,
            estimated_cost_cents: 10,
            depth: 1,
        }],
    };

    let budget = runtime.create_root_budget("full-stack", 10_000, 500);
    let result = runtime
        .execute_plan(&plan, &budget, CapValue::Text(claim.content.clone()))
        .await;

    assert!(result.success);

    // 6. Verify audit trail
    assert!(runtime.verify_integrity());
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
