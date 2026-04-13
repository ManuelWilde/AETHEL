# P0-02: Add Newtype ID Wrappers

## Context

You are working on the AETHEL project — a Rust workspace.
`contracts/src/lib.rs` uses plain `String` for all IDs (claim IDs, mission IDs, etc.).
This means the compiler cannot catch when you accidentally pass a ClaimId where a MissionId is expected.
Task P0-01 has already been completed: `AethelError` exists in `contracts/src/error.rs`.

## Your Task

1. Create `contracts/src/ids.rs` with newtype wrappers for all ID types
2. Add `pub mod ids; pub use ids::*;` to `contracts/src/lib.rs`
3. Do NOT change existing types in lib.rs yet — just add the new module

## Files to Modify

- `contracts/src/ids.rs` — NEW FILE
- `contracts/src/lib.rs` — add module declaration

## Exact Code

### contracts/src/ids.rs — FULL FILE:
```rust
//! Type-safe ID wrappers for the AETHEL ecosystem.
//!
//! Every entity has its own ID type. The compiler prevents mixing them.
//! Example: `fn process(claim: ClaimId)` won't accept a `MissionId`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Macro to define a newtype ID wrapper.
/// Each ID is a String underneath, but the compiler treats them as distinct types.
macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            /// Create a new ID from any string-like value.
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            /// Get the inner string as a reference.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }
    };
}

define_id!(
    /// Unique identifier for a Claim in the verification system.
    ClaimId
);

define_id!(
    /// Unique identifier for a FIMAS mission (top-level task).
    MissionId
);

define_id!(
    /// Unique identifier for a BudgetLease granted by the control plane.
    LeaseId
);

define_id!(
    /// Unique identifier for an ExecutionBranch in the fractal topology.
    BranchId
);

define_id!(
    /// Unique identifier for an AethelTrace (full audit trail of a decision).
    TraceId
);

define_id!(
    /// Unique identifier for a registered Capability.
    CapabilityId
);

define_id!(
    /// Unique identifier for a Pipeline definition.
    PipelineId
);

define_id!(
    /// Unique identifier for a Twin projection (observation doppelganger).
    TwinId
);

define_id!(
    /// Unique identifier for an Agent instance.
    AgentId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_creation() {
        let id = ClaimId::new("claim-123");
        assert_eq!(id.as_str(), "claim-123");
    }

    #[test]
    fn test_id_display() {
        let id = MissionId::new("mission-456");
        assert_eq!(format!("{}", id), "mission-456");
    }

    #[test]
    fn test_id_equality() {
        let a = ClaimId::new("same");
        let b = ClaimId::new("same");
        assert_eq!(a, b);
    }

    #[test]
    fn test_id_inequality() {
        let a = ClaimId::new("one");
        let b = ClaimId::new("two");
        assert_ne!(a, b);
    }

    #[test]
    fn test_id_from_string() {
        let id: ClaimId = "test".into();
        assert_eq!(id.as_str(), "test");
    }

    #[test]
    fn test_id_from_owned_string() {
        let id: ClaimId = String::from("test").into();
        assert_eq!(id.as_str(), "test");
    }

    #[test]
    fn test_id_serde_roundtrip() {
        let original = ClaimId::new("claim-789");
        let json = serde_json::to_string(&original).unwrap();
        let restored: ClaimId = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_ids_are_not_interchangeable() {
        // This test documents the INTENT: different ID types should not be mixable.
        // The compiler enforces this — you cannot pass ClaimId where MissionId is expected.
        // We test this by verifying they are different types with different TypeIds.
        use std::any::TypeId;
        assert_ne!(TypeId::of::<ClaimId>(), TypeId::of::<MissionId>());
        assert_ne!(TypeId::of::<MissionId>(), TypeId::of::<LeaseId>());
        assert_ne!(TypeId::of::<LeaseId>(), TypeId::of::<BranchId>());
    }

    #[test]
    fn test_id_hash_works() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ClaimId::new("a"));
        set.insert(ClaimId::new("b"));
        set.insert(ClaimId::new("a")); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_id_clone() {
        let original = CapabilityId::new("cap-1");
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }
}
```

### contracts/src/lib.rs — add after the `pub mod error;` line (from P0-01):
```rust
pub mod ids;
pub use ids::*;
```

## Validation

```bash
cd contracts && cargo test 2>&1
```

Expected: All tests pass (P0-01 tests + P0-02 tests), zero warnings.

## Done Criteria

- [ ] `contracts/src/ids.rs` exists with 9 ID types
- [ ] Each ID type has: new(), as_str(), Display, From<String>, From<&str>, Serialize, Deserialize
- [ ] `pub mod ids; pub use ids::*;` in lib.rs
- [ ] 10 tests pass
- [ ] Different ID types have different TypeIds (compiler-enforced safety)
