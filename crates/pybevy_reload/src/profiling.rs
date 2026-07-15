use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use bevy::{ecs::world::World, prelude::Resource, time::Time};
use pybevy_ecs::shared::system_runtime::RunProfileSink;
pub use pybevy_ecs::shared::system_runtime::SystemStage;

use crate::{state::ReloadMode, util::lock_or_recover};

/// Statistics about hot reload events for the overlay
#[derive(Resource, Clone)]
pub struct HotReloadStats {
    /// Last reload mode used
    pub last_mode: Option<ReloadMode>,
    /// Timestamp of last reload (in seconds since app start)
    pub last_reload_time: f64,
    /// Total number of reloads
    pub reload_count: u32,
    /// Default reload mode for file changes (toggled with F6)
    pub default_mode: ReloadMode,
    /// Current memory usage in MB
    pub memory_mb: f64,
    /// Current CPU usage percentage
    pub cpu_percent: f32,
    /// FPS rolling average (60 frames)
    pub fps_average: f32,
    /// Current frame FPS
    pub fps_current: f32,
    /// Total system RAM in MB
    pub total_memory_mb: f64,
    /// Number of CPU cores
    pub cpu_core_count: usize,
    /// Python GIL enabled status
    pub gil_enabled: bool,
    /// Total app uptime in seconds
    pub uptime_secs: f64,
    /// Total number of entities
    pub entity_count: usize,
    /// Asset counts by type
    pub asset_counts: HashMap<String, usize>,
    /// Timestamp of last error displayed in overlay (to detect new errors)
    pub last_error_timestamp: f64,
    /// Frame number of last reload (for frame-based cooldown)
    pub last_reload_frame: u32,
}

impl Default for HotReloadStats {
    fn default() -> Self {
        Self {
            last_mode: None,
            last_reload_time: 0.0,
            reload_count: 0,
            default_mode: ReloadMode::Full,
            memory_mb: 0.0,
            cpu_percent: 0.0,
            fps_average: 0.0,
            fps_current: 0.0,
            total_memory_mb: 0.0,
            cpu_core_count: 1,
            gil_enabled: false,
            uptime_secs: 0.0,
            entity_count: 0,
            asset_counts: HashMap::new(),
            last_error_timestamp: 0.0,
            last_reload_frame: 0,
        }
    }
}

/// Resource for tracking system information
#[derive(Resource)]
pub struct SystemMonitor {
    pub system: sysinfo::System,
    /// Process ID for monitoring, None if PID could not be determined
    pub process_pid: Option<sysinfo::Pid>,
    pub last_update: f64,
    /// Last 60 FPS values for rolling average
    pub fps_history: VecDeque<f32>,
    /// Last time the overlay text was updated (for throttling)
    pub last_render_update: f64,
}

/// Performance profiling statistics for dynamic systems
/// Only active when hot reload is enabled
/// Uses interior mutability for concurrent access from multiple systems
#[derive(Resource, Clone)]
pub struct SystemProfiler {
    /// Per-system timing statistics (wrapped in Arc<Mutex> for concurrent access)
    stats: Arc<Mutex<ProfilerData>>,
    /// Rolling window size (frames to average)
    window_size: usize,
}

struct ProfilerData {
    /// Update/Last stage systems
    update_systems: HashMap<String, SystemTimingStats>,
    /// Startup stage systems
    startup_systems: HashMap<String, SystemTimingStats>,
    /// Time when startup systems should stop being displayed (5 seconds after first startup)
    startup_visible_until: Option<f64>,
}

struct SystemTimingStats {
    /// Circular buffer of recent execution times
    recent_times: VecDeque<Duration>,
    /// Cached rolling average (updated each frame)
    average_time: Duration,
    /// Cached rolling max over the same window — preserves spike visibility
    /// that the average smooths away.
    max_time: Duration,
}

