//! Storage traits — persistence abstraction for the AETHEL system.
//!
//! Every persistent operation goes through these traits.
//! Implementations can be SQLite, PostgreSQL, in-memory, or file-based.

use crate::{
    AethelError, Claim, ClaimId, AethelTrace, TraceId,
    AgentReport, AgentId,
};

/// Claim storage — CRUD for claims.
#[async_trait::async_trait]
pub trait ClaimStore: Send + Sync {
    /// Save a claim (insert or update).
    async fn save_claim(&self, claim: &Claim) -> Result<(), AethelError>;
    /// Load a claim by ID.
    async fn load_claim(&self, id: &ClaimId) -> Result<Option<Claim>, AethelError>;
    /// List all claims (paginated).
    async fn list_claims(&self, offset: usize, limit: usize) -> Result<Vec<Claim>, AethelError>;
    /// Delete a claim.
    async fn delete_claim(&self, id: &ClaimId) -> Result<bool, AethelError>;
    /// Count all claims.
    async fn count_claims(&self) -> Result<usize, AethelError>;
}

/// Trace storage — audit trail for all decisions.
#[async_trait::async_trait]
pub trait TraceStore: Send + Sync {
    /// Save a trace.
    async fn save_trace(&self, trace: &AethelTrace) -> Result<(), AethelError>;
    /// Load a trace by ID.
    async fn load_trace(&self, id: &TraceId) -> Result<Option<AethelTrace>, AethelError>;
    /// List traces for a mission.
    async fn list_traces_for_mission(&self, mission_id: &str) -> Result<Vec<AethelTrace>, AethelError>;
}

/// Agent report storage.
#[async_trait::async_trait]
pub trait AgentReportStore: Send + Sync {
    /// Save an agent report.
    async fn save_report(&self, report: &AgentReport) -> Result<(), AethelError>;
    /// Load a report by agent ID.
    async fn load_report(&self, agent_id: &AgentId) -> Result<Option<AgentReport>, AethelError>;
    /// List all reports (paginated).
    async fn list_reports(&self, offset: usize, limit: usize) -> Result<Vec<AgentReport>, AethelError>;
}

/// In-memory implementation for testing.
pub struct InMemoryClaimStore {
    claims: std::sync::Mutex<Vec<Claim>>,
}

impl InMemoryClaimStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self { claims: std::sync::Mutex::new(Vec::new()) }
    }
}

impl Default for InMemoryClaimStore {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl ClaimStore for InMemoryClaimStore {
    async fn save_claim(&self, claim: &Claim) -> Result<(), AethelError> {
        let mut claims = self.claims.lock().map_err(|e| AethelError::Storage(e.to_string()))?;
        if let Some(existing) = claims.iter_mut().find(|c| c.id == claim.id) {
            *existing = claim.clone();
        } else {
            claims.push(claim.clone());
        }
        Ok(())
    }

    async fn load_claim(&self, id: &ClaimId) -> Result<Option<Claim>, AethelError> {
        let claims = self.claims.lock().map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(claims.iter().find(|c| c.id == id.as_str()).cloned())
    }

    async fn list_claims(&self, offset: usize, limit: usize) -> Result<Vec<Claim>, AethelError> {
        let claims = self.claims.lock().map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(claims.iter().skip(offset).take(limit).cloned().collect())
    }

    async fn delete_claim(&self, id: &ClaimId) -> Result<bool, AethelError> {
        let mut claims = self.claims.lock().map_err(|e| AethelError::Storage(e.to_string()))?;
        let len_before = claims.len();
        claims.retain(|c| c.id != id.as_str());
        Ok(claims.len() < len_before)
    }

    async fn count_claims(&self) -> Result<usize, AethelError> {
        let claims = self.claims.lock().map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(claims.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClaimState, ClaimOrigin, SupportLevel, RiskLevel};

    fn make_claim(id: &str) -> Claim {
        Claim {
            id: id.into(),
            content: format!("Claim {}", id),
            state: ClaimState::Generated,
            origin: ClaimOrigin::ModelGenerated,
            support_level: SupportLevel::Unsupported,
            risk: RiskLevel::Low,
            confidence: 0.5,
            evidence_ids: vec![],
            created_at_ms: 0,
            updated_at_ms: 0,
        }
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let store = InMemoryClaimStore::new();
        let claim = make_claim("c1");
        store.save_claim(&claim).await.unwrap();
        let loaded = store.load_claim(&ClaimId::new("c1")).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, "c1");
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let store = InMemoryClaimStore::new();
        let loaded = store.load_claim(&ClaimId::new("nope")).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_save_overwrites() {
        let store = InMemoryClaimStore::new();
        let mut claim = make_claim("c1");
        store.save_claim(&claim).await.unwrap();
        claim.confidence = 0.9;
        store.save_claim(&claim).await.unwrap();
        let loaded = store.load_claim(&ClaimId::new("c1")).await.unwrap().unwrap();
        assert!((loaded.confidence - 0.9).abs() < 0.01);
        assert_eq!(store.count_claims().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_delete() {
        let store = InMemoryClaimStore::new();
        store.save_claim(&make_claim("c1")).await.unwrap();
        assert!(store.delete_claim(&ClaimId::new("c1")).await.unwrap());
        assert_eq!(store.count_claims().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let store = InMemoryClaimStore::new();
        assert!(!store.delete_claim(&ClaimId::new("nope")).await.unwrap());
    }

    #[tokio::test]
    async fn test_list_claims_pagination() {
        let store = InMemoryClaimStore::new();
        for i in 0..10 {
            store.save_claim(&make_claim(&format!("c{}", i))).await.unwrap();
        }
        let page1 = store.list_claims(0, 5).await.unwrap();
        assert_eq!(page1.len(), 5);
        let page2 = store.list_claims(5, 5).await.unwrap();
        assert_eq!(page2.len(), 5);
        let page3 = store.list_claims(10, 5).await.unwrap();
        assert!(page3.is_empty());
    }

    #[tokio::test]
    async fn test_count() {
        let store = InMemoryClaimStore::new();
        assert_eq!(store.count_claims().await.unwrap(), 0);
        store.save_claim(&make_claim("c1")).await.unwrap();
        store.save_claim(&make_claim("c2")).await.unwrap();
        assert_eq!(store.count_claims().await.unwrap(), 2);
    }

    #[test]
    fn test_store_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<InMemoryClaimStore>();
    }
}
