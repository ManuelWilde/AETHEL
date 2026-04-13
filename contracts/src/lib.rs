// AETHEL Contracts — Single Source of Truth
// Every type defined here exists ONCE. All crates depend on this.
// Future: generated from .proto files. Today: hand-written canonical Rust.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! # AETHEL Contracts
//!
//! The canonical type definitions for the entire AETHEL ecosystem.
//! Every concept — claims, omega spectra, LLM routing, verification,
//! control-plane decisions — is defined exactly once, here.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod error;
pub use error::AethelError;

pub mod ids;
pub use ids::*;

pub mod transitions;

pub mod budget;

pub mod capability;
pub use capability::*;

pub mod pipeline;
pub use pipeline::*;

pub mod registry;
pub use registry::*;

pub mod executor;
pub use executor::*;

// ─────────────────────────────────────────────
// L0: Primitives & Enums
// ─────────────────────────────────────────────

/// Readiness of any system component.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Readiness {
    Ready,
    Degraded,
    Blocked,
}

/// Risk classification for claims, operations, and decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// The six ecosystem strands that AETHEL unifies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StrandKind {
    DeepLearning,
    CoWork,
    AiderDesktop,
    AethelAgent,
    AethelPrompting,
    Fimas,
}

/// How a component communicates with the rest of the system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContractMode {
    /// Temporary: JSON bridge for Python services.
    JsonBridge,
    /// Target: shared protobuf contracts.
    SharedProto,
    /// Native Rust-only (no cross-language).
    NativeOnly,
}

// ─────────────────────────────────────────────
// L1: Claim Lifecycle (unified from 4 definitions)
// ─────────────────────────────────────────────

/// The unified claim state machine.
/// Merged from: aethelDeepLearning (7 states), aider_Desktop (7+8 states),
/// aethelCoWork (4 statuses). This is the ONE definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClaimState {
    /// Initial: claim has been generated (by model, human, or import).
    Generated,
    /// Evidence supports the claim but not yet accepted.
    Supported,
    /// Claim has been accepted after verification.
    Accepted,
    /// Claim is deferred — needs more evidence or context.
    Deferred,
    /// Claim has been escalated for human review.
    Escalated,
    /// Claim has been revised based on new evidence.
    Revised,
    /// Claim has been rejected — evidence contradicts it.
    Rejected,
    /// Claim has been retired — no longer relevant.
    Retired,
}

/// Where a claim originated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClaimOrigin {
    ModelGenerated,
    Retrieved,
    HumanEntered,
    ImportedRecord,
}

/// Who performed a review action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewActor {
    System,
    Human,
    Policy,
}

/// Level of support for a claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SupportLevel {
    Unsupported,
    WeakSupport,
    BoundedGrounding,
    StronglySupported,
}

/// A single claim in the AETHEL verification system.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claim {
    pub id: String,
    pub content: String,
    pub state: ClaimState,
    pub origin: ClaimOrigin,
    pub support_level: SupportLevel,
    pub risk: RiskLevel,
    pub confidence: f32,
    pub evidence_ids: Vec<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

// ─────────────────────────────────────────────
// L1: OmegaSpectrum (unified from 5 definitions)
// ─────────────────────────────────────────────

/// The 12 canonical ontological dimensions of AETHEL.
/// Present in v6, Gemini, aider_Desktop — now unified.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum OmegaDimension {
    Hyleron = 0,
    Bion = 1,
    Psychikon = 2,
    Sozikon = 3,
    Technikon = 4,
    Oikonomikon = 5,
    Politikon = 6,
    Sophistikon = 7,
    Noetikon = 8,
    Historeokon = 9,
    Pneumatikon = 10,
    Ludikon = 11,
}

/// The full 32-dimensional AETHEL vector.
/// Slots 0-11: 12 inner WeltΩ layers (OmegaDimension).
/// Slots 12-18: 7 Spheres (Matter, Life, Mind, Society, Technology, Spirit, Transcendence).
/// Slot 19: Apeiron — unbounded dimension, controls fractal depth.
/// Slots 20-23: Meta (Timestamp, Epistemic, BioSensitivity, TenantHash).
/// Slots 24-31: Reserved.
#[repr(C, align(128))]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OmegaSpectrum24 {
    pub values: [f32; 32],
}