impl SystemProfiler {
    /// Create a new profiler with specified window size
    pub fn new(window_size: usize) -> Self {
        Self {
            stats: Arc::new(Mutex::new(ProfilerData {
                update_systems: HashMap::new(),
                startup_systems: HashMap::new(),
                startup_visible_until: None,
            })),
            window_size,
        }
    }

    /// Record a timing measurement for a system (concurrent-safe)
    pub fn record_timing(
        &self,
        system_name: &str,
        duration: Duration,
        stage: SystemStage,
        current_time: f64,
    ) {
        let mut data = lock_or_recover(&self.stats);

        // Set startup visibility timer on first startup system
        if stage == SystemStage::Startup && data.startup_visible_until.is_none() {
            data.startup_visible_until = Some(current_time + 5.0);
        }

        let systems = match stage {
            SystemStage::Startup => &mut data.startup_systems,
            SystemStage::UpdateOrLast => &mut data.update_systems,
        };

        let entry = systems
            .entry(system_name.to_string())
            .or_insert_with(|| SystemTimingStats {
                recent_times: VecDeque::with_capacity(self.window_size),
                average_time: Duration::ZERO,
                max_time: Duration::ZERO,
            });

        // Add new timing to circular buffer
        entry.recent_times.push_back(duration);

        // Remove oldest if we exceed window size
        if entry.recent_times.len() > self.window_size {
            entry.recent_times.pop_front();
        }

        // Recalculate rolling average + max. Window is bounded (default 60),
        // so a full scan per record is cheap.
        if !entry.recent_times.is_empty() {
            let sum: Duration = entry.recent_times.iter().sum();
            entry.average_time = sum / entry.recent_times.len() as u32;
            entry.max_time = entry
                .recent_times
                .iter()
                .copied()
                .max()
                .unwrap_or(Duration::ZERO);
        }
    }

    /// Record one system run's duration into the profiler resource if the world
    /// has one, reading the current time from `Time`. Both backends' `run_unsafe`
    /// epilogue call this so the profiler read, `Time` read, and timing write stay
    /// one implementation and cannot drift between interpreter adapters.
    pub fn record_run(world: &World, system_name: &str, duration: Duration, stage: SystemStage) {
        if let Some(profiler) = world.get_resource::<SystemProfiler>() {
            let current_time = world
                .get_resource::<Time>()
                .map(|t| t.elapsed_secs_f64())
                .unwrap_or(0.0);
            profiler.record_timing(system_name, duration, stage, current_time);
        }
    }

    /// Get the top N Update/Last systems by average execution time (concurrent-safe).
    /// Returns `(name, avg, max)` per entry, both timings over the same rolling window.
    pub fn get_top_n_update(&self, n: usize) -> Vec<(String, Duration, Duration)> {
        let data = lock_or_recover(&self.stats);
        let mut systems: Vec<_> = data
            .update_systems
            .iter()
            .map(|(name, stats)| (name.clone(), stats.average_time, stats.max_time))
            .collect();

        // Sort by average time (descending)
        systems.sort_by_key(|x| std::cmp::Reverse(x.1));

        // Take top N
        systems.into_iter().take(n).collect()
    }

    /// Get the top N Startup systems by average execution time (concurrent-safe).
    /// Returns `(name, avg, max)` per entry.
    pub fn get_top_n_startup(&self, n: usize) -> Vec<(String, Duration, Duration)> {
        let data = lock_or_recover(&self.stats);
        let mut systems: Vec<_> = data
            .startup_systems
            .iter()
            .map(|(name, stats)| (name.clone(), stats.average_time, stats.max_time))
            .collect();

        // Sort by average time (descending)
        systems.sort_by_key(|x| std::cmp::Reverse(x.1));

        // Take top N
        systems.into_iter().take(n).collect()
    }

    /// Check if startup systems should still be displayed
    pub fn should_show_startup(&self, current_time: f64) -> bool {
        let data = lock_or_recover(&self.stats);
        data.startup_visible_until
            .map(|until| current_time < until)
            .unwrap_or(false)
    }

