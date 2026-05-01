pub mod cleanup;
pub mod orchestrator;
pub mod overlay;
pub mod profiling;
pub mod runtime;
pub mod state;
pub mod tracker;
pub mod util;

use std::collections::HashSet;

use bevy::ecs::{component::Component, entity::Entity, resource::Resource};

/// Marker for entities that must survive hot-reload cleanup.
///
/// Unlike `BaseEntitySet` (which captures plugin-init entities automatically),
/// `Retained` is explicitly added by systems that own long-lived entities
/// (editor camera, debug overlays, etc.).
///
/// Entities with `Retained` are skipped by `clear_world_state()` even if
/// they are not in the `BaseEntitySet`.
#[derive(Component, Clone, Copy)]
pub struct Retained;

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