impl OmegaSpectrum24 {
    /// Create a zeroed spectrum.
    pub const fn zero() -> Self {
        Self { values: [0.0; 32] }
    }

    /// Get value for a canonical dimension (0-11).
    pub fn dimension(&self, dim: OmegaDimension) -> f32 {
        self.values[dim as usize]
    }

    /// Set value for a canonical dimension.
    pub fn set_dimension(&mut self, dim: OmegaDimension, value: f32) {
        self.values[dim as usize] = value;
    }

    /// The Apeiron slot (19) — controls fractal depth.
    pub fn apeiron(&self) -> f32 {
        self.values[19]
    }

    /// Bio-sensitivity meta slot (22).
    pub fn bio_sensitivity(&self) -> f32 {
        self.values[22]
    }

    /// Extract the first 12 dimensions as compact spectrum.
    pub fn to_spectrum12(&self) -> OmegaSpectrum12 {
        let mut values = [0.0f32; 12];
        values.copy_from_slice(&self.values[..12]);
        OmegaSpectrum12 { values }
    }

    /// Fractal depth derived from Apeiron.
    /// max_depth = min(HARD_LIMIT, base_depth + floor(apeiron * scale))
    /// NASA JPL Power-of-Ten Rule #2: hard limit = 12.
    pub fn fractal_depth(&self) -> u32 {
        const HARD_LIMIT: u32 = 12;
        const BASE_DEPTH: u32 = 3;
        const SCALE: f32 = 9.0;
        let dynamic = (self.apeiron() * SCALE).floor() as u32;
        (BASE_DEPTH + dynamic).min(HARD_LIMIT)
    }
}

impl Default for OmegaSpectrum24 {
    fn default() -> Self {
        Self::zero()
    }
}

/// Compact 12-dimensional spectrum (the inner WeltΩ layers only).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OmegaSpectrum12 {
    pub values: [f32; 12],
}

// ─────────────────────────────────────────────
// L2: Bio-Gating
// ─────────────────────────────────────────────

/// Biological signal for the AETHEL bio-gating system.
/// Schmitt-Trigger hysteresis: high_threshold / low_threshold.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BioSignal {
    /// Current stress level (0.0 = calm, 1.0 = maximum stress).
    pub stress: f32,
    /// Current focus level (0.0 = distracted, 1.0 = deep focus).
    pub focus: f32,
    /// Heart rate variability indicator.
    pub hrv_coherence: f32,
    /// Timestamp of measurement.
    pub measured_at_ms: u64,
}

/// Schmitt-Trigger gate state for bio-responsive decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BioGateState {
    /// System is in responsive mode (bio signals above high threshold).
    Active,
    /// System is in reduced mode (bio signals below low threshold).
    Reduced,
    /// Hysteresis zone — maintain previous state.
    Holding,
}

// ─────────────────────────────────────────────
// L3: FIMAS Control Plane
// ─────────────────────────────────────────────

/// A routing decision made by the FIMAS control plane.
/// FIMAS decides: which model, which machine, which policy, which budget.
/// Key rule: Control Plane decides. Not Twin. Not Worker.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub decision_id: String,
    pub mission_id: String,
    /// The selected LLM provider + model.
    pub selected_provider: String,
    pub selected_model: String,
    /// Why this route was chosen (audit trail).
    pub routing_reason: String,
    /// Fallback chain if primary fails.
    pub fallback_chain: Vec<String>,
    /// Estimated cost in cents.
    pub cost_estimate_cents: f32,
    /// Estimated latency in ms.
    pub latency_estimate_ms: u32,
    /// The residency constraint.
    pub residency: RoutingResidency,
    /// Routes that were considered and rejected.
    pub rejected_routes: Vec<RejectedRoute>,
}

/// Where computation is allowed to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoutingResidency {
    /// Must run on local device (Apple Silicon, MLX).
    LocalOnly,
    /// Can run on remote API.
    RemoteAllowed,
    /// Prefer local, fallback to remote.
    PreferLocal,
}

