//! Runtime registries for plugin configuration
//!
use std::collections::HashMap;

use bevy::prelude::Resource;

/// Generic key-value config store for plugin discovery.
///
/// Any plugin can insert JSON config under a string key during `build()`.
/// The ControlPlugin exposes these via `GET /api/v1/config/{key}`.
#[derive(Resource, Default)]
pub struct PluginConfigs {
    entries: HashMap<String, serde_json::Value>,
}

impl PluginConfigs {
    pub fn insert(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.entries.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.entries.get(key)
    }

    pub fn all(&self) -> &HashMap<String, serde_json::Value> {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_configs() {
        let configs = PluginConfigs::default();
        assert!(configs.get("anything").is_none());
        assert!(configs.all().is_empty());
    }

    #[test]
    fn insert_and_get() {
        let mut configs = PluginConfigs::default();
        configs.insert("key", serde_json::json!({"enabled": true}));
        let val = configs.get("key").unwrap();
        assert_eq!(val["enabled"], true);
    }

    #[test]
    fn overwrite() {
        let mut configs = PluginConfigs::default();
        configs.insert("k", serde_json::json!(1));
        configs.insert("k", serde_json::json!(2));
        assert_eq!(configs.get("k").unwrap(), &serde_json::json!(2));
    }

    #[test]
    fn all_returns_everything() {
        let mut configs = PluginConfigs::default();
        configs.insert("a", serde_json::json!("x"));
        configs.insert("b", serde_json::json!("y"));
        assert_eq!(configs.all().len(), 2);
    }

    #[test]
    fn insert_accepts_into_string() {
        let mut configs = PluginConfigs::default();
        configs.insert(String::from("owned"), serde_json::json!(null));
        configs.insert("borrowed", serde_json::json!(null));
        assert_eq!(configs.all().len(), 2);
    }
}
