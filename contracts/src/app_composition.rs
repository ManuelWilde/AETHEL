//! App Composition — emergent applications from capability pipelines.
//!
//! Two modes:
//! - Composed: No-code, pipeline-builder. Users compose capabilities visually.
//! - Classic: Own code (Flutter/Web), but built on top of capabilities.

use crate::{
    AethelError, CapabilityId, PipelineId, RiskLevel,
};
use serde::{Deserialize, Serialize};

/// How an app is composed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppMode {
    /// No-code: app is a pipeline of capabilities.
    Composed,
    /// Own code: app uses capabilities as building blocks.
    Classic,
}

/// A composable app definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppDefinition {
    /// Unique app ID.
    pub app_id: String,
    /// Human-readable name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Mode.
    pub mode: AppMode,
    /// Pipeline ID (for Composed mode).
    pub pipeline_id: Option<PipelineId>,
    /// Required capabilities (for Classic mode).
    pub required_capabilities: Vec<CapabilityId>,
    /// App version.
    pub version: String,
    /// Author.
    pub author: String,
    /// Risk level.
    pub risk_level: RiskLevel,
    /// Tags for discovery.
    pub tags: Vec<String>,
}

impl AppDefinition {
    /// Validate the app definition.
    pub fn validate(&self) -> Result<(), AethelError> {
        if self.name.is_empty() {
            return Err(AethelError::Other("App name cannot be empty".into()));
        }
        match self.mode {
            AppMode::Composed => {
                if self.pipeline_id.is_none() {
                    return Err(AethelError::Other("Composed app must have a pipeline_id".into()));
                }
            }
            AppMode::Classic => {
                if self.required_capabilities.is_empty() {
                    return Err(AethelError::Other("Classic app must have at least one required capability".into()));
                }
            }
        }
        Ok(())
    }
}

/// App registry — stores and discovers apps.
pub struct AppRegistry {
    apps: Vec<AppDefinition>,
}

impl AppRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self { apps: Vec::new() }
    }

    /// Register an app.
    pub fn register(&mut self, app: AppDefinition) -> Result<(), AethelError> {
        app.validate()?;
        // Remove existing with same ID
        self.apps.retain(|a| a.app_id != app.app_id);
        self.apps.push(app);
        Ok(())
    }

    /// Get an app by ID.
    pub fn get(&self, app_id: &str) -> Option<&AppDefinition> {
        self.apps.iter().find(|a| a.app_id == app_id)
    }

    /// Find apps by tag.
    pub fn find_by_tag(&self, tag: &str) -> Vec<&AppDefinition> {
        self.apps.iter().filter(|a| a.tags.iter().any(|t| t == tag)).collect()
    }

    /// Find apps by mode.
    pub fn find_by_mode(&self, mode: AppMode) -> Vec<&AppDefinition> {
        self.apps.iter().filter(|a| a.mode == mode).collect()
    }

    /// List all apps.
    pub fn list_all(&self) -> &[AppDefinition] {
        &self.apps
    }

    /// Number of registered apps.
    pub fn len(&self) -> usize {
        self.apps.len()
    }

    /// Is the registry empty?
    pub fn is_empty(&self) -> bool {
        self.apps.is_empty()
    }
}

impl Default for AppRegistry {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn composed_app(id: &str) -> AppDefinition {
        AppDefinition {
            app_id: id.into(),
            name: format!("App {}", id),
            description: "Test app".into(),
            mode: AppMode::Composed,
            pipeline_id: Some(PipelineId::new("pipeline-1")),
            required_capabilities: vec![],
            version: "1.0".into(),
            author: "test".into(),
            risk_level: RiskLevel::Low,
            tags: vec!["test".into()],
        }
    }

    fn classic_app(id: &str) -> AppDefinition {
        AppDefinition {
            app_id: id.into(),
            name: format!("Classic {}", id),
            description: "Classic test app".into(),
            mode: AppMode::Classic,
            pipeline_id: None,
            required_capabilities: vec![CapabilityId::new("cap-1")],
            version: "1.0".into(),
            author: "test".into(),
            risk_level: RiskLevel::Medium,
            tags: vec!["classic".into()],
        }
    }

    #[test]
    fn test_composed_app_valid() {
        assert!(composed_app("a1").validate().is_ok());
    }

    #[test]
    fn test_composed_app_no_pipeline_invalid() {
        let mut app = composed_app("a1");
        app.pipeline_id = None;
        assert!(app.validate().is_err());
    }

    #[test]
    fn test_classic_app_valid() {
        assert!(classic_app("a1").validate().is_ok());
    }

    #[test]
    fn test_classic_app_no_capabilities_invalid() {
        let mut app = classic_app("a1");
        app.required_capabilities = vec![];
        assert!(app.validate().is_err());
    }

    #[test]
    fn test_empty_name_invalid() {
        let mut app = composed_app("a1");
        app.name = String::new();
        assert!(app.validate().is_err());
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut reg = AppRegistry::new();
        reg.register(composed_app("a1")).unwrap();
        assert_eq!(reg.len(), 1);
        assert!(reg.get("a1").is_some());
    }

    #[test]
    fn test_registry_overwrites() {
        let mut reg = AppRegistry::new();
        reg.register(composed_app("a1")).unwrap();
        reg.register(composed_app("a1")).unwrap();
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn test_find_by_tag() {
        let mut reg = AppRegistry::new();
        reg.register(composed_app("a1")).unwrap();
        reg.register(classic_app("a2")).unwrap();
        assert_eq!(reg.find_by_tag("test").len(), 1);
        assert_eq!(reg.find_by_tag("classic").len(), 1);
        assert!(reg.find_by_tag("nonexistent").is_empty());
    }

    #[test]
    fn test_find_by_mode() {
        let mut reg = AppRegistry::new();
        reg.register(composed_app("a1")).unwrap();
        reg.register(classic_app("a2")).unwrap();
        assert_eq!(reg.find_by_mode(AppMode::Composed).len(), 1);
        assert_eq!(reg.find_by_mode(AppMode::Classic).len(), 1);
    }

    #[test]
    fn test_registry_rejects_invalid() {
        let mut reg = AppRegistry::new();
        let mut app = composed_app("a1");
        app.pipeline_id = None;
        assert!(reg.register(app).is_err());
        assert!(reg.is_empty());
    }

    #[test]
    fn test_app_serde() {
        let app = composed_app("a1");
        let json = serde_json::to_string(&app).unwrap();
        let restored: AppDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.app_id, "a1");
        assert_eq!(restored.mode, AppMode::Composed);
    }
}
