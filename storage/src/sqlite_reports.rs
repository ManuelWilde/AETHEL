//! SQLite-backed AgentReportStore implementation.

use aethel_contracts::{AgentId, AgentReport, AgentState, AethelError};
use crate::DbPool;

/// SQLite implementation of AgentReportStore.
pub struct SqliteReportStore {
    pool: DbPool,
}

impl SqliteReportStore {
    /// Create a new store backed by the given database pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Save an agent report.
    pub fn save_report(&self, report: &AgentReport) -> Result<(), AethelError> {
        let conn = self.pool.conn();
        let outcome = format!("{:?}", report.final_state);
        let summary = report
            .output
            .as_deref()
            .or(report.error.as_deref())
            .unwrap_or("");

        let json = serde_json::to_string(report)
            .map_err(|e| AethelError::Storage(format!("JSON serialize failed: {}", e)))?;

        conn.execute(
            "INSERT INTO agent_reports (agent_id, mission_id, outcome, summary, tokens_used, cost_cents, duration_ms, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                report.agent_id.as_str(),
                "", // mission_id not directly on AgentReport
                outcome,
                summary,
                report.tokens_consumed as i64,
                report.cost_consumed_cents as i64,
                report.duration_ms as i64,
                json,
            ],
        )
        .map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Load the latest report for an agent.
    pub fn load_report(&self, agent_id: &AgentId) -> Result<Option<AgentReport>, AethelError> {
        let conn = self.pool.conn();
        let result: Result<String, _> = conn.query_row(
            "SELECT metadata_json FROM agent_reports WHERE agent_id = ?1 ORDER BY created_at DESC LIMIT 1",
            rusqlite::params![agent_id.as_str()],
            |row| row.get(0),
        );

        match result {
            Ok(json) => {
                let report: AgentReport = serde_json::from_str(&json)
                    .map_err(|e| AethelError::Storage(format!("JSON deserialize failed: {}", e)))?;
                Ok(Some(report))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AethelError::Storage(e.to_string())),
        }
    }

    /// List all reports with pagination.
    pub fn list_reports(&self, offset: usize, limit: usize) -> Result<Vec<AgentReport>, AethelError> {
        let conn = self.pool.conn();
        let mut stmt = conn
            .prepare("SELECT metadata_json FROM agent_reports ORDER BY created_at DESC LIMIT ?1 OFFSET ?2")
            .map_err(|e| AethelError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map(rusqlite::params![limit as i64, offset as i64], |row| {
                let json: String = row.get(0)?;
                Ok(json)
            })
            .map_err(|e| AethelError::Storage(e.to_string()))?;

        let mut reports = Vec::new();
        for row in rows {
            let json = row.map_err(|e| AethelError::Storage(e.to_string()))?;
            let report: AgentReport = serde_json::from_str(&json)
                .map_err(|e| AethelError::Storage(format!("JSON deserialize failed: {}", e)))?;
            reports.push(report);
        }
        Ok(reports)
    }

    /// Count all reports.
    pub fn count_reports(&self) -> Result<usize, AethelError> {
        let conn = self.pool.conn();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM agent_reports", [], |row| row.get(0))
            .map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(count as usize)
    }

    /// Count reports by outcome.
    pub fn count_by_outcome(&self, state: AgentState) -> Result<usize, AethelError> {
        let conn = self.pool.conn();
        let outcome = format!("{:?}", state);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_reports WHERE outcome = ?1",
                rusqlite::params![outcome],
                |row| row.get(0),
            )
            .map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(count as usize)
    }

    /// Get total tokens consumed across all reports.
    pub fn total_tokens(&self) -> Result<u64, AethelError> {
        let conn = self.pool.conn();
        let total: i64 = conn
            .query_row("SELECT COALESCE(SUM(tokens_used), 0) FROM agent_reports", [], |row| row.get(0))
            .map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(total as u64)
    }

    /// Get total cost in cents across all reports.
    pub fn total_cost_cents(&self) -> Result<u64, AethelError> {
        let conn = self.pool.conn();
        let total: i64 = conn
            .query_row("SELECT COALESCE(SUM(cost_cents), 0) FROM agent_reports", [], |row| row.get(0))
            .map_err(|e| AethelError::Storage(e.to_string()))?;
        Ok(total as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_db;

    #[test]
    fn test_count_empty() {
        let db = test_db();
        let store = SqliteReportStore::new(db);
        assert_eq!(store.count_reports().unwrap(), 0);
    }

    #[test]
    fn test_total_tokens_empty() {
        let db = test_db();
        let store = SqliteReportStore::new(db);
        assert_eq!(store.total_tokens().unwrap(), 0);
    }

    #[test]
    fn test_load_nonexistent() {
        let db = test_db();
        let store = SqliteReportStore::new(db);
        let result = store.load_report(&AgentId::new("nope")).unwrap();
        assert!(result.is_none());
    }
}
