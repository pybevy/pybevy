use std::collections::HashSet;

use bevy::prelude::Resource;
use pybevy_core::PluginIdentity;

/// Resource tracking which plugins were installed when the live App started.
/// Reloads compare against this baseline because native plugin additions and
/// removals do not take effect until the App restarts.
#[derive(Resource, Default)]
pub struct PluginTracker {
    /// Set of plugin identities installed in the live App.
    pub known_plugins: HashSet<PluginIdentity>,
    /// Whether the initial app supplied a baseline, including an empty one.
    pub baseline_initialized: bool,
}

/// Number of old generations to keep alive (avoids gutting systems we might roll back to)
pub const KEEP_ALIVE_GENERATIONS: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_tracker_insert_and_diff() {
        let mut tracker = PluginTracker::default();
        tracker
            .known_plugins
            .insert(PluginIdentity::new("DefaultPlugins", None));
        tracker
            .known_plugins
            .insert(PluginIdentity::new("PhysicsPlugin", None));
        assert_eq!(tracker.known_plugins.len(), 2);

        // Simulate new reload with different plugins
        let mut new_plugins = HashSet::new();
        new_plugins.insert(PluginIdentity::new("DefaultPlugins", None));
        new_plugins.insert(PluginIdentity::new("AudioPlugin", None));

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

        assert_eq!(added, vec![PluginIdentity::new("AudioPlugin", None)]);
        assert_eq!(removed, vec![PluginIdentity::new("PhysicsPlugin", None)]);
    }
}
