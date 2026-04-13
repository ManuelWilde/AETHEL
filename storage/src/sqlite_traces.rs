//! SQLite-backed TraceStore implementation.
//!
//! AethelTrace is complex with many nested types, so we serialize
//! the entire trace as JSON and store it alongside indexed metadata.

use aethel_contracts::{AethelError, AethelTrace, TraceId};
use crate::DbPool;

/// SQLite implementation of TraceStore.
pub struct SqliteTraceStore {
    pool: DbPool,
}

impl SqliteTraceStore {
    /// Create a new store backed by the given database pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Save a trace (insert or replace).
    /// Stores the full trace as JSON for fidelity, with indexed columns for querying.
    pub fn save_trace(&self, trace: &AethelTrace) -> Result<(), AethelError> {
        let conn = self.pool.conn();
        let json = serde_json::to_string(trace)
            .map_err(|e| AethelError::Storage(format!("JSON serialize failed: {}", e)))?;

        conn.execute(
            "INSERT OR REPLACE INTO traces (id, mission_id, agent_id, action, detail, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                trace.trace_id,
                trace.mission_id,
                "", // agent_id not on AethelTrace directly
                "trace",
                format!("{} claims, {} verifications", trace.claims.len(), trace.verifications.len()),
                json,
            ],
        )
        .map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Load a trace by ID.
    pub fn load_trace(&self, id: &TraceId) -> Result<Option<AethelTrace>, AethelError> {
        let conn = self.pool.conn();
        let result: Result<String, _> = conn.query_row(
            "SELECT metadata_json FROM traces WHERE id = ?1",
            rusqlite::params![id.as_str()],
            |row| row.get(0),
        );

        match result {
            Ok(json) => {
                let trace: AethelTrace = serde_json::from_str(&json)
                    .map_err(|e| AethelError::Storage(format!("JSON deserialize failed: {}", e)))?;
                Ok(Some(trace))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AethelError::Storage(e.to_string())),
        }
    }

    /// List traces for a mission.
    pub fn list_traces_for_mission(&self, mission_id: &str) -> Result<Vec<AethelTrace>, AethelError> {
        let conn = self.pool.conn();
        let mut stmt = conn
            .prepare("SELECT metadata_json FROM traces WHERE mission_id = ?1 ORDER BY timestamp")
            .map_err(|e| AethelError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map(rusqlite::params![mission_id], |row| {
                let json: String = row.get(0)?;
                Ok(json)
            })
            .map_err(|e| AethelError::Storage(e.to_string()))?;

        let mut traces = Vec::new();
        for row in rows {
            let json = row.map_err(|e| AethelError::Storage(e.to_string()))?;
            let trace: AethelTrace = serde_json::from_str(&json)
                .map_err(|e| AethelError::Storage(format!("JSON deserialize failed: {}", e)))?;
            traces.push(trace);
        }
        Ok(traces)
    }

    /// Count all traces.
    pub fn count_traces(&self) -> Result<usize, AethelError> {
        let conn = self.pool.conn();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM traces", [], |row| row.get(0))
            .map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(count as usize)
    }

    /// Delete a trace.
    pub fn delete_trace(&self, id: &TraceId) -> Result<bool, AethelError> {
        let conn = self.pool.conn();
        let count = conn
            .execute("DELETE FROM traces WHERE id = ?1", rusqlite::params![id.as_str()])
            .map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db;

    // Note: Creating a full AethelTrace requires many nested types.
    // These tests verify DB operations work. Full round-trip tests
    // require the types to implement Serialize/Deserialize properly
    // and are deferred to integration tests with the contracts crate.

    #[test]
    fn test_count_empty() {
        let db = test_db();
        let store = SqliteTraceStore::new(db);
        assert_eq!(store.count_traces().unwrap(), 0);
    }

    #[test]
    fn test_load_nonexistent() {
        let db = test_db();
        let store = SqliteTraceStore::new(db);
        let result = store.load_trace(&TraceId::new("nope")).unwrap();
        assert!(result.is_none());
    }
}
