# P1-01: Define Capability Trait + CapValue

## Prerequisites

P0-01 through P0-04 must be merged to main.

## Context

You are working on the AETHEL project — a Rust workspace.
Phase 0 added: AethelError, newtype IDs, ClaimState transitions, BudgetLease enforcement.
Now we build the Capability system — the core of AETHEL.
"Everything is a Capability" means every function in the system implements one trait.

## Git Branch

```bash
git checkout main && git pull
git checkout -b P1-01-capability-trait
```

## Your Task

1. Create `contracts/src/capability.rs` with:
   - `CapabilityCategory` enum (6 variants)
   - `CapValue` enum (typed input/output for capabilities)
   - `CapabilityDescriptor` struct (metadata about a capability)
   - `Capability` trait (async, the central abstraction)
2. Add `pub mod capability; pub use capability::*;` to `contracts/src/lib.rs`

## Dependencies Required

Ensure `contracts/Cargo.toml` has:
```toml
async-trait = "0.1"  # should already be there from P0-01
```

## Exact Code

### contracts/src/capability.rs:
```rust
//! The Capability system — the heart of AETHEL.
//!
//! Every function, service, agent, ML model, and UI component in AETHEL
//! implements the `Capability` trait. This enables:
//! - **Composability**: Any capability can be input to any other (if types match)
//! - **Discoverability**: All capabilities are registered and queryable
//! - **Auditability**: Every capability execution is traced
//! - **Budgeting**: Every capability consumes from a BudgetLease
//!
//! # Design Decisions
//! - `CapValue` is an enum (not trait objects) for serialization + FFI compatibility
//! - `Capability` trait is async (LLM calls, voice processing are inherently async)
//! - `accepts()` enables runtime type checking for pipeline validation
//! - `descriptor()` returns metadata without executing the capability

use crate::{
    AethelError, CapabilityId, Claim, OmegaSpectrum24, BioSignal,
    RoutingDecision, VerificationResult, ThoughtEfficiency, RiskLevel,
};
use serde::{Deserialize, Serialize};

/// Categories of capabilities in the AETHEL system.
///
/// Used for filtering, discovery, and UI grouping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityCategory {
    /// Input capabilities: Bio sensors, Voice VAD, Vision input
    Sensing,
    /// Data transformation: NLP, Spectrum computation, Feature extraction
    Processing,
    /// Decision-making: Claim engine, Contemplation, Verification
    Reasoning,
    /// Execution: LLM inference, Agent spawning, TTS output
    Acting,
    /// Oversight: Audit logging, Risk assessment, Gate checking
    Governing,
    /// Output: UI rendering, Export, Visualization
    Presenting,
}

/// Typed input/output values for capabilities.
///
/// Every capability takes a `CapValue` and returns a `CapValue`.
/// This enum is the lingua franca of the pipeline system.
///
/// # Why an enum and not trait objects?
/// - Serializable (for persistence, FFI, and network transport)
/// - Exhaustive matching (compiler checks all cases)
/// - No heap allocation for small values
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CapValue {
    /// Plain text (prompts, responses, transcriptions)
    Text(String),
    /// A single claim with epistemic metadata
    Claim(Box<Claim>),
    /// Multiple claims (from ensemble/swarm)
    Claims(Vec<Claim>),
    /// 32-dimensional ontological vector
    Spectrum(OmegaSpectrum24),
    /// Operator biological state
    BioSignal(BioSignal),
    /// A routing decision from Triplex Via
    Routing(Box<RoutingDecision>),
    /// Result of claim verification
    Verification(Box<VerificationResult>),
    /// Efficiency metrics of a completed execution
    Efficiency(ThoughtEfficiency),
    /// Arbitrary JSON (for flexibility, use sparingly)
    Json(serde_json::Value),
    /// Raw bytes (audio, images, binary data)
    Bytes(Vec<u8>),
    /// No value (for capabilities that are side-effect only)
    Nothing,
}

impl CapValue {
    /// Returns the type name of this value. Used for pipeline type checking.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Text(_) => "Text",
            Self::Claim(_) => "Claim",
            Self::Claims(_) => "Claims",
            Self::Spectrum(_) => "Spectrum",
            Self::BioSignal(_) => "BioSignal",
            Self::Routing(_) => "Routing",
            Self::Verification(_) => "Verification",
            Self::Efficiency(_) => "Efficiency",
            Self::Json(_) => "Json",
            Self::Bytes(_) => "Bytes",
            Self::Nothing => "Nothing",
        }
    }

    /// Returns true if this value is Nothing.
    pub fn is_nothing(&self) -> bool {
        matches!(self, Self::Nothing)
    }

    /// Try to extract as Text. Returns None if not Text.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(t) => Some(t),
            _ => None,
        }
    }

    /// Try to extract as Claim. Returns None if not Claim.
    pub fn as_claim(&self) -> Option<&Claim> {
        match self {
            Self::Claim(c) => Some(c),
            _ => None,
        }
    }
}

/// Metadata describing a capability's identity, types, and cost.
///
/// Every capability has exactly one descriptor. It never changes at runtime.
/// Used for: registry lookup, pipeline validation, UI display, cost estimation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    /// Unique identifier
    pub id: CapabilityId,
    /// Human-readable name (e.g., "OpenAI GPT-4o Inference")
    pub name: String,
    /// Category for filtering and grouping
    pub category: CapabilityCategory,
    /// Type name of expected input (e.g., "Text", "Bytes", "Any")
    pub input_type_name: String,
    /// Type name of produced output (e.g., "Claim", "Text")
    pub output_type_name: String,
    /// Estimated cost per execution in cents
    pub estimated_cost_cents: f32,
    /// Estimated latency per execution in milliseconds
    pub estimated_latency_ms: u32,
    /// Risk level for EU AI Act compliance
    pub risk_level: RiskLevel,
}

/// The central trait of the AETHEL system.
///
/// Every function, service, agent, model, and UI component implements this.
/// The trait is async because most real capabilities involve I/O
/// (LLM calls, database queries, sensor reads).
///
/// # Implementing a new Capability
/// ```ignore
/// struct MyCapability { ... }
///
/// #[async_trait::async_trait]
/// impl Capability for MyCapability {
///     fn descriptor(&self) -> &CapabilityDescriptor { &self.desc }
///     fn accepts(&self, input: &CapValue) -> bool { matches!(input, CapValue::Text(_)) }
///     async fn execute(&self, input: CapValue) -> Result<CapValue, AethelError> {
///         let text = input.as_text().ok_or(AethelError::TypeMismatch { ... })?;
///         // ... do work ...
///         Ok(CapValue::Text(result))
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait Capability: Send + Sync {
    /// Returns the immutable descriptor of this capability.
    fn descriptor(&self) -> &CapabilityDescriptor;

    /// Returns true if this capability can process the given input.
    /// Used for pipeline validation before execution.
    fn accepts(&self, input: &CapValue) -> bool;

    /// Execute this capability with the given input.
    /// Returns the output value or an error.
    async fn execute(&self, input: CapValue) -> Result<CapValue, AethelError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClaimState;

    // ── CapValue tests ──

    #[test]
    fn test_cap_value_type_names_unique() {
        let values = vec![
            CapValue::Text("hello".into()),
            CapValue::Nothing,
            CapValue::Json(serde_json::Value::Null),
            CapValue::Bytes(vec![1, 2, 3]),
        ];
        let names: Vec<_> = values.iter().map(|v| v.type_name()).collect();
        // Check no duplicates
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names.len(), sorted.len(), "Duplicate type names found");
    }

    #[test]
    fn test_cap_value_as_text() {
        let v = CapValue::Text("hello".into());
        assert_eq!(v.as_text(), Some("hello"));
        assert!(v.as_claim().is_none());
    }

    #[test]
    fn test_cap_value_as_text_wrong_type() {
        let v = CapValue::Nothing;
        assert!(v.as_text().is_none());
    }

    #[test]
    fn test_cap_value_is_nothing() {
        assert!(CapValue::Nothing.is_nothing());
        assert!(!CapValue::Text("x".into()).is_nothing());
    }

    #[test]
    fn test_cap_value_serde_text() {
        let original = CapValue::Text("hello world".into());
        let json = serde_json::to_string(&original).unwrap();
        let restored: CapValue = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.type_name(), "Text");
        assert_eq!(restored.as_text(), Some("hello world"));
    }

    #[test]
    fn test_cap_value_serde_nothing() {
        let original = CapValue::Nothing;
        let json = serde_json::to_string(&original).unwrap();
        let restored: CapValue = serde_json::from_str(&json).unwrap();
        assert!(restored.is_nothing());
    }

    // ── CapabilityDescriptor tests ──

    #[test]
    fn test_descriptor_serde_roundtrip() {
        let desc = CapabilityDescriptor {
            id: CapabilityId::new("test-cap"),
            name: "Test Capability".into(),
            category: CapabilityCategory::Processing,
            input_type_name: "Text".into(),
            output_type_name: "Claim".into(),
            estimated_cost_cents: 0.5,
            estimated_latency_ms: 100,
            risk_level: RiskLevel::Low,
        };
        let json = serde_json::to_string(&desc).unwrap();
        let restored: CapabilityDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, desc.id);
        assert_eq!(restored.name, "Test Capability");
    }

    // ── CapabilityCategory tests ──

    #[test]
    fn test_category_serde() {
        let cat = CapabilityCategory::Reasoning;
        let json = serde_json::to_string(&cat).unwrap();
        let restored: CapabilityCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, CapabilityCategory::Reasoning);
    }

    // ── Mock Capability for trait testing ──

    struct MockCap {
        desc: CapabilityDescriptor,
    }

    impl MockCap {
        fn new() -> Self {
            Self {
                desc: CapabilityDescriptor {
                    id: CapabilityId::new("mock"),
                    name: "Mock".into(),
                    category: CapabilityCategory::Processing,
                    input_type_name: "Text".into(),
                    output_type_name: "Text".into(),
                    estimated_cost_cents: 0.0,
                    estimated_latency_ms: 0,
                    risk_level: RiskLevel::Low,
                },
            }
        }
    }

    #[async_trait::async_trait]
    impl Capability for MockCap {
        fn descriptor(&self) -> &CapabilityDescriptor {
            &self.desc
        }
        fn accepts(&self, input: &CapValue) -> bool {
            matches!(input, CapValue::Text(_))
        }
        async fn execute(&self, input: CapValue) -> Result<CapValue, AethelError> {
            match input {
                CapValue::Text(t) => Ok(CapValue::Text(format!("processed: {}", t))),
                _ => Err(AethelError::TypeMismatch {
                    expected: "Text".into(),
                    got: input.type_name().into(),
                }),
            }
        }
    }

    #[tokio::test]
    async fn test_mock_capability_execute() {
        let cap = MockCap::new();
        let result = cap.execute(CapValue::Text("hello".into())).await.unwrap();
        assert_eq!(result.as_text(), Some("processed: hello"));
    }

    #[tokio::test]
    async fn test_mock_capability_rejects_wrong_type() {
        let cap = MockCap::new();
        let result = cap.execute(CapValue::Nothing).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_capability_accepts() {
        let cap = MockCap::new();
        assert!(cap.accepts(&CapValue::Text("x".into())));
        assert!(!cap.accepts(&CapValue::Nothing));
    }

    #[test]
    fn test_capability_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockCap>();
    }
}
```

### contracts/Cargo.toml — add to [dev-dependencies]:
```toml
[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

### contracts/src/lib.rs — add module declaration:
```rust
pub mod capability;
pub use capability::*;
```

## Validation

```bash
cd contracts && cargo test --workspace 2>&1
```

Expected: All tests pass (P0 + P1-01), zero warnings.

## Done Criteria

- [ ] `contracts/src/capability.rs` exists
- [ ] `CapabilityCategory` enum with 6 variants
- [ ] `CapValue` enum with 11 variants + type_name() + as_text() + as_claim()
- [ ] `CapabilityDescriptor` struct with all fields
- [ ] `Capability` trait with descriptor(), accepts(), execute()
- [ ] MockCap passes all tests including async execute
- [ ] Capability: Send + Sync verified
- [ ] 12+ tests pass

## Git

```bash
git add -A
git commit -m "P1-01: Define Capability trait, CapValue enum, CapabilityDescriptor

- Capability trait: async execute(), accepts(), descriptor()
- CapValue: 11 typed variants (Text, Claim, Spectrum, Bio, etc.)
- CapabilityDescriptor: id, name, category, input/output types, cost, risk
- MockCap demonstrates trait implementation
- 12+ tests including async execution"
git push -u origin P1-01-capability-trait
gh pr create --title "P1-01: Define Capability trait" --body "$(cat tasks/P1-01-capability-trait.md)"
```
