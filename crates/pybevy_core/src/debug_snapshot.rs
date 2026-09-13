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
    /// Process-wide uptime, sourced from the operating system.
    pub uptime_secs: f64,
    /// Elapsed real time in the current Bevy app generation. This resets on a
    /// full reload when Bevy's `Time<Real>` resource is recreated.
    pub generation_uptime_secs: f64,

    pub entity_count: usize,
    /// Asset type name → count (e.g. "Mesh" → 9)
    pub asset_counts: Vec<(String, usize)>,

    pub gil_enabled: bool,

    /// Top update/last systems with rolling-window timing.
    pub update_profiles: Vec<SystemProfile>,
    /// Startup systems with rolling-window timing.
    pub startup_profiles: Vec<SystemProfile>,

    /// Total number of systems across all schedules (includes engine
    /// infrastructure and inert prior-generation "zombie" systems).
    pub total_schedule_systems: usize,
    /// Systems registered for the latest reload generation (the live scene
    /// systems). Stays flat across reloads while `total_schedule_systems` rises,
    /// which is expected generation accumulation, not a leak.
    pub current_generation_systems: usize,
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

/// Per-system timing record. `avg_ms` and `max_ms` are computed over the
/// same rolling window in `pybevy_reload::SystemProfiler` (default 60 frames).
/// `max_ms` lets consumers see spike behavior that the average smooths out.
#[derive(Clone, Default, Debug)]
pub struct SystemProfile {
    pub name: String,
    pub avg_ms: f64,
    pub max_ms: f64,
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
    /// Number of systems in all schedules (total, includes zombies)
    pub schedule_systems: usize,
    /// Systems registered for this generation (the live scene systems)
    pub current_generation_systems: usize,
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::{DebugSnapshot, ReloadMemorySnapshotInfo, SystemProfile};

    #[test]
    fn debug_snapshot_default_is_an_empty_unpopulated_snapshot() {
        let snap = DebugSnapshot::default();

        assert!(!snap.populated);
        assert_eq!(snap.reload_count, 0);
        assert_eq!(snap.last_reload_mode, None);
        assert!(!snap.reload_failed);
        assert_eq!(snap.reload_failure_reason, None);
        assert_eq!(snap.memory_mb, 0.0);
        assert_eq!(snap.total_memory_mb, 0.0);
        assert_eq!(snap.cpu_percent, 0.0);
        assert_eq!(snap.cpu_core_count, 0);
        assert_eq!(snap.fps_average, 0.0);
        assert_eq!(snap.fps_current, 0.0);
        assert_eq!(snap.uptime_secs, 0.0);
        assert_eq!(snap.generation_uptime_secs, 0.0);
        assert_eq!(snap.entity_count, 0);
        assert!(snap.asset_counts.is_empty());
        assert!(!snap.gil_enabled);
        assert!(snap.update_profiles.is_empty());
        assert!(snap.startup_profiles.is_empty());
        assert_eq!(snap.total_schedule_systems, 0);
        assert_eq!(snap.current_generation_systems, 0);
        assert_eq!(snap.python_gc_objects, 0);
        assert_eq!(snap.memory_growth_mb, 0.0);
        assert_eq!(snap.memory_peak_mb, 0.0);
        assert!(!snap.memory_warning);
        assert!(snap.reload_memory_snapshots.is_empty());
    }

    #[test]
    fn debug_snapshot_clone_is_fieldwise_independent() {
        let mut original = DebugSnapshot {
            populated: true,
            reload_count: 3,
            last_reload_mode: Some("full".to_owned()),
            memory_mb: 128.5,
            asset_counts: vec![("Mesh".to_owned(), 2)],
            update_profiles: vec![SystemProfile {
                name: "update".to_owned(),
                avg_ms: 1.0,
                max_ms: 2.0,
            }],
            ..DebugSnapshot::default()
        };
        let mut clone = original.clone();

        clone.reload_count = 4;
        clone.last_reload_mode = None;
        clone.asset_counts.push(("Image".to_owned(), 1));
        clone.update_profiles.push(SystemProfile::default());
        clone
            .reload_memory_snapshots
            .push(ReloadMemorySnapshotInfo {
                generation: 1,
                rss_mb: 10.0,
                delta_mb: 0.0,
                gc_objects: 5,
                schedule_systems: 1,
                current_generation_systems: 1,
            });

        assert_eq!(original.reload_count, 3);
        assert_eq!(original.last_reload_mode.as_deref(), Some("full"));
        assert_eq!(original.asset_counts.len(), 1);
        assert_eq!(original.update_profiles.len(), 1);
        assert!(original.reload_memory_snapshots.is_empty());

        original.memory_mb = 999.0;
        assert_eq!(
            clone.memory_mb, 128.5,
            "the clone must not alias the original's fields"
        );
    }

    #[test]
    fn system_profile_and_reload_memory_snapshot_defaults() {
        let profile = SystemProfile::default();
        assert_eq!(profile.name, "");
        assert_eq!(profile.avg_ms, 0.0);
        assert_eq!(profile.max_ms, 0.0);

        let snapshot = ReloadMemorySnapshotInfo::default();
        assert_eq!(snapshot.generation, 0);
        assert_eq!(snapshot.rss_mb, 0.0);
        assert_eq!(snapshot.delta_mb, 0.0);
        assert_eq!(snapshot.gc_objects, 0);
        assert_eq!(snapshot.schedule_systems, 0);
        assert_eq!(snapshot.current_generation_systems, 0);
    }
}
