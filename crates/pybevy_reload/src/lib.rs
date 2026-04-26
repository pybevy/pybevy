pub mod cleanup;
pub mod orchestrator;
pub mod overlay;
pub mod profiling;
pub mod runtime;
pub mod state;
pub mod tracker;
pub mod util;

/// Marker component added to entities spawned by user code
/// Entities WITHOUT this marker are preserved during hot reload (Bevy internals)
#[derive(bevy::ecs::component::Component, Clone, Copy)]
pub struct HotReloadable;

use std::collections::HashSet;

use bevy::ecs::{entity::Entity, resource::Resource};

/// Entities that existed before any user code ran (plugin-init entities).
///
/// Captured once (before the first Full reload) and persists across reloads.
/// On Full reload, every entity NOT in this set is despawned — this catches
/// both user-spawned entities and Bevy-internal side-effect entities (e.g.,
/// `bevy_picking::PointerId` spawned per camera) that would otherwise leak.
#[derive(Resource)]
pub struct BaseEntitySet {
    pub entities: HashSet<Entity>,
}

pub use orchestrator::{HotReloadStateAccess, perform_reload};
pub use overlay::{
    MemoryOverlayVisible, StartPaused, render_hot_reload_overlay, spawn_hot_reload_overlay_system,
    update_system_stats,
};
pub use profiling::{
    HotReloadStats, MemoryProfile, ReloadMemorySnapshot, SystemMonitor, SystemProfiler, SystemStage,
};
pub use runtime::{ReloadError, ReloadRuntime};
pub use state::{HotReloadGeneration, ReloadMode, generation_matches, startup_or_reload};
pub use tracker::{KEEP_ALIVE_GENERATIONS, PluginTracker};
pub use util::{
    count_schedule_systems, get_current_rss_mb, is_verbose, lock_or_recover, parse_resolution,
};