/// A route that was considered but rejected.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RejectedRoute {
    pub provider: String,
    pub model: String,
    pub reason: String,
}

/// A budget lease granted by the FIMAS control plane.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BudgetLease {
    pub lease_id: String,
    pub mission_id: String,
    /// Maximum tokens this lease allows.
    pub max_tokens: u64,
    /// Maximum cost in cents.
    pub max_cost_cents: f32,
    /// Maximum wall-clock time in ms.
    pub max_duration_ms: u64,
    /// Tokens consumed so far.
    pub tokens_used: u64,
    /// Cost consumed so far.
    pub cost_used_cents: f32,
    pub granted_at_ms: u64,
    pub expires_at_ms: u64,
}

/// An execution branch in the FIMAS fractal topology.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionBranch {
    pub branch_id: String,
    pub parent_branch_id: Option<String>,
    pub mission_id: String,
    pub state: ExecutionBranchState,
    pub depth: u32,
    /// The omega spectrum at this branch point.
    pub spectrum: OmegaSpectrum24,
    /// Routing decision that created this branch.
    pub routing_decision_id: String,
    /// Budget lease governing this branch.
    pub budget_lease_id: String,
    pub created_at_ms: u64,
}

/// State of an execution branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionBranchState {
    Draft,
    Planned,
    Running,
    WaitingForReview,
    Completed,
    Failed,
    Cancelled,
    RolledBack,
}

/// A twin projection — observation-only doppelganger.
/// Twins observe. They do not decide.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TwinProjection {
    pub twin_id: String,
    pub source_branch_id: String,
    pub freshness: TwinFreshness,
    /// Observed cost so far.
    pub observed_cost_cents: f32,
    /// Observed health of the branch.
    pub observed_health: Readiness,
    /// Memory usage observed.
    pub observed_memory_bytes: u64,
    pub projected_at_ms: u64,
}

/// How fresh a twin's observation is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TwinFreshness {
    Current,
    Lagging,
    Stale,
    Recovering,
}

// ─────────────────────────────────────────────
// L4: Verification
// ─────────────────────────────────────────────

/// Result of a verification pass through the membrane.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationResult {
    pub claim_id: String,
    pub passed_layers: u8,
    pub total_layers: u8,
    pub final_confidence: f32,
    pub risk_assessment: RiskLevel,
    pub reviewer: ReviewActor,
    pub details: Vec<LayerResult>,
    pub verified_at_ms: u64,
}

/// Result of a single verification layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerResult {
    pub layer_index: u8,
    pub layer_name: String,
    pub passed: bool,
    pub confidence_delta: f32,
    pub note: String,
}

// ─────────────────────────────────────────────
// L5: LLM Provider (unified from 3 definitions)
// ─────────────────────────────────────────────

/// The kinds of LLM providers AETHEL can route to.
/// Merged from: aethelPromtEngineerung (6 kinds), aethelGemini (5 classes),
/// aethelAgent (6 kinds). This is the ONE definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderKind {
    /// Local MLX model on Apple Silicon.
    Mlx,
    /// OpenAI API.
    OpenAi,
    /// OpenAI-compatible API (Anthropic, Groq, Together, etc.).
    OpenAiCompatible,
    /// Local Ollama server.
    Ollama,
    /// Local llama.cpp server.
    LlamaCpp,
    /// vLLM server (local or remote).
    Vllm,
    /// Rule-based, no LLM needed — deterministic path.
    Deterministic,
}

/// Provider capability profile.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub provider: ProviderKind,
    pub model: String,
    pub max_context_tokens: u32,
    pub supports_json_schema: bool,
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub cost_per_1k_tokens: f32,
    pub is_local: bool,
}

/// A request to an LLM provider.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmRequest {
    pub provider: ProviderKind,
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub output_mode: OutputMode,
    pub response_schema: Option<String>,
}

/// How the LLM should format its output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutputMode {
    Text,
    Json,
    JsonSchema,
}

