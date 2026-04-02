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
