# P0-01: Add AethelError to contracts

## Context

You are working on the AETHEL project — a Rust workspace.
The file `contracts/src/lib.rs` contains 1037 lines of type definitions (structs, enums) but has NO error type.
Every future trait method needs to return `Result<T, AethelError>`, so we need this first.

## Your Task

1. Add `thiserror` and `async-trait` as dependencies to `contracts/Cargo.toml`
2. Create a new file `contracts/src/error.rs` with the `AethelError` enum
3. Add `pub mod error;` and `pub use error::AethelError;` to `contracts/src/lib.rs`

## Files to Modify

- `contracts/Cargo.toml` — add dependencies
- `contracts/src/error.rs` — NEW FILE
- `contracts/src/lib.rs` — add module declaration at top

## Exact Code

### contracts/Cargo.toml — add to [dependencies]:
```toml
thiserror = "1"
async-trait = "0.1"
```

### contracts/src/error.rs — FULL FILE:
```rust
//! Unified error type for the AETHEL ecosystem.
//!
//! Every trait method in contracts returns `Result<T, AethelError>`.
//! Error variants are grouped by domain.

use crate::ClaimState;
use std::fmt;

/// The unified error type for all AETHEL operations.
///
/// # Design Decisions
/// - Uses `thiserror` for automatic Display + Error impl
/// - Every variant has a human-readable message
/// - `Send + Sync` compatible (required for async traits)
/// - Recoverable errors use descriptive strings, not nested errors
#[derive(Debug, thiserror::Error)]
pub enum AethelError {
    // ── State Machine ──
    /// A claim transition was attempted that violates the state machine rules.
    /// Example: Generated → Accepted (must go through Supported first).
    #[error("Invalid state transition: {from:?} → {to:?}")]
    InvalidTransition { from: ClaimState, to: ClaimState },

    // ── Budget ──
    /// An operation would exceed the granted budget (tokens or cost).
    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),

    // ── Capability System ──
    /// A capability was requested by ID but not found in the registry.
    #[error("Capability not found: {0}")]
    CapabilityNotFound(String),

    /// Pipeline step output type does not match next step's input type.
    #[error("Type mismatch in pipeline: expected '{expected}', got '{got}'")]
    TypeMismatch { expected: String, got: String },

    /// A pipeline step failed during execution.
    #[error("Pipeline step {index} failed: {reason}")]
    PipelineStepFailed { index: usize, reason: String },

    // ── Governance ──
    /// An operation was attempted that is in the forbidden zones list.
    #[error("Forbidden operation: {0}")]
    ForbiddenOperation(String),

    /// Responsible Scaling Gate blocked execution.
    #[error("Scaling gate blocked: verification capacity ({capacity}) < requested depth ({depth})")]
    ScalingGateBlocked { capacity: u8, depth: u32 },

    // ── Storage ──
    /// A storage operation (save, load, query) failed.
    #[error("Storage error: {0}")]
    Storage(String),

    // ── Provider / LLM ──
    /// An LLM provider call failed.
    #[error("Provider error: {0}")]
    Provider(String),

    // ── Timeout ──
    /// An operation exceeded its time budget.
    #[error("Timeout after {ms}ms")]
    Timeout { ms: u64 },

    // ── I/O ──
    /// Standard I/O error (file, network, etc.).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // ── Serialization ──
    /// JSON serialization/deserialization failed.
    #[error("Serialization error: {0}")]
    Serialization(String),

    // ── Generic ──
    /// Catch-all for errors that don't fit other categories.
    /// Use sparingly — prefer specific variants.
    #[error("{0}")]
    Other(String),
}

// Verify Send + Sync at compile time (required for async traits)
const _: () = {
    fn assert_send_sync<T: Send + Sync>() {}
    fn check() { assert_send_sync::<AethelError>(); }
};
```

### contracts/src/lib.rs — add at the VERY TOP (after the `use` statements, before L0):
```rust
pub mod error;
pub use error::AethelError;
```

Find the line `use std::collections::HashMap;` (around line 14) and add the module declarations right after it.

## Tests to Write

Add at the bottom of `contracts/src/error.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_transition() {
        let e = AethelError::InvalidTransition {
            from: ClaimState::Generated,
            to: ClaimState::Accepted,
        };
        let msg = format!("{}", e);
        assert!(msg.contains("Generated"));
        assert!(msg.contains("Accepted"));
    }

    #[test]
    fn test_error_display_budget() {
        let e = AethelError::BudgetExceeded("tokens: 500 + 600 > 1000".into());
        assert!(e.to_string().contains("500"));
    }

    #[test]
    fn test_error_display_type_mismatch() {
        let e = AethelError::TypeMismatch {
            expected: "Text".into(),
            got: "Spectrum".into(),
        };
        assert!(e.to_string().contains("Text"));
        assert!(e.to_string().contains("Spectrum"));
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let e: AethelError = io_err.into();
        assert!(matches!(e, AethelError::Io(_)));
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AethelError>();
    }

    #[test]
    fn test_all_variants_have_display() {
        // Ensure every variant produces a non-empty string
        let errors = vec![
            AethelError::InvalidTransition { from: ClaimState::Generated, to: ClaimState::Retired },
            AethelError::BudgetExceeded("test".into()),
            AethelError::CapabilityNotFound("test".into()),
            AethelError::TypeMismatch { expected: "a".into(), got: "b".into() },
            AethelError::PipelineStepFailed { index: 0, reason: "test".into() },
            AethelError::ForbiddenOperation("test".into()),
            AethelError::ScalingGateBlocked { capacity: 3, depth: 5 },
            AethelError::Storage("test".into()),
            AethelError::Provider("test".into()),
            AethelError::Timeout { ms: 1000 },
            AethelError::Serialization("test".into()),
            AethelError::Other("test".into()),
        ];
        for e in &errors {
            assert!(!e.to_string().is_empty(), "Empty display for {:?}", e);
        }
    }
}
```

## Validation

```bash
cd contracts && cargo test 2>&1
```

Expected: All tests pass, zero warnings.

## Done Criteria

- [ ] `contracts/src/error.rs` exists with `AethelError` enum (12 variants)
- [ ] `thiserror` and `async-trait` in Cargo.toml dependencies
- [ ] `pub mod error; pub use error::AethelError;` in lib.rs
- [ ] 6 tests pass
- [ ] `AethelError` is Send + Sync (compile-time verified)
- [ ] Every variant has a human-readable Display message