/// Response from an LLM provider.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmResponse {
    pub text: String,
    pub tokens_used: u32,
    pub model: String,
    pub duration_ms: u64,
    pub provider: ProviderKind,
}

/// The 10 Omega model roles from aethelGemini.
/// Each role can be bound to a different provider+model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OmegaModelRole {
    OmegaStruct,
    OmegaLogos,
    OmegaCode,
    OmegaPublish,
    OmegaExplain,
    OmegaReview,
    OmegaBitplay,
    OmegaAudio,
    OmegaSurface,
    OmegaEducation,
}

/// A model slot — binds a role to a specific provider+model.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelSlot {
    pub role: OmegaModelRole,
    pub primary_provider: String,
    pub fallback_provider: Option<String>,
    pub model_id: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub requires_retrieval: bool,
}

// ─────────────────────────────────────────────
// L6: Agent
// ─────────────────────────────────────────────

/// Task domain classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskDomain {
    Coding,
    PromptEngineering,
    IntentInference,
    Verification,
    Research,
    Operations,
    General,
}

/// Reasoning mode the agent should use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReasoningMode {
    Reactive,
    Deliberative,
    Search,
    Reflective,
    MultiAgent,
    HybridNeuroSymbolic,
}

/// A task request for the AETHEL agent system.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequest {
    pub id: String,
    pub domain: TaskDomain,
    pub prompt: String,
    pub risk: RiskLevel,
    pub reasoning_mode: ReasoningMode,
    /// Omega spectrum context for this task.
    pub spectrum: OmegaSpectrum24,
    /// FIMAS routing decision (if already made).
    pub routing_decision_id: Option<String>,
    /// FIMAS budget lease (if already granted).
    pub budget_lease_id: Option<String>,
}

// ─────────────────────────────────────────────
// L7: Governance
// ─────────────────────────────────────────────

/// A governance gate — approval checkpoint.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GovernanceGate {
    pub name: String,
    pub owner: String,
    pub readiness: Readiness,
    pub required: bool,
}

/// A work cell — bounded unit of attention.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkCell {
    pub name: String,
    pub mission: String,
    pub attention_budget: u32,
    pub requires_feedback_loop: bool,
}

/// Operator surface — how a human interacts with the system.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperatorSurface {
    pub name: String,
    pub supports_live_control: bool,
    pub supports_agent_topology: bool,
    pub supports_multi_user: bool,
}

// ─────────────────────────────────────────────
// L8: Forbidden Zones (immutable safety constraints)
// ─────────────────────────────────────────────

/// Operations that are NEVER allowed, regardless of permissions.
/// From aethelGemini ModelForbiddenZones.
pub const FORBIDDEN_OPERATIONS: &[&str] = &[
    "canonize",                    // No direct canonical writes
    "trigger_final_promotion",     // Promotion is system-controlled
    "override_policy",             // Policy changes require governance
    "replace_meta_governance",     // Meta-governance is immutable
    "solidify_unbound_world_claims", // Unbound claims must pass verification
    "direct_canonical_write",      // All writes through verified pipeline
    "bypass_verify",               // Diakrisis verification is mandatory
];

/// Check if an operation is forbidden.
pub fn is_forbidden(operation: &str) -> bool {
    FORBIDDEN_OPERATIONS.contains(&operation)
}

// ─────────────────────────────────────────────
// L9: Thought Economics (inspired by Muse Spark scaling principles)
// ─────────────────────────────────────────────
// Meta scales a MODEL along 3 axes: Pretraining, RL, Test-Time Compute.
// AETHEL scales a SYSTEM along 3 axes: Ontological, Epistemic, Bio-Adaptive.
// These types capture the system-level scaling laws.

/// Thought pressure applied to an agent execution.
/// Analogous to Muse Spark's Thought Compression: tighter budget = denser thinking.
/// Phase transition: at some pressure level, agents shift to fundamentally
/// more efficient reasoning (fewer tokens, same or better confidence).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThoughtPressure {
    /// Token budget for this thought. Lower = higher pressure.
    pub token_budget: u32,
    /// Time budget in milliseconds. Lower = higher pressure.
    pub time_budget_ms: u64,
    /// Normalized pressure (0.0 = unlimited, 1.0 = extreme compression).
    /// Computed from token_budget relative to task complexity.
    pub pressure_normalized: f32,
    /// Whether the agent has crossed the phase-transition threshold
    /// where compressed thinking becomes fundamentally more efficient.
    /// Observed empirically, not set manually.
    pub phase_transitioned: bool,
}

