//! AETHEL API Server — REST endpoints for the AETHEL platform.
//!
//! Endpoints:
//!   GET  /health                    — Health check
//!   GET  /api/v1/system/summary     — System summary
//!   POST /api/v1/claims             — Create a claim
//!   GET  /api/v1/claims             — List claims
//!   GET  /api/v1/claims/:id         — Get a claim
//!   DELETE /api/v1/claims/:id       — Delete a claim
//!   POST /api/v1/claims/:id/transition — Transition claim state
//!   POST /api/v1/bio/signal         — Process bio-signal
//!   GET  /api/v1/audit/verify       — Verify audit chain
//!   POST /api/v1/audit/record       — Record audit decision

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use aethel_contracts::*;
use aethel_storage::{DbPool, SqliteClaimStore};

// ─── Application State ──────────────────────────

struct AppState {
    system: AethelSystem,
    claim_store: SqliteClaimStore,
}

type SharedState = Arc<AppState>;

// ─── Request/Response DTOs ──────────────────────

#[derive(Deserialize)]
struct CreateClaimRequest {
    content: String,
    #[serde(default = "default_risk")]
    risk: String,
    #[serde(default = "default_confidence")]
    confidence: f32,
}

fn default_risk() -> String {
    "Low".to_string()
}
fn default_confidence() -> f32 {
    0.5
}

#[derive(Deserialize)]
struct TransitionRequest {
    target_state: String,
}

#[derive(Deserialize)]
struct BioSignalRequest {
    stress: f64,
    coherence: f64,
    focus: f64,
}

#[derive(Deserialize)]
struct AuditRecordRequest {
    decision: String,
    #[serde(default = "default_risk")]
    risk: String,
}

#[derive(Deserialize)]
struct PaginationParams {
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    20
}

#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Json<Self> {
        Json(Self {
            success: true,
            data: Some(data),
            error: None,
        })
    }

    fn err(msg: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::BAD_REQUEST,
            Json(Self {
                success: false,
                data: None,
                error: Some(msg.into()),
            }),
        )
    }
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

#[derive(Serialize)]
struct BioSignalResponse {
    stress: f64,
    coherence: f64,
    focus: f64,
    bio_gate_activated: bool,
}

#[derive(Serialize)]
struct AuditVerifyResponse {
    integrity: bool,
    block_count: usize,
}

#[derive(Serialize)]
struct ClaimResponse {
    id: String,
    content: String,
    state: String,
    origin: String,
    risk: String,
    confidence: f32,
    evidence_ids: Vec<String>,
}

impl From<Claim> for ClaimResponse {
    fn from(c: Claim) -> Self {
        Self {
            id: c.id,
            content: c.content,
            state: format!("{:?}", c.state),
            origin: format!("{:?}", c.origin),
            risk: format!("{:?}", c.risk),
            confidence: c.confidence,
            evidence_ids: c.evidence_ids,
        }
    }
}

// ─── Handlers ───────────────────────────────────

