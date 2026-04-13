//! Capability Registry — discovery and lookup of registered capabilities.

use crate::{
    Capability, CapabilityCategory, CapabilityDescriptor, CapabilityId,
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

    /// Remove a capability by ID.
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
    use crate::{AethelError, CapValue, RiskLevel};

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
        reg.register(Arc::new(MockCap::new("cap1", "Cap1", CapabilityCategory::Processing, "Text", "Text")));
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
    }

    #[test]
    fn test_find_by_category() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(MockCap::new("s1", "Sensor1", CapabilityCategory::Sensing, "Bytes", "BioSignal")));
        reg.register(Arc::new(MockCap::new("p1", "Proc1", CapabilityCategory::Processing, "Text", "Text")));
        reg.register(Arc::new(MockCap::new("p2", "Proc2", CapabilityCategory::Processing, "Text", "Claim")));
        assert_eq!(reg.find_by_category(CapabilityCategory::Processing).len(), 2);
        assert_eq!(reg.find_by_category(CapabilityCategory::Sensing).len(), 1);
        assert!(reg.find_by_category(CapabilityCategory::Acting).is_empty());
    }

    #[test]
    fn test_find_by_input_type() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(MockCap::new("c1", "C1", CapabilityCategory::Processing, "Text", "Claim")));
        reg.register(Arc::new(MockCap::new("c2", "C2", CapabilityCategory::Processing, "Bytes", "Text")));
        reg.register(Arc::new(MockCap::new("c3", "C3", CapabilityCategory::Processing, "Any", "Text")));
        assert_eq!(reg.find_by_input_type("Text").len(), 2); // c1 + c3 (Any)
    }

    #[test]
    fn test_find_by_output_type() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(MockCap::new("c1", "C1", CapabilityCategory::Processing, "Text", "Claim")));
        reg.register(Arc::new(MockCap::new("c2", "C2", CapabilityCategory::Processing, "Bytes", "Text")));
        assert_eq!(reg.find_by_output_type("Claim").len(), 1);
    }

    #[test]
    fn test_find_connectable_after() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(MockCap::new("c1", "C1", CapabilityCategory::Processing, "Text", "Claim")));
        reg.register(Arc::new(MockCap::new("c2", "C2", CapabilityCategory::Reasoning, "Claim", "Verification")));
        reg.register(Arc::new(MockCap::new("c3", "C3", CapabilityCategory::Processing, "Text", "Text")));
        let after_c1 = reg.find_connectable_after(&CapabilityId::new("c1"));
        assert_eq!(after_c1.len(), 1);
        assert_eq!(after_c1[0].descriptor().name, "C2");
    }

    #[test]
    fn test_list_all() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(MockCap::new("c1", "C1", CapabilityCategory::Processing, "Text", "Text")));
        reg.register(Arc::new(MockCap::new("c2", "C2", CapabilityCategory::Sensing, "Bytes", "BioSignal")));
        assert_eq!(reg.list_all().len(), 2);
    }

    #[test]
    fn test_default_is_empty() {
        let reg = CapabilityRegistry::default();
        assert!(reg.is_empty());
    }
}