/// Efficiency metrics for a completed agent execution.
/// The core AETHEL scaling metric: epistemic value per cost.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThoughtEfficiency {
    /// Confidence achieved (0.0 - 1.0) from verification membrane.
    pub confidence_achieved: f32,
    /// Tokens consumed.
    pub tokens_consumed: u32,
    /// Wall-clock time in ms.
    pub duration_ms: u64,
    /// Cost in cents.
    pub cost_cents: f32,
    /// **Confidence per token** — the primary AETHEL efficiency metric.
    /// Higher = agent thinks more efficiently.
    /// Formula: confidence_achieved / tokens_consumed
    pub confidence_per_token: f32,
    /// **Confidence per cent** — epistemic value per cost.
    /// This is what AETHEL optimizes instead of "capability per FLOP".
    pub confidence_per_cent: f32,
    /// Verification layers passed.
    pub layers_passed: u8,
    /// Total verification layers.
    pub layers_total: u8,
}

impl ThoughtEfficiency {
    /// Compute efficiency metrics from raw measurements.
    pub fn compute(
        confidence: f32,
        tokens: u32,
        duration_ms: u64,
        cost_cents: f32,
        layers_passed: u8,
        layers_total: u8,
    ) -> Self {
        let cpt = if tokens > 0 { confidence / tokens as f32 } else { 0.0 };
        let cpc = if cost_cents > 0.0 { confidence / cost_cents } else { 0.0 };
        Self {
            confidence_achieved: confidence,
            tokens_consumed: tokens,
            duration_ms,
            cost_cents,
            confidence_per_token: cpt,
            confidence_per_cent: cpc,
            layers_passed,
            layers_total,
        }
    }
}

// ─────────────────────────────────────────────
// L10: Contemplation — Multi-Agent Parallel Reasoning
// ─────────────────────────────────────────────
// Analogous to Muse Spark's Contemplating Mode, but at system level:
// multiple agents with different models produce independent claims,
// then epistemic aggregation synthesizes the result.
// Not "best answer wins" — claims are weighted by verified confidence.

/// How a swarm of agents should reason about a problem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContemplationMode {
    /// Single agent, single model. Simple path.
    Solo,
    /// Multiple agents in parallel, same model.
    /// Cheap diversity through temperature/sampling variation.
    Ensemble,
    /// Multiple agents in parallel, different specialized models.
    /// Each model chosen by ontological fit (OmegaSpectrum routing).
    OntologicalSwarm,
    /// Sequential deepening: start with one agent, spawn sub-agents
    /// at increasing fractal depth if confidence is insufficient.
    FractalDeepening,
}

/// A contemplation session — manages parallel agent reasoning.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContemplationSession {
    pub session_id: String,
    pub mission_id: String,
    pub mode: ContemplationMode,
    /// How many parallel agents are active.
    pub swarm_size: u8,
    /// Maximum fractal depth allowed (from Apeiron).
    pub max_depth: u32,
    /// All claims produced by agents in this session.
    pub claims: Vec<SwarmClaim>,
    /// The synthesized result after epistemic aggregation.
    pub synthesis: Option<EpistemicSynthesis>,
    pub started_at_ms: u64,
    pub completed_at_ms: Option<u64>,
}

/// A claim produced by one agent in a contemplation swarm.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwarmClaim {
    pub agent_index: u8,
    pub claim: Claim,
    pub verification: Option<VerificationResult>,
    pub efficiency: ThoughtEfficiency,
    /// Which model produced this claim.
    pub provider: ProviderKind,
    pub model: String,
    /// The ontological spectrum of the agent's context.
    pub agent_spectrum: OmegaSpectrum24,
}

