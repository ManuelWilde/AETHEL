//! SQLite-backed ClaimStore implementation.

use aethel_contracts::{
    AethelError, Claim, ClaimId, ClaimOrigin, ClaimState, RiskLevel, SupportLevel,
};
use crate::DbPool;

/// SQLite implementation of ClaimStore.
pub struct SqliteClaimStore {
    pool: DbPool,
}

impl SqliteClaimStore {
    /// Create a new store backed by the given database pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Save a claim (insert or replace).
    pub fn save_claim(&self, claim: &Claim) -> Result<(), AethelError> {
        let conn = self.pool.conn();
        conn.execute(
            "INSERT OR REPLACE INTO claims (id, mission_id, text, state, source, confidence, risk_level, metadata_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
            rusqlite::params![
                claim.id,
                "", // mission_id — Claim doesn't carry it, store empty
                claim.content,
                format!("{:?}", claim.state),
                format!("{:?}", claim.origin),
                claim.confidence,
                format!("{:?}", claim.risk),
                serde_json::to_string(&claim.evidence_ids).unwrap_or_default(),
            ],
        )
        .map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Load a claim by ID.
    pub fn load_claim(&self, id: &ClaimId) -> Result<Option<Claim>, AethelError> {
        let conn = self.pool.conn();
        let mut stmt = conn
            .prepare("SELECT id, text, state, source, confidence, risk_level, metadata_json FROM claims WHERE id = ?1")
            .map_err(|e| AethelError::Storage(e.to_string()))?;

        let result = stmt
            .query_row(rusqlite::params![id.as_str()], |row| {
                Ok(RawClaim {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    state: row.get(2)?,
                    origin: row.get(3)?,
                    confidence: row.get(4)?,
                    risk: row.get(5)?,
                    evidence_json: row.get(6)?,
                })
            })
            .optional()
            .map_err(|e| AethelError::Storage(e.to_string()))?;

        match result {
            Some(raw) => Ok(Some(raw.into_claim()?)),
            None => Ok(None),
        }
    }

    /// List all claims with pagination.
    pub fn list_claims(&self, offset: usize, limit: usize) -> Result<Vec<Claim>, AethelError> {
        let conn = self.pool.conn();
        let mut stmt = conn
            .prepare("SELECT id, text, state, source, confidence, risk_level, metadata_json FROM claims ORDER BY id LIMIT ?1 OFFSET ?2")
            .map_err(|e| AethelError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map(rusqlite::params![limit as i64, offset as i64], |row| {
                Ok(RawClaim {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    state: row.get(2)?,
                    origin: row.get(3)?,
                    confidence: row.get(4)?,
                    risk: row.get(5)?,
                    evidence_json: row.get(6)?,
                })
            })
            .map_err(|e| AethelError::Storage(e.to_string()))?;

        let mut claims = Vec::new();
        for row in rows {
            let raw = row.map_err(|e| AethelError::Storage(e.to_string()))?;
            claims.push(raw.into_claim()?);
        }
        Ok(claims)
    }

    /// Delete a claim by ID. Returns true if a row was deleted.
    pub fn delete_claim(&self, id: &ClaimId) -> Result<bool, AethelError> {
        let conn = self.pool.conn();
        let count = conn
            .execute("DELETE FROM claims WHERE id = ?1", rusqlite::params![id.as_str()])
            .map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(count > 0)
    }

    /// Count all claims.
    pub fn count_claims(&self) -> Result<usize, AethelError> {
        let conn = self.pool.conn();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM claims", [], |row| row.get(0))
            .map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(count as usize)
    }

    /// Find claims by state.
    pub fn find_by_state(&self, state: ClaimState) -> Result<Vec<Claim>, AethelError> {
        let conn = self.pool.conn();
        let state_str = format!("{:?}", state);
        let mut stmt = conn
            .prepare("SELECT id, text, state, source, confidence, risk_level, metadata_json FROM claims WHERE state = ?1")
            .map_err(|e| AethelError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map(rusqlite::params![state_str], |row| {
                Ok(RawClaim {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    state: row.get(2)?,
                    origin: row.get(3)?,
                    confidence: row.get(4)?,
                    risk: row.get(5)?,
                    evidence_json: row.get(6)?,
                })
            })
            .map_err(|e| AethelError::Storage(e.to_string()))?;

        let mut claims = Vec::new();
        for row in rows {
            let raw = row.map_err(|e| AethelError::Storage(e.to_string()))?;
            claims.push(raw.into_claim()?);
        }
        Ok(claims)
    }

    /// Find claims by risk level.
    pub fn find_by_risk(&self, risk: RiskLevel) -> Result<Vec<Claim>, AethelError> {
        let conn = self.pool.conn();
        let risk_str = format!("{:?}", risk);
        let mut stmt = conn
            .prepare("SELECT id, text, state, source, confidence, risk_level, metadata_json FROM claims WHERE risk_level = ?1")
            .map_err(|e| AethelError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map(rusqlite::params![risk_str], |row| {
                Ok(RawClaim {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    state: row.get(2)?,
                    origin: row.get(3)?,
                    confidence: row.get(4)?,
                    risk: row.get(5)?,
                    evidence_json: row.get(6)?,
                })
            })
            .map_err(|e| AethelError::Storage(e.to_string()))?;

        let mut claims = Vec::new();
        for row in rows {
            let raw = row.map_err(|e| AethelError::Storage(e.to_string()))?;
            claims.push(raw.into_claim()?);
        }
        Ok(claims)
    }
}

// Helper for raw DB rows before parsing enums.
struct RawClaim {
    id: String,
    content: String,
    state: String,
    origin: String,
    confidence: f32,
    risk: String,
    evidence_json: String,
}

impl RawClaim {
    fn into_claim(self) -> Result<Claim, AethelError> {
        let state = parse_claim_state(&self.state)?;
        let origin = parse_claim_origin(&self.origin)?;
        let risk = parse_risk_level(&self.risk)?;
        let evidence_ids: Vec<String> = serde_json::from_str(&self.evidence_json)
            .unwrap_or_default();

        Ok(Claim {
            id: self.id,
            content: self.content,
            state,
            origin,
            support_level: SupportLevel::Unsupported, // stored claims reset to unsupported
            risk,
            confidence: self.confidence,
            evidence_ids,
            created_at_ms: 0,
            updated_at_ms: 0,
        })
    }
}

fn parse_claim_state(s: &str) -> Result<ClaimState, AethelError> {
    match s {
        "Generated" => Ok(ClaimState::Generated),
        "Supported" => Ok(ClaimState::Supported),
        "Accepted" => Ok(ClaimState::Accepted),
        "Deferred" => Ok(ClaimState::Deferred),
        "Escalated" => Ok(ClaimState::Escalated),
        "Revised" => Ok(ClaimState::Revised),
        "Rejected" => Ok(ClaimState::Rejected),
        "Retired" => Ok(ClaimState::Retired),
        _ => Err(AethelError::Storage(format!("Unknown ClaimState: {}", s))),
    }
}

fn parse_claim_origin(s: &str) -> Result<ClaimOrigin, AethelError> {
    match s {
        "ModelGenerated" => Ok(ClaimOrigin::ModelGenerated),
        "UserSupplied" => Ok(ClaimOrigin::UserSupplied),
        "Hybrid" => Ok(ClaimOrigin::Hybrid),
        "ExternalSource" => Ok(ClaimOrigin::ExternalSource),
        _ => Err(AethelError::Storage(format!("Unknown ClaimOrigin: {}", s))),
    }
}

fn parse_risk_level(s: &str) -> Result<RiskLevel, AethelError> {
    match s {
        "Low" => Ok(RiskLevel::Low),
        "Medium" => Ok(RiskLevel::Medium),
        "High" => Ok(RiskLevel::High),
        "Critical" => Ok(RiskLevel::Critical),
        _ => Err(AethelError::Storage(format!("Unknown RiskLevel: {}", s))),
    }
}

/// Extension trait for optional query results.
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db;

    fn make_claim(id: &str) -> Claim {
        Claim {
            id: id.to_string(),
            content: format!("Claim {}", id),
            state: ClaimState::Generated,
            origin: ClaimOrigin::ModelGenerated,
            support_level: SupportLevel::Unsupported,
            risk: RiskLevel::Low,
            confidence: 0.75,
            evidence_ids: vec!["ev1".to_string()],
            created_at_ms: 1000,
            updated_at_ms: 1000,
        }
    }

    #[test]
    fn test_save_and_load() {
        let db = test_db();
        let store = SqliteClaimStore::new(db);
        let claim = make_claim("c1");
        store.save_claim(&claim).unwrap();
        let loaded = store.load_claim(&ClaimId::new("c1")).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, "c1");
    }

    #[test]
    fn test_load_nonexistent() {
        let db = test_db();
        let store = SqliteClaimStore::new(db);
        let loaded = store.load_claim(&ClaimId::new("nope")).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_save_overwrites() {
        let db = test_db();
        let store = SqliteClaimStore::new(db);
        let mut claim = make_claim("c1");
        store.save_claim(&claim).unwrap();
        claim.confidence = 0.95;
        store.save_claim(&claim).unwrap();
        let loaded = store.load_claim(&ClaimId::new("c1")).unwrap().unwrap();
        assert!((loaded.confidence - 0.95).abs() < 0.01);
        assert_eq!(store.count_claims().unwrap(), 1);
    }

    #[test]
    fn test_delete() {
        let db = test_db();
        let store = SqliteClaimStore::new(db);
        store.save_claim(&make_claim("c1")).unwrap();
        assert!(store.delete_claim(&ClaimId::new("c1")).unwrap());
        assert_eq!(store.count_claims().unwrap(), 0);
    }

    #[test]
    fn test_list_pagination() {
        let db = test_db();
        let store = SqliteClaimStore::new(db);
        for i in 0..10 {
            store.save_claim(&make_claim(&format!("c{:02}", i))).unwrap();
        }
        let page1 = store.list_claims(0, 5).unwrap();
        assert_eq!(page1.len(), 5);
        let page2 = store.list_claims(5, 5).unwrap();
        assert_eq!(page2.len(), 5);
    }

    #[test]
    fn test_find_by_state() {
        let db = test_db();
        let store = SqliteClaimStore::new(db);
        store.save_claim(&make_claim("c1")).unwrap();

        let mut accepted = make_claim("c2");
        accepted.state = ClaimState::Accepted;
        store.save_claim(&accepted).unwrap();

        let generated = store.find_by_state(ClaimState::Generated).unwrap();
        assert_eq!(generated.len(), 1);
        assert_eq!(generated[0].id, "c1");
    }

    #[test]
    fn test_find_by_risk() {
        let db = test_db();
        let store = SqliteClaimStore::new(db);

        let mut high = make_claim("high-risk");
        high.risk = RiskLevel::High;
        store.save_claim(&high).unwrap();
        store.save_claim(&make_claim("low-risk")).unwrap();

        let results = store.find_by_risk(RiskLevel::High).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "high-risk");
    }
}
