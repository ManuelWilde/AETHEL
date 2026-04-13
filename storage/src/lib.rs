//! # AETHEL Storage
//!
//! SQLite-backed persistence implementing the storage traits from `aethel_contracts`.
//! Provides durable storage for claims, traces, and agent reports with
//! full-text search, indexing, and transactional guarantees.

#![forbid(unsafe_code)]

pub mod sqlite_claims;
pub mod sqlite_traces;
pub mod sqlite_reports;
pub mod migrations;

pub use sqlite_claims::SqliteClaimStore;
pub use sqlite_traces::SqliteTraceStore;
pub use sqlite_reports::SqliteReportStore;
pub use migrations::run_migrations;

use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Shared SQLite connection wrapper.
/// Uses Mutex for thread-safe access (SQLite is single-writer).
#[derive(Clone)]
pub struct DbPool {
    conn: Arc<Mutex<Connection>>,
}

impl DbPool {
    /// Open or create a SQLite database at the given path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let pool = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        Ok(pool)
    }

    /// Create an in-memory database (for testing).
    pub fn in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let pool = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        Ok(pool)
    }

    /// Get a lock on the connection.
    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    /// Initialize all tables.
    pub fn initialize(&self) -> Result<(), rusqlite::Error> {
        run_migrations(&self.conn())
    }
}

/// Create a fully initialized in-memory database (convenience for tests).
pub fn test_db() -> DbPool {
    let pool = DbPool::in_memory().expect("Failed to create in-memory DB");
    pool.initialize().expect("Failed to run migrations");
    pool
}