/// Epistemic synthesis — the aggregated result of contemplation.
/// Not "majority vote". Weighted by verified confidence + ontological fit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpistemicSynthesis {
    /// The synthesized claim (may combine insights from multiple agents).
    pub synthesized_claim: Claim,
    /// How confidence was aggregated.
    pub aggregation_method: AggregationMethod,
    /// Total epistemic weight (sum of confidence × layer_pass_rate × ontological_fit).
    pub total_epistemic_weight: f32,
    /// Agreement ratio among agents (0.0 = total disagreement, 1.0 = unanimous).
    pub agreement_ratio: f32,
    /// Dissenting claims that contradicted the synthesis.
    pub dissenting_claims: Vec<String>,
    /// Total cost of the entire contemplation.
    pub total_cost_cents: f32,
    /// Total tokens consumed across all agents.
    pub total_tokens: u64,
    /// System-level efficiency: synthesis confidence per total cost.
    pub system_confidence_per_cent: f32,
}

/// How claims from multiple agents are aggregated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AggregationMethod {
    /// Weighted average by verified confidence.
    ConfidenceWeighted,
    /// Weighted by confidence × ontological fit to the task spectrum.
    OntologicalWeighted,
    /// Bayesian update: each claim updates a prior.
    BayesianUpdate,
    /// Highest verified confidence wins (but dissent is recorded).
    MaxConfidence,
}

// ─────────────────────────────────────────────
// L11: System Scaling Laws
// ─────────────────────────────────────────────
// Meta measures: pass@1, pass@16, log-linear RL scaling.
// AETHEL measures: confidence scaling, efficiency scaling, epistemic yield.
// These are the system's own scaling laws — predictable, measurable.

/// A scaling observation — one data point in AETHEL's self-measurement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScalingObservation {
    /// Which axis is being measured.
    pub axis: ScalingAxis,
    /// The independent variable (e.g., number of verification layers,
    /// number of swarm agents, token budget).
    pub input_variable: f64,
    /// The dependent variable (e.g., confidence, accuracy, cost).
    pub output_variable: f64,
    /// Task domain this was measured in.
    pub domain: TaskDomain,
    /// Timestamp.
    pub observed_at_ms: u64,
}

/// The three scaling axes of the AETHEL system.
/// Each should exhibit log-linear scaling when the system is healthy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScalingAxis {
    /// More verification layers → log-linearly higher confidence.
    /// x = number of membrane layers, y = final confidence.
    VerificationDepth,
    /// More swarm agents → log-linearly better claim quality.
    /// x = swarm size, y = synthesis confidence.
    SwarmBreadth,
    /// More token budget → log-linearly higher confidence, but with
    /// a phase-transition point where efficiency jumps.
    /// x = token budget, y = confidence_per_token.
    ThoughtBudget,
    /// Bio-adaptation: as bio-signal integration improves,
    /// operator satisfaction and task success rate increase.
    /// x = bio-signal fidelity, y = task success rate.
    BioAdaptation,
    /// Ontological routing precision: better spectrum classification
    /// → better model-task fit → higher confidence per cost.
    /// x = ontological fit score, y = confidence_per_cent.
    OntologicalPrecision,
}

// ─────────────────────────────────────────────
// L12: Responsible Scaling Architecture (inspired by Anthropic RSP)
// ─────────────────────────────────────────────
// Anthropic scales capability ONLY when safety catches up.
// AETHEL implements this as architectural constraints:
// - Fractal depth cannot exceed verification depth
// - Capability gates halt execution until safeguards are ready
// - Dual-use awareness through ontological risk scoring

/// Responsible Scaling Gate — the system's own RSP.
/// Before any execution branch spawns, this gate checks:
/// can we VERIFY what this branch will produce?
/// If not, the branch is blocked. Not deferred — blocked.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResponsibleScalingGate {
    /// Current verification capacity (number of membrane layers available).
    pub verification_capacity: u8,
    /// Requested execution depth (fractal depth of the branch).
    pub requested_depth: u32,
    /// The scaling ratio: verification_capacity / requested_depth.
    /// Must be >= 1.0 for the gate to open.
    /// If < 1.0: system refuses to execute — "Safeguards not ready."
    pub scaling_ratio: f32,
    /// Whether the gate allows execution.
    pub gate_open: bool,
    /// If blocked, why.
    pub block_reason: Option<String>,
}