async fn health() -> Json<ApiResponse<HealthResponse>> {
    ApiResponse::ok(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn system_summary(
    State(state): State<SharedState>,
) -> Json<ApiResponse<SystemSummary>> {
    let summary = state.system.summary();
    ApiResponse::ok(summary)
}

async fn create_claim(
    State(state): State<SharedState>,
    Json(req): Json<CreateClaimRequest>,
) -> Result<Json<ApiResponse<ClaimResponse>>, (StatusCode, Json<ApiResponse<ClaimResponse>>)> {
    let risk = parse_risk(&req.risk);
    let id = format!(
        "claim-{}",
        &req.content[..req.content.len().min(12)]
            .replace(' ', "-")
            .to_lowercase()
    );

    let claim = Claim {
        id: id.clone(),
        content: req.content,
        state: ClaimState::Generated,
        origin: ClaimOrigin::UserSupplied,
        support_level: SupportLevel::Unsupported,
        risk,
        confidence: req.confidence,
        evidence_ids: vec![],
        created_at_ms: now_ms(),
        updated_at_ms: now_ms(),
    };

    state
        .claim_store
        .save_claim(&claim)
        .map_err(|e| ApiResponse::err(e.to_string()))?;

    // Audit the creation
    state.system.audit_decision(
        &format!("Claim created: {}", id),
        risk,
    );

    Ok(ApiResponse::ok(ClaimResponse::from(claim)))
}

async fn list_claims(
    State(state): State<SharedState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<Vec<ClaimResponse>>>, (StatusCode, Json<ApiResponse<Vec<ClaimResponse>>>)>
{
    let claims = state
        .claim_store
        .list_claims(params.offset, params.limit)
        .map_err(|e| ApiResponse::err(e.to_string()))?;

    let responses: Vec<ClaimResponse> = claims.into_iter().map(ClaimResponse::from).collect();
    Ok(ApiResponse::ok(responses))
}

async fn get_claim(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ClaimResponse>>, (StatusCode, Json<ApiResponse<ClaimResponse>>)> {
    let claim = state
        .claim_store
        .load_claim(&ClaimId::new(&id))
        .map_err(|e| ApiResponse::err(e.to_string()))?;

    match claim {
        Some(c) => Ok(ApiResponse::ok(ClaimResponse::from(c))),
        None => Err(ApiResponse::err(format!("Claim '{}' not found", id))),
    }
}

async fn delete_claim(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<String>>)> {
    let deleted = state
        .claim_store
        .delete_claim(&ClaimId::new(&id))
        .map_err(|e| ApiResponse::err(e.to_string()))?;

    if deleted {
        Ok(ApiResponse::ok(format!("Claim '{}' deleted", id)))
    } else {
        Err(ApiResponse::err(format!("Claim '{}' not found", id)))
    }
}

async fn transition_claim(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<TransitionRequest>,
) -> Result<Json<ApiResponse<ClaimResponse>>, (StatusCode, Json<ApiResponse<ClaimResponse>>)> {
    let mut claim = state
        .claim_store
        .load_claim(&ClaimId::new(&id))
        .map_err(|e| ApiResponse::err(e.to_string()))?
        .ok_or_else(|| ApiResponse::err(format!("Claim '{}' not found", id)))?;

    let target = parse_claim_state_str(&req.target_state);
    let new_state = claim
        .state
        .transition(target)
        .map_err(|e| ApiResponse::err(e.to_string()))?;

    claim.state = new_state;
    claim.updated_at_ms = now_ms();

    state
        .claim_store
        .save_claim(&claim)
        .map_err(|e| ApiResponse::err(e.to_string()))?;

    state.system.audit_decision(
        &format!("Claim '{}' transitioned to {:?}", id, new_state),
        claim.risk,
    );

    Ok(ApiResponse::ok(ClaimResponse::from(claim)))
}

async fn bio_signal(
    State(state): State<SharedState>,
    Json(req): Json<BioSignalRequest>,
) -> Json<ApiResponse<BioSignalResponse>> {
    let activated = state
        .system
        .process_bio_signal(req.stress, req.coherence, req.focus);

    ApiResponse::ok(BioSignalResponse {
        stress: req.stress,
        coherence: req.coherence,
        focus: req.focus,
        bio_gate_activated: activated,
    })
}

async fn audit_verify(
    State(state): State<SharedState>,
) -> Json<ApiResponse<AuditVerifyResponse>> {
    let integrity = state.system.verify_audit_integrity();
    let summary = state.system.summary();
    ApiResponse::ok(AuditVerifyResponse {
        integrity,
        block_count: summary.audit_blocks,
    })
}

async fn audit_record(
    State(state): State<SharedState>,
    Json(req): Json<AuditRecordRequest>,
) -> Json<ApiResponse<String>> {
    let risk = parse_risk(&req.risk);
    state.system.audit_decision(&req.decision, risk);
    ApiResponse::ok(format!("Decision recorded: {}", req.decision))
}

// ─── Helpers ────────────────────────────────────

fn parse_risk(s: &str) -> RiskLevel {
    match s.to_lowercase().as_str() {
        "low" => RiskLevel::Low,
        "medium" | "med" => RiskLevel::Medium,
        "high" => RiskLevel::High,
        "critical" | "crit" => RiskLevel::Critical,
        _ => RiskLevel::Low,
    }
}

fn parse_claim_state_str(s: &str) -> ClaimState {
    match s.to_lowercase().as_str() {
        "generated" => ClaimState::Generated,
        "supported" => ClaimState::Supported,
        "accepted" => ClaimState::Accepted,
        "deferred" => ClaimState::Deferred,
        "escalated" => ClaimState::Escalated,
        "revised" => ClaimState::Revised,
        "rejected" => ClaimState::Rejected,
        "retired" => ClaimState::Retired,
        _ => ClaimState::Generated,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─── Main ───────────────────────────────────────

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Database
    let db_path = std::env::var("AETHEL_DB").unwrap_or_else(|_| "aethel.db".to_string());
    let pool = DbPool::open(&db_path).expect("Failed to open database");
    pool.initialize().expect("Failed to run migrations");

    // System
    let system = AethelSystem::new(
        ComplianceManifest::aethel_default(),
        CompressionConfig::default(),
    );

    let state = Arc::new(AppState {
        system,
        claim_store: SqliteClaimStore::new(pool),
    });

    // Routes
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/system/summary", get(system_summary))
        .route("/api/v1/claims", post(create_claim))
        .route("/api/v1/claims", get(list_claims))
        .route("/api/v1/claims/{id}", get(get_claim))
        .route("/api/v1/claims/{id}", delete(delete_claim))
        .route("/api/v1/claims/{id}/transition", post(transition_claim))
        .route("/api/v1/bio/signal", post(bio_signal))
        .route("/api/v1/audit/verify", get(audit_verify))
        .route("/api/v1/audit/record", post(audit_record))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Server
    let port: u16 = std::env::var("AETHEL_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("AETHEL API Server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
