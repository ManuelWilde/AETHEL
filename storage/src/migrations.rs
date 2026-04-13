//! Database migrations — creates and upgrades the schema.

use rusqlite::Connection;

/// Run all migrations to bring the database to the current schema version.
pub fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS claims (
            id TEXT PRIMARY KEY,
            mission_id TEXT NOT NULL,
            text TEXT NOT NULL,
            state TEXT NOT NULL DEFAULT 'Generated',
            source TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 0.0,
            risk_level TEXT NOT NULL DEFAULT 'Low',
            parent_claim_id TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            metadata_json TEXT NOT NULL DEFAULT '{}',
            FOREIGN KEY (parent_claim_id) REFERENCES claims(id)
        );

        CREATE INDEX IF NOT EXISTS idx_claims_mission ON claims(mission_id);
        CREATE INDEX IF NOT EXISTS idx_claims_state ON claims(state);
        CREATE INDEX IF NOT EXISTS idx_claims_risk ON claims(risk_level);

        CREATE TABLE IF NOT EXISTS traces (
            id TEXT PRIMARY KEY,
            mission_id TEXT NOT NULL,
            agent_id TEXT NOT NULL,
            action TEXT NOT NULL,
            detail TEXT NOT NULL DEFAULT '',
            timestamp TEXT NOT NULL DEFAULT (datetime('now')),
            parent_trace_id TEXT,
            duration_ms INTEGER,
            metadata_json TEXT NOT NULL DEFAULT '{}'
        );

        CREATE INDEX IF NOT EXISTS idx_traces_mission ON traces(mission_id);
        CREATE INDEX IF NOT EXISTS idx_traces_agent ON traces(agent_id);

        CREATE TABLE IF NOT EXISTS agent_reports (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id TEXT NOT NULL,
            mission_id TEXT NOT NULL,
            outcome TEXT NOT NULL,
            summary TEXT NOT NULL DEFAULT '',
            tokens_used INTEGER NOT NULL DEFAULT 0,
            cost_cents INTEGER NOT NULL DEFAULT 0,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            metadata_json TEXT NOT NULL DEFAULT '{}'
        );

        CREATE INDEX IF NOT EXISTS idx_reports_agent ON agent_reports(agent_id);
        CREATE INDEX IF NOT EXISTS idx_reports_mission ON agent_reports(mission_id);
        CREATE INDEX IF NOT EXISTS idx_reports_outcome ON agent_reports(outcome);

        CREATE TABLE IF NOT EXISTS audit_blocks (
            block_index INTEGER PRIMARY KEY,
            decision TEXT NOT NULL,
            risk_level TEXT NOT NULL,
            previous_hash TEXT NOT NULL,
            block_hash TEXT NOT NULL,
            timestamp TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY
        );

        INSERT OR IGNORE INTO schema_version (version) VALUES (1);
        "
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_run_cleanly() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        // Verify tables exist
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        // claims, traces, agent_reports, audit_blocks, schema_version = 5
        assert!(count >= 5, "Expected at least 5 tables, got {}", count);
    }

    #[test]
    fn test_migrations_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap(); // second run should not fail
    }
}