    /// Clear all profiling statistics (concurrent-safe)
    pub fn clear(&self) {
        let mut data = lock_or_recover(&self.stats);
        data.update_systems.clear();
        data.startup_systems.clear();
        data.startup_visible_until = None;
    }
}

impl RunProfileSink for SystemProfiler {
    fn record(
        &self,
        system_name: &str,
        duration: Duration,
        stage: SystemStage,
        app_time_seconds: f64,
    ) {
        self.record_timing(system_name, duration, stage, app_time_seconds);
    }
}

/// Memory snapshot captured at each reload event
#[derive(Clone)]
pub struct ReloadMemorySnapshot {
    /// Generation number at time of snapshot
    pub generation: u32,
    /// Process RSS in MB
    pub rss_mb: f64,
    /// Delta from previous snapshot (MB)
    pub delta_mb: f64,
    /// Python GC tracked objects (sum of gc.get_count())
    pub gc_objects: usize,
    /// Total systems across all schedules (includes engine infrastructure and
    /// inert prior-generation "zombie" systems that no longer run).
    pub schedule_systems: usize,
    /// Systems registered for this reload's generation (the live scene systems).
    /// `schedule_systems - current_generation_systems` is infrastructure plus
    /// gated-off prior generations, so a rising `schedule_systems` across reloads
    /// while this stays flat is expected accumulation, not a leak.
    pub current_generation_systems: usize,
}

/// Resource tracking memory across reloads (rolling window of snapshots).
/// Captures a snapshot at each reload for trend analysis.
#[derive(Resource)]
pub struct MemoryProfile {
    /// Rolling window of reload snapshots (capped at MAX_SNAPSHOTS)
    pub snapshots: Vec<ReloadMemorySnapshot>,
    /// Baseline RSS captured after first Startup (MB)
    pub baseline_rss_mb: f64,
    /// Peak RSS observed (MB)
    pub peak_rss_mb: f64,
    /// Whether baseline has been captured
    pub baseline_captured: bool,
    /// Warning threshold: growth above baseline that triggers warning (MB)
    pub warning_threshold_mb: f64,
}

impl Default for MemoryProfile {
    fn default() -> Self {
        Self {
            snapshots: Vec::with_capacity(Self::MAX_SNAPSHOTS),
            baseline_rss_mb: 0.0,
            peak_rss_mb: 0.0,
            baseline_captured: false,
            warning_threshold_mb: 200.0, // Warn after 200MB growth above baseline
        }
    }
}

impl MemoryProfile {
    pub const MAX_SNAPSHOTS: usize = 20;

    /// Capture a snapshot at reload time
    pub fn capture_snapshot(
        &mut self,
        generation: u32,
        rss_mb: f64,
        gc_objects: usize,
        schedule_systems: usize,
        current_generation_systems: usize,
    ) {
        let delta_mb = self
            .snapshots
            .last()
            .map(|prev| rss_mb - prev.rss_mb)
            .unwrap_or(0.0);

        if rss_mb > self.peak_rss_mb {
            self.peak_rss_mb = rss_mb;
        }

        let snapshot = ReloadMemorySnapshot {
            generation,
            rss_mb,
            delta_mb,
            gc_objects,
            schedule_systems,
            current_generation_systems,
        };

        if self.snapshots.len() >= Self::MAX_SNAPSHOTS {
            self.snapshots.remove(0);
        }
        self.snapshots.push(snapshot);
    }

    /// Capture baseline RSS after first Startup
    pub fn capture_baseline(&mut self, rss_mb: f64) {
        if !self.baseline_captured {
            self.baseline_rss_mb = rss_mb;
            self.peak_rss_mb = rss_mb;
            self.baseline_captured = true;
        }
    }

    /// Check if memory growth exceeds warning threshold
    pub fn is_warning(&self, current_rss_mb: f64) -> bool {
        self.baseline_captured
            && (current_rss_mb - self.baseline_rss_mb) > self.warning_threshold_mb
    }

