//! Cross-crate debug/performance snapshot.
//!
//! Written by the hot reload overlay system in the main crate,
//! read by the MCP server to expose diagnostics to agents.

use bevy::prelude::Resource;

/// Snapshot of debug overlay data, updated ~4x/sec by the overlay system.
#[derive(Resource, Clone, Default)]
pub struct DebugSnapshot {
    /// Whether this snapshot has been populated at least once
    pub populated: bool,

    pub reload_count: u32,
    pub last_reload_mode: Option<String>,
    /// Whether the last reload attempt failed (app running previous generation)
    pub reload_failed: bool,
    /// Reason for reload failure, if any
    pub reload_failure_reason: Option<String>,

    pub memory_mb: f64,
    pub total_memory_mb: f64,
    pub cpu_percent: f32,
    pub cpu_core_count: usize,

    pub fps_average: f32,
    pub fps_current: f32,
    pub uptime_secs: f64,

    pub entity_count: usize,
    /// Asset type name → count (e.g. "Mesh" → 9)
    pub asset_counts: Vec<(String, usize)>,

    pub gil_enabled: bool,

    /// Top update/last systems: (name, avg_ms)
    pub update_profiles: Vec<(String, f64)>,
    /// Startup systems: (name, avg_ms)
    pub startup_profiles: Vec<(String, f64)>,

    /// Total number of systems across all schedules
    pub total_schedule_systems: usize,
    /// Python GC tracked objects (gen0 + gen1 + gen2)
    pub python_gc_objects: usize,
    /// Memory growth since baseline (MB)
    pub memory_growth_mb: f64,
    /// Peak memory observed (MB)
    pub memory_peak_mb: f64,
    /// Whether memory growth exceeds warning threshold
    pub memory_warning: bool,
    /// Per-reload memory snapshots (most recent last, capped at 20)
    pub reload_memory_snapshots: Vec<ReloadMemorySnapshotInfo>,
}

/// Per-reload memory snapshot exposed via DebugSnapshot for MCP diagnostics
#[derive(Clone, Default)]
pub struct ReloadMemorySnapshotInfo {
    /// Generation number
    pub generation: u32,
    /// RSS at time of reload (MB)
    pub rss_mb: f64,
    /// Delta from previous reload (MB)
    pub delta_mb: f64,
    /// Python GC tracked objects at reload time
    pub gc_objects: usize,
    /// Number of systems in all schedules
    pub schedule_systems: usize,
}
