use std::collections::HashSet;

use bevy::prelude::Resource;
use pybevy_core::PluginIdentity;

/// Resource tracking plugin declarations from the last committed generation.
#[derive(Resource, Default)]
pub struct PluginTracker {
    /// Set of plugin identities declared by the last committed generation.
    pub known_plugins: HashSet<PluginIdentity>,
    /// Whether a committed generation supplied a baseline, including an empty one.
    pub baseline_initialized: bool,
}

impl PluginTracker {
    /// Commit one generation's declarations and return its deterministic delta.
    pub fn commit(
        &mut self,
        new_plugins: HashSet<PluginIdentity>,
    ) -> (Vec<PluginIdentity>, Vec<PluginIdentity>) {
        if !self.baseline_initialized {
            self.known_plugins = new_plugins;
            self.baseline_initialized = true;
            return (Vec::new(), Vec::new());
        }

        let mut added: Vec<_> = new_plugins
            .difference(&self.known_plugins)
            .cloned()
            .collect();
        let mut removed: Vec<_> = self
            .known_plugins
            .difference(&new_plugins)
            .cloned()
            .collect();
        added.sort();
        removed.sort();
        self.known_plugins = new_plugins;
        (added, removed)
    }
}

/// Number of old generations to keep alive (avoids gutting systems we might roll back to)
pub const KEEP_ALIVE_GENERATIONS: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_returns_each_delta_once() {
        let mut tracker = PluginTracker::default();
        let initial = [
            PluginIdentity::new("DefaultPlugins", None),
            PluginIdentity::new("PhysicsPlugin", None),
        ]
        .into_iter()
        .collect();
        assert_eq!(tracker.commit(initial), (Vec::new(), Vec::new()));

        let changed: HashSet<PluginIdentity> = [
            PluginIdentity::new("AudioPlugin", None),
            PluginIdentity::new("DefaultPlugins", None),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            tracker.commit(changed.clone()),
            (
                vec![PluginIdentity::new("AudioPlugin", None)],
                vec![PluginIdentity::new("PhysicsPlugin", None)]
            )
        );
        assert_eq!(tracker.commit(changed), (Vec::new(), Vec::new()));
    }
}