impl ResponsibleScalingGate {
    /// Evaluate whether execution should proceed.
    /// Core rule: you cannot go deeper than you can verify.
    pub fn evaluate(verification_capacity: u8, requested_depth: u32) -> Self {
        let ratio = if requested_depth > 0 {
            verification_capacity as f32 / requested_depth as f32
        } else {
            f32::INFINITY
        };
        let gate_open = ratio >= 1.0;
        let block_reason = if !gate_open {
            Some(format!(
                "Verification capacity ({}) < requested depth ({}). \
                 Safeguards must catch up before scaling. RSP ratio: {:.2}",
                verification_capacity, requested_depth, ratio
            ))
        } else {
            None
        };
        Self {
            verification_capacity,
            requested_depth,
            scaling_ratio: ratio,
            gate_open,
            block_reason,
        }
    }
}

/// Ontological risk profile — dual-use awareness through the spectrum.
/// Certain ontological regions (e.g., high Technikon + low Ethikon)
/// automatically trigger stricter verification and governance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OntologicalRiskProfile {
    /// The spectrum being assessed.
    pub spectrum: OmegaSpectrum24,
    /// Computed risk score (0.0 = safe, 1.0 = maximum concern).
    pub dual_use_score: f32,
    /// Which dimensions contribute most to the risk.
    pub risk_drivers: Vec<(OmegaDimension, f32)>,
    /// Required governance level based on risk.
    pub required_governance: GovernanceLevel,
}

/// Governance levels triggered by ontological risk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GovernanceLevel {
    /// Standard: automated verification sufficient.
    Standard,
    /// Elevated: additional membrane layers required.
    Elevated,
    /// High: human review required before execution.
    HumanReviewRequired,
    /// Critical: execution blocked pending governance approval.
    /// Like Anthropic not releasing Mythos — capability exists but is held back.
    Blocked,
}

impl OntologicalRiskProfile {
    /// Compute dual-use risk from an OmegaSpectrum24.
    ///
    /// High risk indicators:
    /// - High Technikon (slot 4) + Low Pneumatikon (slot 10): tool without ethics
    /// - High Politikon (slot 6) + Low Noetikon (slot 8): power without knowledge
    /// - High capability dimensions + Low meta-epistemic (slot 21): confident but uncalibrated
    ///
    /// Low risk indicators:
    /// - Balanced spectrum: no extreme concentration
    /// - High Noetikon: knowledge-seeking
    /// - High Pneumatikon: aesthetically/spiritually grounded
    pub fn compute(spectrum: &OmegaSpectrum24) -> Self {
        let technikon = spectrum.dimension(OmegaDimension::Technikon);
        let pneumatikon = spectrum.dimension(OmegaDimension::Pneumatikon);
        let politikon = spectrum.dimension(OmegaDimension::Politikon);
        let noetikon = spectrum.dimension(OmegaDimension::Noetikon);
        let meta_epistemic = spectrum.values[21]; // Meta-Epistemic slot

        let mut risk_drivers = Vec::new();
        let mut score = 0.0f32;

        // Tool without grounding
        if technikon > 0.7 && pneumatikon < 0.3 {
            let contrib = (technikon - pneumatikon) * 0.4;
            score += contrib;
            risk_drivers.push((OmegaDimension::Technikon, contrib));
        }

        // Power without knowledge
        if politikon > 0.7 && noetikon < 0.3 {
            let contrib = (politikon - noetikon) * 0.3;
            score += contrib;
            risk_drivers.push((OmegaDimension::Politikon, contrib));
        }

        // Confident but uncalibrated
        if meta_epistemic < 0.2 {
            let contrib = (1.0 - meta_epistemic) * 0.3;
            score += contrib;
        }

        score = score.clamp(0.0, 1.0);

        let required_governance = match score {
            s if s < 0.25 => GovernanceLevel::Standard,
            s if s < 0.50 => GovernanceLevel::Elevated,
            s if s < 0.75 => GovernanceLevel::HumanReviewRequired,
            _ => GovernanceLevel::Blocked,
        };

        Self {
            spectrum: *spectrum,
            dual_use_score: score,
            risk_drivers,
            required_governance,
        }
    }
}

