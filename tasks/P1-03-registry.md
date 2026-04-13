# P1-03: Capability Registry

## Prerequisites

P1-02 must be merged to main.

## Context

You are working on the AETHEL project — a Rust workspace.
P1-01 added: Capability trait, CapValue, CapabilityDescriptor, CapabilityCategory.
P1-02 added: Pipeline.
Now we need a registry where capabilities are registered and discoverable.

## Git Branch

```bash
git checkout main && git pull
git checkout -b P1-03-registry
```

## Your Task

1. Create `contracts/src/registry.rs` with:
   - `CapabilityRegistry` struct (in-memory storage of registered capabilities)
   - `register()` — add a capability
   - `get()` — retrieve by ID
   - `find_by_category()` — filter by CapabilityCategory
   - `find_by_input_type()` — find capabilities that accept a given type
   - `find_by_output_type()` — find capabilities that produce a given type
   - `list_all()` — list all registered capability descriptors
   - `remove()` — unregister a capability
2. Add `pub mod registry; pub use registry::*;` to `contracts/src/lib.rs`

## Exact Code

### contracts/src/registry.rs:
```rust
//! Capability Registry — discovery and lookup of registered capabilities.
//!
//! Every capability in the AETHEL system is registered here.
//! The registry enables:
//! - Discovery by category, input type, output type
//! - Pipeline builder: find capabilities that can connect to each other
//! - UI: list all available capabilities for the operator
//!
//! # Thread Safety
//! The registry is `Send + Sync` and uses `Arc<dyn Capability>` for shared ownership.

use crate::{
    Capability, CapabilityCategory, CapabilityDescriptor, CapabilityId,
    AethelError,
};
use std::collections::HashMap;
use std::sync::Arc;

/// In-memory registry of all available capabilities.
pub struct CapabilityRegistry {
    capabilities: HashMap<CapabilityId, Arc<dyn Capability>>,
}

impl CapabilityRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            capabilities: HashMap::new(),
        }
    }

    /// Register a capability. Overwrites if ID already exists.
    pub fn register(&mut self, capability: Arc<dyn Capability>) {
        let id = capability.descriptor().id.clone();
        self.capabilities.insert(id, capability);
    }

    /// Get a capability by its ID.
    pub fn get(&self, id: &CapabilityId) -> Option<Arc<dyn Capability>> {
        self.capabilities.get(id).cloned()
    }

    /// Remove a capability by ID. Returns true if it existed.
    pub fn remove(&mut self, id: &CapabilityId) -> bool {
        self.capabilities.remove(id).is_some()
    }

    /// Number of registered capabilities.
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    /// Is the registry empty?
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// List all registered capability descriptors.
    pub fn list_all(&self) -> Vec<&CapabilityDescriptor> {
        self.capabilities.values().map(|c| c.descriptor()).collect()
    }

    /// Find capabilities by category.
    pub fn find_by_category(&self, category: CapabilityCategory) -> Vec<Arc<dyn Capability>> {
        self.capabilities
            .values()
            .filter(|c| c.descriptor().category == category)
            .cloned()
            .collect()
    }

    /// Find capabilities that accept a given input type name.
    pub fn find_by_input_type(&self, type_name: &str) -> Vec<Arc<dyn Capability>> {
        self.capabilities
            .values()
            .filter(|c| {
                let desc = c.descriptor();
                desc.input_type_name == type_name || desc.input_type_name == "Any"
            })
            .cloned()
            .collect()
    }

    /// Find capabilities that produce a given output type name.
    pub fn find_by_output_type(&self, type_name: &str) -> Vec<Arc<dyn Capability>> {
        self.capabilities
            .values()
            .filter(|c| {
                let desc = c.descriptor();
                desc.output_type_name == type_name || desc.output_type_name == "Any"
            })
            .cloned()
            .collect()
    }

    /// Find capabilities that can follow a given capability in a pipeline.
    /// i.e., capabilities whose input_type matches the given capability's output_type.
    pub fn find_connectable_after(&self, capability_id: &CapabilityId) -> Vec<Arc<dyn Capability>> {
        if let Some(cap) = self.get(capability_id) {
            let output_type = &cap.descriptor().output_type_name;
            self.find_by_input_type(output_type)
        } else {
            Vec::new()
        }
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CapValue, RiskLevel};

    struct MockCap {
        desc: CapabilityDescriptor,
    }

    impl MockCap {
        fn new(id: &str, name: &str, category: CapabilityCategory, input: &str, output: &str) -> Self {
            Self {
                desc: CapabilityDescriptor {
                    id: CapabilityId::new(id),
                    name: name.into(),
                    category,
                    input_type_name: input.into(),
                    output_type_name: output.into(),
                    estimated_cost_cents: 0.0,
                    estimated_latency_ms: 0,
                    risk_level: RiskLevel::Low,
                },
            }
        }
    }

    #[async_trait::async_trait]
    impl Capability for MockCap {
        fn descriptor(&self) -> &CapabilityDescriptor { &self.desc }
        fn accepts(&self, _input: &CapValue) -> bool { true }
        async fn execute(&self, input: CapValue) -> Result<CapValue, AethelError> {
            Ok(input)
        }
    }

    #[test]
    fn test_new_registry_is_empty() {
        let reg = CapabilityRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn test_register_and_get() {
        let mut reg = CapabilityRegistry::new();
        let cap = Arc::new(MockCap::new("cap1", "Cap1", CapabilityCategory::Processing, "Text", "Text"));
        reg.register(cap);
        assert_eq!(reg.len(), 1);
        let found = reg.get(&CapabilityId::new("cap1"));
        assert!(found.is_some());
        assert_eq!(found.unwrap().descriptor().name, "Cap1");
    }

    #[test]
    fn test_get_nonexistent() {
        let reg = CapabilityRegistry::new();
        assert!(reg.get(&CapabilityId::new("nope")).is_none());
    }

    #[test]
    fn test_register_overwrites() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(MockCap::new("cap1", "V1", CapabilityCategory::Processing, "Text", "Text")));
        reg.register(Arc::new(MockCap::new("cap1", "V2", CapabilityCategory::Processing, "Text", "Text")));
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.get(&CapabilityId::new("cap1")).unwrap().descriptor().name, "V2");
    }

    #[test]
    fn test_remove() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(MockCap::new("cap1", "Cap1", CapabilityCategory::Processing, "Text", "Text")));
        assert!(reg.remove(&CapabilityId::new("cap1")));
        assert!(reg.is_empty());
        assert!(!reg.remove(&CapabilityId::new("cap1"))); // already removed
    }

    #[test]
    fn test_find_by_category() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(MockCap::new("s1", "Sensor1", CapabilityCategory::Sensing, "Bytes", "BioSignal")));
        reg.register(Arc::new(MockCap::new("p1", "Proc1", CapabilityCategory::Processing, "Text", "Text")));
        reg.register(Arc::new(MockCap::new("p2", "Proc2", CapabilityCategory::Processing, "Text", "Claim")));
        let procs = reg.find_by_category(CapabilityCategory::Processing);
        assert_eq!(procs.len(), 2);
        let sensors = reg.find_by_category(CapabilityCategory::Sensing);
        assert_eq!(sensors.len(), 1);
        let acting = reg.find_by_category(CapabilityCategory::Acting);
        assert!(acting.is_empty());
    }

    #[test]
    fn test_find_by_input_type() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(MockCap::new("c1", "C1", CapabilityCategory::Processing, "Text", "Claim")));
        reg.register(Arc::new(MockCap::new("c2", "C2", CapabilityCategory::Processing, "Bytes", "Text")));
        reg.register(Arc::new(MockCap::new("c3", "C3", CapabilityCategory::Processing, "Any", "Text")));
        let text_caps = reg.find_by_input_type("Text");
        assert_eq!(text_caps.len(), 2); // c1 + c3 (Any matches)
    }

    #[test]
    fn test_find_by_output_type() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(MockCap::new("c1", "C1", CapabilityCategory::Processing, "Text", "Claim")));
        reg.register(Arc::new(MockCap::new("c2", "C2", CapabilityCategory::Processing, "Bytes", "Text")));
        let claim_prods = reg.find_by_output_type("Claim");
        assert_eq!(claim_prods.len(), 1);
    }

    #[test]
    fn test_find_connectable_after() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(MockCap::new("c1", "C1", CapabilityCategory::Processing, "Text", "Claim")));
        reg.register(Arc::new(MockCap::new("c2", "C2", CapabilityCategory::Reasoning, "Claim", "Verification")));
        reg.register(Arc::new(MockCap::new("c3", "C3", CapabilityCategory::Processing, "Text", "Text")));
        // After c1 (outputs Claim), c2 (accepts Claim) should be connectable
        let after_c1 = reg.find_connectable_after(&CapabilityId::new("c1"));
        assert_eq!(after_c1.len(), 1);
        assert_eq!(after_c1[0].descriptor().name, "C2");
    }

    #[test]
    fn test_find_connectable_after_nonexistent() {
        let reg = CapabilityRegistry::new();
        let result = reg.find_connectable_after(&CapabilityId::new("nope"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_list_all() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(MockCap::new("c1", "C1", CapabilityCategory::Processing, "Text", "Text")));
        reg.register(Arc::new(MockCap::new("c2", "C2", CapabilityCategory::Sensing, "Bytes", "BioSignal")));
        let all = reg.list_all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_default_is_empty() {
        let reg = CapabilityRegistry::default();
        assert!(reg.is_empty());
    }
}
```

### contracts/src/lib.rs — add module declaration:
```rust
pub mod registry;
pub use registry::*;
```

## Validation

```bash
cd contracts && cargo test --workspace 2>&1
```

Expected: All tests pass (P0 + P1-01 + P1-02 + P1-03), zero warnings.

## Done Criteria

- [ ] `contracts/src/registry.rs` exists
- [ ] `CapabilityRegistry` with register, get, remove, len, is_empty
- [ ] `find_by_category()` filters correctly
- [ ] `find_by_input_type()` and `find_by_output_type()` support "Any" wildcard
- [ ] `find_connectable_after()` finds capabilities that can follow a given one
- [ ] `list_all()` returns all descriptors
- [ ] 12+ tests pass
- [ ] All previous tests still pass

## Git

```bash
git add -A
git commit -m "P1-03: Capability Registry — discovery and lookup

- CapabilityRegistry: register, get, remove, find_by_category
- find_by_input_type/output_type with 'Any' wildcard support
- find_connectable_after for pipeline builder
- 12+ tests"
git push -u origin P1-03-registry
gh pr create --title "P1-03: Capability Registry" --body "$(cat tasks/P1-03-registry.md)"
```