    /// Get memory growth since baseline
    pub fn growth_mb(&self, current_rss_mb: f64) -> f64 {
        if self.baseline_captured {
            current_rss_mb - self.baseline_rss_mb
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_system_profiler_new() {
        let profiler = SystemProfiler::new(60);
        assert_eq!(profiler.get_top_n_update(10).len(), 0);
        assert_eq!(profiler.get_top_n_startup(10).len(), 0);
    }

    #[test]
    fn test_system_profiler_record_and_get_top() {
        let profiler = SystemProfiler::new(60);

        profiler.record_timing(
            "fast_system",
            Duration::from_micros(100),
            SystemStage::UpdateOrLast,
            0.0,
        );
        profiler.record_timing(
            "slow_system",
            Duration::from_millis(5),
            SystemStage::UpdateOrLast,
            0.0,
        );

        let top = profiler.get_top_n_update(10);
        assert_eq!(top.len(), 2);
        // Sorted descending by time
        assert_eq!(top[0].0, "slow_system");
        assert_eq!(top[1].0, "fast_system");
    }

    #[test]
    fn test_system_profiler_startup_vs_update() {
        let profiler = SystemProfiler::new(60);

        profiler.record_timing(
            "startup_sys",
            Duration::from_millis(10),
            SystemStage::Startup,
            1.0,
        );
        profiler.record_timing(
            "update_sys",
            Duration::from_millis(5),
            SystemStage::UpdateOrLast,
            1.0,
        );

        assert_eq!(profiler.get_top_n_startup(10).len(), 1);
        assert_eq!(profiler.get_top_n_update(10).len(), 1);
        assert_eq!(profiler.get_top_n_startup(10)[0].0, "startup_sys");
        assert_eq!(profiler.get_top_n_update(10)[0].0, "update_sys");
    }

    #[test]
    fn test_system_profiler_rolling_average() {
        let profiler = SystemProfiler::new(3);

        profiler.record_timing(
            "sys",
            Duration::from_millis(10),
            SystemStage::UpdateOrLast,
            0.0,
        );
        profiler.record_timing(
            "sys",
            Duration::from_millis(20),
            SystemStage::UpdateOrLast,
            0.0,
        );
        profiler.record_timing(
            "sys",
            Duration::from_millis(30),
            SystemStage::UpdateOrLast,
            0.0,
        );

        let top = profiler.get_top_n_update(1);
        // Average of 10, 20, 30 = 20ms
        assert_eq!(top[0].1, Duration::from_millis(20));

        // Add another - should evict the first (10ms), average now (20+30+40)/3 = 30ms
        profiler.record_timing(
            "sys",
            Duration::from_millis(40),
            SystemStage::UpdateOrLast,
            0.0,
        );
        let top = profiler.get_top_n_update(1);
        assert_eq!(top[0].1, Duration::from_millis(30));
    }

    #[test]
    fn test_system_profiler_should_show_startup() {
        let profiler = SystemProfiler::new(60);

        // No startup systems recorded yet
        assert!(!profiler.should_show_startup(0.0));

        // Record a startup system at time 1.0 - visible until 6.0
        profiler.record_timing(
            "startup_sys",
            Duration::from_millis(1),
            SystemStage::Startup,
            1.0,
        );
        assert!(profiler.should_show_startup(3.0));
        assert!(profiler.should_show_startup(5.9));
        assert!(!profiler.should_show_startup(6.1));
    }

    #[test]
    fn test_system_profiler_clear() {
        let profiler = SystemProfiler::new(60);
        profiler.record_timing(
            "sys",
            Duration::from_millis(1),
            SystemStage::UpdateOrLast,
            0.0,
        );
        profiler.record_timing(
            "startup",
            Duration::from_millis(1),
            SystemStage::Startup,
            0.0,
        );

        profiler.clear();
        assert_eq!(profiler.get_top_n_update(10).len(), 0);
        assert_eq!(profiler.get_top_n_startup(10).len(), 0);
        assert!(!profiler.should_show_startup(0.0));
    }

    #[test]
    fn test_system_profiler_top_n_limit() {
        let profiler = SystemProfiler::new(60);
        for i in 0..10 {
            profiler.record_timing(
                &format!("sys_{}", i),
                Duration::from_millis(i as u64),
                SystemStage::UpdateOrLast,
                0.0,
            );
        }
        let top3 = profiler.get_top_n_update(3);
        assert_eq!(top3.len(), 3);
        // Should be the slowest 3
        assert_eq!(top3[0].1, Duration::from_millis(9));
    }

    #[test]
    fn record_run_files_timing_when_profiler_present() {
        let mut world = World::new();
        world.insert_resource(SystemProfiler::new(60));
        // No `Time` inserted: current_time falls back to 0.0 but the run is still
        // recorded, matching the previous inline epilogue's `unwrap_or(0.0)`.
        SystemProfiler::record_run(
            &world,
            "sys",
            Duration::from_millis(2),
            SystemStage::UpdateOrLast,
        );
        let profiler = world.get_resource::<SystemProfiler>().unwrap();
        let top = profiler.get_top_n_update(10);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].0, "sys");
    }