// ─────────────────────────────────────────────
// L13: The Three Scaling Philosophies — Unified
// ─────────────────────────────────────────────
// Meta:     "More capability per FLOP" — efficiency-first scaling.
// Anthropic: "Scale only when safety catches up" — safety-gated scaling.
// AETHEL:   "More verified truth per cost, but never faster than
//            verification can follow" — epistemically honest scaling.
//
// The synthesis:
// 1. From Meta:     Thought compression, predictable scaling laws,
//                   multi-agent contemplation, efficiency metrics.
// 2. From Anthropic: Capability gates, responsible scaling ratios,
//                   dual-use ontological risk, forbidden zones.
// 3. From AETHEL:  OmegaSpectrum routing, bio-adaptive pressure,
//                   claim lifecycle, epistemic aggregation.
//
// AETHEL's unique position: we don't build a better model.
// We build a system that makes ANY model — big or small, own or foreign —
// epistemically honest, ontologically aware, bio-adaptive, and safe.

/// The complete execution trace of the AETHEL system.
/// Every decision, every routing, every verification, every scaling gate.
/// Full auditability — not "trust me", but "here's the proof".
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AethelTrace {
    pub trace_id: String,
    pub mission_id: String,
    /// The task's ontological classification.
    pub spectrum: OmegaSpectrum24,
    /// Bio-signal at decision time.
    pub bio_signal: Option<BioSignal>,
    /// Ontological risk assessment.
    pub risk_profile: OntologicalRiskProfile,
    /// Responsible scaling gate result.
    pub scaling_gate: ResponsibleScalingGate,
    /// Triplex Via routing trace.
    pub routing_trace: Option<TriplexViaTrace>,
    /// Contemplation mode and results.
    pub contemplation: Option<ContemplationSession>,
    /// Thought pressure applied.
    pub thought_pressure: ThoughtPressure,
    /// Final efficiency metrics.
    pub efficiency: Option<ThoughtEfficiency>,
    /// All claims produced.
    pub claims: Vec<Claim>,
    /// All verifications performed.
    pub verifications: Vec<VerificationResult>,
    /// Twin observations.
    pub twin_projections: Vec<TwinProjection>,
    /// Scaling observations for the system's self-measurement.
    pub scaling_observations: Vec<ScalingObservation>,
    pub started_at_ms: u64,
    pub completed_at_ms: Option<u64>,
}

/// Triplex Via — the three-phase routing algorithm.
/// From aethelGemini's ProviderRouter, now system-wide.
///
/// Phase 1: Purificatio — Policy filter.
///   What is ALLOWED? Privacy constraints, residency rules, forbidden zones.
///   Hard constraints that eliminate providers.
///
/// Phase 2: Illuminatio — Bio-gating + ontological fit.
///   What is APPROPRIATE? Operator stress level, task spectrum,
///   available cognitive budget. Soft constraints that rank providers.
///
/// Phase 3: Unio — Selection.
///   What is OPTIMAL from what remains? Cost, latency, capability,
///   historical efficiency (confidence_per_cent for this domain).
///   Final pick + fallback chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TriplexViaTrace {
    /// Phase 1: providers eliminated by policy.
    pub purificatio_eliminated: Vec<RejectedRoute>,
    /// Phase 1: providers that survived policy filter.
    pub purificatio_survivors: Vec<String>,
    /// Phase 2: bio-signal used for ranking.
    pub illuminatio_bio_signal: Option<BioSignal>,
    /// Phase 2: ontological fit scores for surviving providers.
    pub illuminatio_fit_scores: Vec<(String, f32)>,
    /// Phase 3: final selection with reasoning.
    pub unio_selected: String,
    pub unio_reason: String,
    /// Phase 3: fallback chain.
    pub unio_fallbacks: Vec<String>,
    /// Total routing decision time in microseconds.
    pub routing_duration_us: u64,
}
