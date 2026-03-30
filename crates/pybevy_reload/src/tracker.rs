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
