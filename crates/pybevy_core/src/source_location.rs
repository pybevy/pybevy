//! Python source location tracking for entities and assets.
//!
//! When enabled, tracks where entities are spawned and assets are created
//! in Python source code. Useful for editor/debugging tools.
//!
//! Tracking is disabled by default. Enable via `SourceLocationConfig`.

use std::collections::HashMap;

use bevy::{asset::UntypedAssetId, prelude::*};

/// Python source location where an entity was spawned or asset was created.
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct SourceLocation {
    /// File path (absolute or module-relative)
    pub file: String,
    /// Line number (1-indexed)
    pub line: u32,
    /// Function/method name
    pub function: String,
}

impl SourceLocation {
    pub fn new(file: String, line: u32, function: String) -> Self {
        Self {
            file,
            line,
            function,
        }
    }
}

/// Configuration for source location tracking.
///
/// Disabled by default for zero overhead. Enable when editor mode is active
/// or when debugging tools need entity-to-source mapping.
#[derive(Resource, Clone)]
pub struct SourceLocationConfig {
    /// Master enable flag
    pub enabled: bool,
    /// Track entity spawns
    pub track_entities: bool,
    /// Track asset creation
    pub track_assets: bool,
}

impl Default for SourceLocationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            track_entities: true,
            track_assets: true,
        }
    }
}

impl SourceLocationConfig {
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }

    pub fn should_track_entities(&self) -> bool {
        self.enabled && self.track_entities
    }

    pub fn should_track_assets(&self) -> bool {
        self.enabled && self.track_assets
    }
}

/// Asset source locations (stored separately since assets don't have components).
///
/// Maps untyped asset IDs to the Python source location where they were created.
#[derive(Resource, Default)]
pub struct AssetSourceLocations {
    pub locations: HashMap<UntypedAssetId, SourceLocation>,
}

impl AssetSourceLocations {
    pub fn insert<A: Asset>(&mut self, handle: &Handle<A>, location: SourceLocation) {
        self.locations.insert(handle.id().untyped(), location);
    }

    pub fn get<A: Asset>(&self, handle: &Handle<A>) -> Option<&SourceLocation> {
        self.locations.get(&handle.id().untyped())
    }

    pub fn get_by_untyped(&self, id: UntypedAssetId) -> Option<&SourceLocation> {
        self.locations.get(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_is_disabled() {
        let config = SourceLocationConfig::default();
        assert!(!config.enabled);
        assert!(config.track_entities);
        assert!(config.track_assets);
        assert!(!config.should_track_entities());
        assert!(!config.should_track_assets());
    }

    #[test]
    fn config_enabled() {
        let config = SourceLocationConfig::enabled();
        assert!(config.enabled);
        assert!(config.should_track_entities());
        assert!(config.should_track_assets());
    }

    #[test]
    fn config_selective_tracking() {
        let config = SourceLocationConfig {
            enabled: true,
            track_entities: true,
            track_assets: false,
        };
        assert!(config.should_track_entities());
        assert!(!config.should_track_assets());
    }
}