    #[test]
    fn record_run_is_noop_without_profiler() {
        let world = World::new();
        // Must not panic when the world has no profiler (hot reload inactive).
        SystemProfiler::record_run(
            &world,
            "sys",
            Duration::from_millis(2),
            SystemStage::UpdateOrLast,
        );
    }

    #[test]
    fn test_memory_profile_baseline() {
        let mut profile = MemoryProfile::default();
        assert!(!profile.baseline_captured);
        profile.capture_baseline(100.0);
        assert!(profile.baseline_captured);
        assert_eq!(profile.baseline_rss_mb, 100.0);
        // Second call is a no-op
        profile.capture_baseline(200.0);
        assert_eq!(profile.baseline_rss_mb, 100.0);
    }

    #[test]
    fn test_memory_profile_growth() {
        let mut profile = MemoryProfile::default();
        profile.capture_baseline(100.0);
        assert_eq!(profile.growth_mb(150.0), 50.0);
        assert!(!profile.is_warning(150.0));
        // Exceed warning threshold (200MB default)
        assert!(profile.is_warning(350.0));
    }

    #[test]
    fn test_memory_profile_snapshots() {
        let mut profile = MemoryProfile::default();
        profile.capture_snapshot(1, 100.0, 5000, 20, 6);
        assert_eq!(profile.snapshots.len(), 1);
        assert_eq!(profile.snapshots[0].delta_mb, 0.0);
        assert_eq!(profile.snapshots[0].current_generation_systems, 6);
        assert_eq!(profile.peak_rss_mb, 100.0);

        profile.capture_snapshot(2, 110.0, 5500, 25, 6);
        assert_eq!(profile.snapshots.len(), 2);
        assert_eq!(profile.snapshots[1].delta_mb, 10.0);
        assert_eq!(profile.peak_rss_mb, 110.0);

        // Peak tracks correctly even if current RSS drops
        profile.capture_snapshot(3, 105.0, 5200, 22, 6);
        assert_eq!(profile.peak_rss_mb, 110.0);
        assert_eq!(profile.snapshots[2].delta_mb, -5.0);
    }

    #[test]
    fn test_memory_profile_rolling_window() {
        let mut profile = MemoryProfile::default();
        for i in 0..25 {
            profile.capture_snapshot(i, 100.0 + i as f64, 5000, 20, 6);
        }
        assert_eq!(profile.snapshots.len(), MemoryProfile::MAX_SNAPSHOTS);
        // First snapshot should be generation 5 (0-4 were evicted)
        assert_eq!(profile.snapshots[0].generation, 5);
    }
}
