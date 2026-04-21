use bevy::prelude::Resource;

/// Resource tracking which plugins have been registered across reloads.
/// Used for delta detection: new plugins are reported, removed plugins
/// trigger a "restart required" warning.
#[derive(Resource, Default)]
pub struct PluginTracker {
    /// Set of plugin names that were present in the last successful reload
    pub known_plugins: std::collections::HashSet<String>,
}

/// Number of old generations to keep alive (avoids gutting systems we might roll back to)
pub const KEEP_ALIVE_GENERATIONS: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_tracker_default_empty() {
        let tracker = PluginTracker::default();
        assert!(tracker.known_plugins.is_empty());
    }

    #[test]
    fn test_plugin_tracker_insert_and_diff() {
        let mut tracker = PluginTracker::default();
        tracker.known_plugins.insert("DefaultPlugins".to_string());
        tracker.known_plugins.insert("PhysicsPlugin".to_string());
        assert_eq!(tracker.known_plugins.len(), 2);

        // Simulate new reload with different plugins
        let mut new_plugins = std::collections::HashSet::new();
        new_plugins.insert("DefaultPlugins".to_string());
        new_plugins.insert("AudioPlugin".to_string());

        // Compute delta
        let added: Vec<_> = new_plugins
            .difference(&tracker.known_plugins)
            .cloned()
            .collect();
        let removed: Vec<_> = tracker
            .known_plugins
            .difference(&new_plugins)
            .cloned()
            .collect();

        assert_eq!(added, vec!["AudioPlugin".to_string()]);
        assert_eq!(removed, vec!["PhysicsPlugin".to_string()]);
    }
}
