use std::{
    env,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU32, Ordering},
    },
};

/// Helper to lock a mutex, recovering from poison if a thread panicked while holding it.
/// This prevents cascading panics when one thread fails - the data may be in an
/// inconsistent state but we can continue operating.
fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| {
        bevy::log::warn!(
            "Recovered from poisoned mutex - a thread may have panicked while holding the lock"
        );
        poisoned.into_inner()
    })
}

use bevy::{
    animation::{AnimationClip, graph::AnimationGraph},
    app::{
        App, First, FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Last,
        Main, PostStartup, PostUpdate, PreStartup, PreUpdate, Startup, Update,
    },
    asset::{Asset, AssetServer, Assets},
    ecs::{
        component::Component,
        schedule::{Chain, IntoScheduleConfigs, ScheduleConfigs, Schedules},
        world::World,
    },
    image::Image,
    mesh::Mesh,
    pbr::StandardMaterial,
    prelude::{Resource, default},
    scene::Scene,
    ui::widget::Text,
};
use pybevy_core::PyPlugin;
use pyo3::{
    prelude::*,
    types::{PyAnyMethods, PyType},
};

use crate::{
    app::{PyStage, SimTick, app::PyApp, chained_systems::PyChainedSystems},
    ecs::{
        dynamic_system::{DynamicSystem, DynamicSystemHandle},
        messages::MessageRegistry,
    },
};

/// Check if verbose debug output is enabled via environment variable
fn is_verbose() -> bool {
    env::var("PYBEVY_VERBOSE")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// Parse a "WIDTHxHEIGHT" resolution string into (width, height) as f32.
/// Accepts both lowercase 'x' and uppercase 'X' as separators.
fn parse_resolution(s: &str) -> Option<(f32, f32)> {
    let (w, h) = s.split_once('x').or_else(|| s.split_once('X'))?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

/// Get current RSS of this process in MB using the SystemMonitor resource.
/// Returns 0.0 if the resource is not available or PID lookup fails.
fn get_current_rss_mb(world: &World) -> f64 {
    let Some(monitor) = world.get_resource::<SystemMonitor>() else {
        return 0.0;
    };
    let Some(pid) = monitor.process_pid else {
        return 0.0;
    };
    monitor
        .system
        .process(pid)
        .map(|p| p.memory() as f64 / 1_048_576.0)
        .unwrap_or(0.0)
}

/// Get total Python GC tracked objects (sum of gc.get_count() across all generations).
fn get_python_gc_objects() -> usize {
    Python::attach(|py| {
        if let Ok(gc) = py.import("gc") {
            if let Ok(counts) = gc.call_method0("get_count") {
                if let Ok(tuple) = counts.extract::<(usize, usize, usize)>() {
                    return tuple.0 + tuple.1 + tuple.2;
                }
            }
        }
        0
    })
}

/// Count total systems across all schedules in the world.
fn count_schedule_systems(world: &World) -> usize {
    use bevy::ecs::schedule::Schedules;
    let Some(schedules) = world.get_resource::<Schedules>() else {
        return 0;
    };
    schedules.iter().map(|(_, s)| s.systems_len()).sum()
}

/// Detect if Python GIL is enabled (Python 3.13+ free-threading detection)
fn detect_gil_status() -> bool {
    Python::attach(|py| {
        // Try Python 3.13+ free-threading detection
        if let Ok(sys) = py.import("sys") {
            if let Ok(enabled_attr) = sys.getattr("_is_gil_enabled") {
                if let Ok(result) = enabled_attr.call0() {
                    return result.extract().unwrap_or(true);
                }
            }
        }
        // GIL is enabled by default (standard CPython)
        true
    })
}

/// Marker component added to entities spawned by user code
/// Entities WITHOUT this marker are preserved during hot reload (Bevy internals)
#[derive(Component, Clone, Copy)]
pub struct HotReloadable;

/// Shared state for hot reload functionality
/// Arc<Mutex<...>> allows thread-safe access from both Python watcher thread and Bevy systems
#[derive(Clone)]
pub struct HotReloadState {
    inner: Arc<Mutex<HotReloadStateInner>>,
}

/// Mode for hot reload operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadMode {
    /// Full reload: despawn entities, clear custom resources, reload all systems including Startup
    Full,
    /// Partial reload: keep entities and resources, only update Update/Last systems
    Partial,
}

struct HotReloadStateInner {
    /// Function to call to get fresh systems when reload is requested
    /// This should return a new create_app() function
    loader_func: Option<Py<PyAny>>,
    /// Flag indicating a reload has been requested
    reload_pending: bool,
    /// Mode for the next reload (Full or Partial)
    reload_mode: ReloadMode,
    /// Default mode for file-triggered reloads (can be toggled with F6)
    default_reload_mode: ReloadMode,
    /// Current generation counter - systems check this to know if they should run
    /// This is atomic so it can be read without locking from Bevy systems
    generation: Arc<AtomicU32>,
}

impl HotReloadState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HotReloadStateInner {
                loader_func: None,
                reload_pending: false,
                reload_mode: ReloadMode::Full,
                default_reload_mode: ReloadMode::Partial,
                generation: Arc::new(AtomicU32::new(0)),
            })),
        }
    }

    /// Create a new state with a specific generation (for hot reload temp apps)
    pub(crate) fn with_generation(generation: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HotReloadStateInner {
                loader_func: None,
                reload_pending: false,
                reload_mode: ReloadMode::Full,
                default_reload_mode: ReloadMode::Partial,
                generation: Arc::new(AtomicU32::new(generation)),
            })),
        }
    }

    /// Get the current default reload mode
    pub fn get_default_mode(&self) -> ReloadMode {
        lock_or_recover(&self.inner).default_reload_mode
    }

    /// Set the default reload mode (for file-triggered reloads)
    pub fn set_default_mode(&self, mode: ReloadMode) {
        lock_or_recover(&self.inner).default_reload_mode = mode;
    }

    /// Get the current generation counter
    pub fn generation(&self) -> Arc<AtomicU32> {
        lock_or_recover(&self.inner).generation.clone()
    }

    /// Increment the generation counter (disables all old systems)
    pub fn increment_generation(&self) {
        let inner = lock_or_recover(&self.inner);
        inner.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Get the current generation value
    pub fn current_generation(&self) -> u32 {
        lock_or_recover(&self.inner)
            .generation
            .load(Ordering::SeqCst)
    }

    /// Set the generation counter to a specific value (for rollback on reload failure)
    pub fn set_generation(&self, generation: u32) {
        let inner = lock_or_recover(&self.inner);
        inner.generation.store(generation, Ordering::SeqCst);
    }

    /// Set the loader function that will be called to recreate the app
    pub fn set_loader(&self, func: Py<PyAny>) {
        let mut inner = lock_or_recover(&self.inner);
        inner.loader_func = Some(func);
    }

    /// Check if hot reload is enabled (loader function is set)
    pub fn is_enabled(&self) -> bool {
        let inner = lock_or_recover(&self.inner);
        inner.loader_func.is_some()
    }

    /// Check if the next reload will be in partial mode
    /// Used by CLI loader to determine whether to enable component caching
    pub fn is_partial_reload(&self) -> bool {
        let inner = lock_or_recover(&self.inner);
        let is_partial = inner.reload_mode == ReloadMode::Partial;
        if is_verbose() {
            eprintln!(
                "🔍 is_partial_reload() called: reload_mode = {:?}, returning {}",
                inner.reload_mode, is_partial
            );
        }
        is_partial
    }

    /// Request a reload (called from Python watcher thread)
    pub fn request_reload(&self, mode: ReloadMode) {
        let mut inner = lock_or_recover(&self.inner);
        inner.reload_pending = true;
        inner.reload_mode = mode;
    }

    /// Check if reload is pending and get the loader function and mode if available
    pub fn take_pending_reload(&self, py: Python) -> Option<(Py<PyAny>, ReloadMode)> {
        let mut inner = lock_or_recover(&self.inner);
        if inner.reload_pending {
            inner.reload_pending = false;
            inner
                .loader_func
                .as_ref()
                .map(|f| (f.clone_ref(py), inner.reload_mode))
        } else {
            None
        }
    }
}

/// Bevy resource that holds the hot reload state
#[derive(Resource, Clone)]
pub struct HotReloadResource {
    pub state: HotReloadState,
    error_state: Arc<Mutex<Vec<PyErr>>>,
}

impl HotReloadResource {
    pub fn new(state: HotReloadState, error_state: Arc<Mutex<Vec<PyErr>>>) -> Self {
        Self { state, error_state }
    }
}

/// Bevy resource that tracks which generation of systems should be active
/// Systems added with a specific generation will only run when this matches
#[derive(Resource, Clone)]
pub struct HotReloadGeneration {
    /// Current active generation
    pub current: u32,
    /// Atomic counter shared with HotReloadState
    generation_counter: Arc<AtomicU32>,
    /// Track which generations have already run their Startup schedule
    /// This prevents Startup from running multiple times per generation
    startup_run_for_generations: Arc<std::sync::Mutex<std::collections::HashSet<u32>>>,
}

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
    /// Whether the last reload attempt failed
    pub last_reload_failed: bool,
    /// Failure reason for the last reload attempt
    pub last_failure_reason: Option<String>,
    /// Asset counts by type
    pub asset_counts: std::collections::HashMap<String, usize>,
    /// Timestamp of last error displayed in overlay (to detect new errors)
    pub last_error_timestamp: f64,
    /// Frame number of last reload (for frame-based cooldown)
    pub last_reload_frame: u32,
}

/// Resource for tracking system information
#[derive(Resource)]
pub struct SystemMonitor {
    system: sysinfo::System,
    /// Process ID for monitoring, None if PID could not be determined
    process_pid: Option<sysinfo::Pid>,
    last_update: f64,
    /// Last 60 FPS values for rolling average
    fps_history: std::collections::VecDeque<f32>,
    /// Last time the overlay text was updated (for throttling)
    last_render_update: f64,
}

/// Stage where a system runs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemStage {
    Startup,
    UpdateOrLast,
}

/// Number of old generations to keep alive (avoids gutting systems we might roll back to)
const KEEP_ALIVE_GENERATIONS: u32 = 1;

/// Registry tracking DynamicSystem handles by generation.
/// Used to "gut" old-generation systems, releasing their Python references
/// even though the DynamicSystem structs remain in Bevy's schedule.
#[derive(Resource, Default)]
pub(crate) struct DynamicSystemRegistry {
    generations: std::collections::HashMap<u32, Vec<DynamicSystemHandle>>,
    /// System names from the most recent generation, for detecting renames/removals
    known_systems: std::collections::HashSet<String>,
}

impl DynamicSystemRegistry {
    /// Register a system handle for a generation
    pub(crate) fn register(&mut self, generation: u32, handle: DynamicSystemHandle) {
        self.generations.entry(generation).or_default().push(handle);
    }

    /// Gut all systems from generations older than the threshold.
    /// Acquires GIL first, then per-system Mutexes, ensuring consistent
    /// lock ordering with DynamicSystem::run_unsafe (GIL → Mutex).
    pub(crate) fn cleanup_old_generations(&mut self, keep_after: u32) {
        let old_gens: Vec<u32> = self
            .generations
            .keys()
            .filter(|&&g| g < keep_after)
            .copied()
            .collect();

        if old_gens.is_empty() {
            return;
        }

        // Hold GIL for the entire gut loop so dropping Py<PyAny> refs is safe,
        // and lock ordering is GIL → per-system Mutex (matching run_unsafe).
        Python::attach(|_py| {
            for old_gen in old_gens {
                if let Some(handles) = self.generations.remove(&old_gen) {
                    for handle in handles {
                        let mut inner = handle.lock().unwrap_or_else(|p| p.into_inner());
                        inner.gut();
                    }
                }
            }
        });
    }

    /// Compare new system names against known set, update tracker, return removed names.
    pub(crate) fn detect_system_delta(
        &mut self,
        new_systems: std::collections::HashSet<String>,
    ) -> Vec<String> {
        if self.known_systems.is_empty() {
            // First reload: record the initial set
            self.known_systems = new_systems;
            return Vec::new();
        }

        let removed: Vec<_> = self
            .known_systems
            .difference(&new_systems)
            .cloned()
            .collect();
        self.known_systems = new_systems;
        removed
    }
}

/// Memory snapshot captured at each reload event
#[derive(Clone)]
pub(crate) struct ReloadMemorySnapshot {
    /// Generation number at time of snapshot
    pub generation: u32,
    /// Process RSS in MB
    pub rss_mb: f64,
    /// Delta from previous snapshot (MB)
    pub delta_mb: f64,
    /// Python GC tracked objects (sum of gc.get_count())
    pub gc_objects: usize,
    /// Total systems across all schedules
    pub schedule_systems: usize,
}

/// Resource tracking memory across reloads (rolling window of snapshots).
/// Captures a snapshot at each reload for trend analysis.
#[derive(Resource)]
pub(crate) struct MemoryProfile {
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
    const MAX_SNAPSHOTS: usize = 20;

    /// Capture a snapshot at reload time
    pub fn capture_snapshot(
        &mut self,
        generation: u32,
        rss_mb: f64,
        gc_objects: usize,
        schedule_systems: usize,
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

/// Whether memory overlay (F7) is visible
#[derive(Resource)]
pub(crate) struct MemoryOverlayVisible(pub bool);

/// Whether the app started in paused mode (--pause flag)
/// When true, user systems are disabled until Space is pressed
#[derive(Resource)]
pub(crate) struct StartPaused(pub bool);

/// Resource tracking which plugins have been registered across reloads.
/// Used for delta detection: new plugins are reported, removed plugins
/// trigger a "restart required" warning.
#[derive(Resource, Default)]
pub(crate) struct PluginTracker {
    /// Set of plugin names that were present in the last successful reload
    pub known_plugins: std::collections::HashSet<String>,
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
    update_systems: std::collections::HashMap<String, SystemTimingStats>,
    /// Startup stage systems
    startup_systems: std::collections::HashMap<String, SystemTimingStats>,
    /// Time when startup systems should stop being displayed (5 seconds after first startup)
    startup_visible_until: Option<f64>,
}

struct SystemTimingStats {
    /// Circular buffer of recent execution times
    recent_times: std::collections::VecDeque<std::time::Duration>,
    /// Cached rolling average (updated each frame)
    average_time: std::time::Duration,
}

impl SystemProfiler {
    /// Create a new profiler with specified window size
    pub fn new(window_size: usize) -> Self {
        Self {
            stats: Arc::new(Mutex::new(ProfilerData {
                update_systems: std::collections::HashMap::new(),
                startup_systems: std::collections::HashMap::new(),
                startup_visible_until: None,
            })),
            window_size,
        }
    }

    /// Record a timing measurement for a system (concurrent-safe)
    pub fn record_timing(
        &self,
        system_name: &str,
        duration: std::time::Duration,
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
                recent_times: std::collections::VecDeque::with_capacity(self.window_size),
                average_time: std::time::Duration::ZERO,
            });

        // Add new timing to circular buffer
        entry.recent_times.push_back(duration);

        // Remove oldest if we exceed window size
        if entry.recent_times.len() > self.window_size {
            entry.recent_times.pop_front();
        }

        // Recalculate rolling average
        if !entry.recent_times.is_empty() {
            let sum: std::time::Duration = entry.recent_times.iter().sum();
            entry.average_time = sum / entry.recent_times.len() as u32;
        }
    }

    /// Get the top N Update/Last systems by average execution time (concurrent-safe)
    pub fn get_top_n_update(&self, n: usize) -> Vec<(String, std::time::Duration)> {
        let data = lock_or_recover(&self.stats);
        let mut systems: Vec<(String, std::time::Duration)> = data
            .update_systems
            .iter()
            .map(|(name, stats)| (name.clone(), stats.average_time))
            .collect();

        // Sort by average time (descending)
        systems.sort_by(|a, b| b.1.cmp(&a.1));

        // Take top N
        systems.into_iter().take(n).collect()
    }

    /// Get the top N Startup systems by average execution time (concurrent-safe)
    pub fn get_top_n_startup(&self, n: usize) -> Vec<(String, std::time::Duration)> {
        let data = lock_or_recover(&self.stats);
        let mut systems: Vec<(String, std::time::Duration)> = data
            .startup_systems
            .iter()
            .map(|(name, stats)| (name.clone(), stats.average_time))
            .collect();

        // Sort by average time (descending)
        systems.sort_by(|a, b| b.1.cmp(&a.1));

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

impl HotReloadGeneration {
    pub fn new(generation_counter: Arc<AtomicU32>) -> Self {
        Self {
            current: generation_counter.load(Ordering::SeqCst),
            generation_counter,
            startup_run_for_generations: Arc::new(std::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
        }
    }

    /// Update to the latest generation from the atomic counter
    pub fn update(&mut self) {
        self.current = self.generation_counter.load(Ordering::SeqCst);
    }

    /// Mark that Startup has run for the current generation
    pub fn mark_startup_run(&self) {
        if let Ok(mut set) = self.startup_run_for_generations.lock() {
            set.insert(self.current);
        }
    }

    /// Check if Startup has already run for a given generation
    pub fn has_startup_run(&self, generation: u32) -> bool {
        self.startup_run_for_generations
            .lock()
            .map(|set| set.contains(&generation))
            .unwrap_or(false)
    }
}

/// Python-exposed reload state class (for CLI watcher thread)
#[pyclass(name = "AppReloadState")]
pub struct PyAppReloadState {
    state: HotReloadState,
}

#[pymethods]
impl PyAppReloadState {
    /// Called by CLI watcher thread when files change - full reload
    pub fn set_pending_reload(&self) {
        self.state.request_reload(ReloadMode::Full);
    }

    /// Called by CLI watcher thread for partial reload (Update systems only)
    pub fn set_pending_partial_reload(&self) {
        self.state.request_reload(ReloadMode::Partial);
    }

    /// Called by CLI watcher thread when file changes are detected
    /// Only triggers reload if one isn't already pending (e.g., from F5)
    /// Uses the current default_reload_mode (which can be toggled with F6)
    pub fn trigger_reload_if_needed(&self, _default_mode_is_partial: bool) {
        let mut inner = lock_or_recover(&self.state.inner);

        // If reload already pending (e.g., F5 just pressed), don't override it
        if inner.reload_pending {
            return;
        }

        // No reload pending, trigger one with the current default mode
        let mode = inner.default_reload_mode;

        inner.reload_pending = true;
        inner.reload_mode = mode;
    }

    /// Get the current default reload mode
    pub fn get_default_mode(&self) -> String {
        let mode = self.state.get_default_mode();
        match mode {
            ReloadMode::Full => "Full".to_string(),
            ReloadMode::Partial => "Partial".to_string(),
        }
    }

    /// Check if the next reload will be in partial mode
    /// Used by CLI loader to determine whether to enable component caching
    pub fn is_partial_reload(&self) -> bool {
        self.state.is_partial_reload()
    }
}

impl PyAppReloadState {
    pub fn new(state: HotReloadState) -> Self {
        Self { state }
    }
}

use crate::ecs::resource::PyResource;

/// Python-exposed hot reload control resource (for in-game systems)
/// This allows Python systems to request reloads dynamically (e.g., on F5 press)
#[pyclass(name = "HotReloadControl", extends = PyResource, frozen)]
#[derive(Clone)]
pub struct PyHotReloadControl {
    state: HotReloadState,
}

#[pymethods]
impl PyHotReloadControl {
    /// Request an immediate full reload
    /// This will despawn entities, clear custom resources, and reload all systems including Startup
    /// The reload happens immediately without waiting for a file change
    pub fn request_full_reload(&self) {
        self.state.request_reload(ReloadMode::Full);
    }

    /// Request an immediate partial reload
    /// This preserves entities and resources, only updating Update/Last systems
    pub fn request_partial_reload(&self) {
        self.state.request_reload(ReloadMode::Partial);
    }

    /// Check if hot reload is currently enabled
    pub fn is_enabled(&self) -> bool {
        self.state.is_enabled()
    }

    /// Get the current generation number
    pub fn generation(&self) -> u32 {
        self.state.current_generation()
    }
}

impl PyHotReloadControl {
    pub fn new(state: HotReloadState) -> (Self, PyResource) {
        (Self { state }, PyResource)
    }
}

/// Built-in system that checks for F5/F6 keypress and triggers reload or mode toggle
/// This runs automatically when hot reload is enabled - no user code needed
pub(crate) fn handle_f5_reload_system(world: &mut World) {
    use bevy::input::{ButtonInput, keyboard::KeyCode};

    // Read all key states upfront, then drop the immutable borrow
    let (f5_pressed, f6_pressed, f7_pressed, space_pressed) = {
        let Some(input) = world.get_resource::<ButtonInput<KeyCode>>() else {
            return;
        };
        (
            input.just_pressed(KeyCode::F5),
            input.just_pressed(KeyCode::F6),
            input.just_pressed(KeyCode::F7),
            input.just_pressed(KeyCode::Space),
        )
    };

    // Check if Space was pressed while in paused mode
    if space_pressed {
        let is_paused = world.get_resource::<StartPaused>().is_some_and(|p| p.0);
        if is_paused {
            eprintln!("▶ Space pressed — loading scene...");
            if let Some(mut paused) = world.get_resource_mut::<StartPaused>() {
                paused.0 = false;
            }
            if let Some(reload_res) = world.get_resource::<HotReloadResource>() {
                reload_res.state.request_reload(ReloadMode::Full);
            }
        }
    }

    // Check if F5 was just pressed (full reload)
    if f5_pressed {
        // Enforce a frame-based cooldown to let the render pipeline finish processing
        // entity despawns before spawning new ones. Without this, rapid reloads can
        // corrupt GPU buffer state and permanently degrade FPS.
        const RELOAD_COOLDOWN_FRAMES: u32 = 5;

        let current_frame = world
            .get_resource::<bevy::diagnostic::FrameCount>()
            .map(|f| f.0)
            .unwrap_or(0);
        let last_reload_frame = world
            .get_resource::<HotReloadStats>()
            .map(|s| s.last_reload_frame)
            .unwrap_or(0);
        let frames_since = current_frame.saturating_sub(last_reload_frame);

        if last_reload_frame > 0 && frames_since < RELOAD_COOLDOWN_FRAMES {
            // Skip — render pipeline still syncing
        } else {
            if is_verbose() {
                eprintln!("🔄 F5 pressed! Triggering full reload...");
            }
            // Clear paused state if active
            if let Some(mut paused) = world.get_resource_mut::<StartPaused>() {
                if paused.0 {
                    paused.0 = false;
                }
            }
            if let Some(reload_res) = world.get_resource::<HotReloadResource>() {
                reload_res.state.request_reload(ReloadMode::Full);
            }
        }
    }

    // Check if F6 was just pressed (toggle default mode)
    if f6_pressed {
        // Toggle mode in both HotReloadStats (for display) and HotReloadResource (for CLI access)
        let new_mode = if let Some(mut stats) = world.get_resource_mut::<HotReloadStats>() {
            stats.default_mode = match stats.default_mode {
                ReloadMode::Full => ReloadMode::Partial,
                ReloadMode::Partial => ReloadMode::Full,
            };
            Some(stats.default_mode)
        } else {
            None
        };

        // Sync the mode to HotReloadResource so CLI can access it
        if let (Some(mode), Some(reload_res)) =
            (new_mode, world.get_resource::<HotReloadResource>())
        {
            reload_res.state.set_default_mode(mode);

            if is_verbose() {
                eprintln!("🔄 F6 pressed! Default mode toggled to: {:?}", mode);
            }
        }
    }

    // Check if F7 was just pressed (toggle memory overlay)
    if f7_pressed {
        if let Some(mut visible) = world.get_resource_mut::<MemoryOverlayVisible>() {
            visible.0 = !visible.0;
            if is_verbose() {
                eprintln!(
                    "📊 F7 pressed! Memory overlay: {}",
                    if visible.0 { "ON" } else { "OFF" }
                );
            }
        }
    }
}

/// System that runs each frame to check for hot reload requests
pub fn check_hot_reload_system(world: &mut World) {
    // Check for MCP reload requests (cross-crate mailbox from pybevy_core)
    if let Some(mut mcp_request) = world.get_resource_mut::<pybevy_core::PendingReloadRequest>() {
        if let Some(mcp_mode) = mcp_request.mode.take() {
            if let Some(reload_res) = world.get_resource::<HotReloadResource>() {
                let mode = match mcp_mode {
                    pybevy_core::ReloadRequestMode::Full => ReloadMode::Full,
                    pybevy_core::ReloadRequestMode::Partial => ReloadMode::Partial,
                };
                reload_res.state.request_reload(mode);
            }
        }
    }

    // First check if reload is pending WITHOUT acquiring GIL (fast path)
    let is_reload_pending = {
        let reload_res = match world.get_resource::<HotReloadResource>() {
            Some(res) => res,
            None => return, // No reload resource, skip
        };

        // Check the atomic flag without GIL
        let inner = lock_or_recover(&reload_res.state.inner);
        inner.reload_pending
    };

    // If no reload pending, return early (99.99% of frames)
    if !is_reload_pending {
        return;
    }

    // Reload is pending - now acquire GIL briefly to get the loader func and mode
    let reload_data = Python::attach(|py| {
        // Take the pending reload (consumes the flag)
        let reload_res = world.resource::<HotReloadResource>();
        let reload_info = reload_res.state.take_pending_reload(py);

        let (loader_func, mode) = match reload_info {
            Some((func, mode)) => (func, mode),
            None => return None, // Already consumed by another thread
        };

        // Get the error state for passing to new systems
        let error_state = reload_res.error_state.clone();

        Some((loader_func, mode, error_state))
    });

    // If we got the loader func, perform reload WITHOUT holding GIL
    if let Some((loader_func, mode, error_state)) = reload_data {
        if let Err(e) = perform_reload(world, loader_func, mode, error_state) {
            let error_msg = e.to_string();
            Python::attach(|py| e.print(py));

            // Write reload errors to LastSystemError so MCP can see them
            let timestamp = world
                .get_resource::<bevy::time::Time>()
                .map(|t| t.elapsed_secs_f64())
                .unwrap_or(0.0);
            let mut last_error =
                world.get_resource_or_insert_with(pybevy_core::LastSystemError::default);
            last_error.error = Some(error_msg);
            last_error.timestamp_secs = timestamp;
        }
    }
}

/// Clear only programmatically-created assets (those added via `assets.add()`).
///
/// File-loaded assets (loaded via `asset_server.load()`) are preserved so that
/// handles returned by the AssetServer remain valid after reload. Without this,
/// `asset_server.load("model.glb#Scene0")` returns a stale handle pointing to
/// cleared data because the AssetServer deduplicates by path.
fn clear_programmatic_assets<T: Asset>(world: &mut World, verbose: bool, name: &str) {
    // Collect IDs of programmatic assets (those without a path in AssetServer)
    let ids_to_remove: Vec<_> = {
        let Some(asset_server) = world.get_resource::<AssetServer>() else {
            // No AssetServer — clear all assets (no file-loaded assets to preserve)
            if let Some(mut assets) = world.get_resource_mut::<Assets<T>>() {
                let count = assets.len();
                if count > 0 {
                    let handles: Vec<_> = assets.ids().map(|id| id.untyped()).collect();
                    for handle in handles {
                        assets.remove(handle.typed::<T>());
                    }
                    if verbose {
                        eprintln!("      Cleared {} {} (no AssetServer)", count, name);
                    }
                }
            }
            return;
        };
        let Some(assets) = world.get_resource::<Assets<T>>() else {
            return;
        };

        assets
            .ids()
            .filter(|id| asset_server.get_path(id.untyped()).is_none())
            .collect()
    };

    if ids_to_remove.is_empty() {
        return;
    }

    if let Some(mut assets) = world.get_resource_mut::<Assets<T>>() {
        for id in &ids_to_remove {
            assets.remove(*id);
        }
    }

    if verbose {
        let preserved = world.get_resource::<Assets<T>>().map_or(0, |a| a.len());
        eprintln!(
            "      Cleared {} programmatic {} (preserved {} file-loaded)",
            ids_to_remove.len(),
            name,
            preserved
        );
    }
}

/// Clear all custom Python resources from PyResourceStorage
/// Preserves built-in resources (Time, AssetServer, etc.) and HotReloadControl
fn clear_custom_resources(world: &mut World, verbose: bool) {
    use crate::ecs::resource_type::{PyResourceStorage, PyResourceType};

    // Save HotReloadControl before clearing (it needs to persist across reloads)
    // We iterate through all resources and save the one that is HotReloadControl
    let hot_reload_control = Python::attach(|py| {
        if let Some(storage) = world.get_resource::<PyResourceStorage>() {
            // Iterate through all resources to find HotReloadControl
            for resource in storage.resources.values() {
                let resource_bound = resource.bind(py);
                // Check if this resource is a HotReloadControl by trying to extract it
                if resource_bound
                    .extract::<PyRef<PyHotReloadControl>>()
                    .is_ok()
                {
                    return Some(resource.clone_ref(py));
                }
            }
        }
        None
    });

    // Clear PyResourceStorage (custom Python resources)
    if let Some(mut storage) = world.get_resource_mut::<PyResourceStorage>() {
        let count = storage.resources.len();

        // Drop all Python resource instances within Python::attach to ensure GIL
        Python::attach(|_py| {
            storage.resources.clear();
        });

        if verbose {
            eprintln!("   → Cleared {} custom Python resources", count);
        }
    }

    // Restore HotReloadControl after clearing
    if let Some(control) = hot_reload_control {
        Python::attach(|py| {
            // Get the type from the instance
            let control_bound = control.bind(py);
            let control_type = control_bound.get_type();
            let type_ptr = control_type.as_type_ptr();
            let py_resource_type = PyResourceType::Custom(type_ptr);

            // Re-insert the saved control
            if let Err(e) = py_resource_type.insert_into_world(world, py, control) {
                eprintln!("Warning: Failed to restore HotReloadControl: {:?}", e);
            }
        });
    }

    // Note: We preserve ResourceRegistry (type_ptr → ComponentId mapping)
    // ComponentIds are stable and reused across reloads for the same types
    // Only the Python object instances need clearing (except HotReloadControl)
}

/// Public function to clear all entities and custom resources (for JupyBevy scene reset).
///
/// This is similar to hot reload's Full mode entity/resource clearing but without
/// the system reloading logic. Used by JupyBevy to reset the scene when creating
/// a new instance.
///
/// Clears:
/// - All entities (no HotReloadable marker check - clears everything)
/// - Custom Python resources from PyResourceStorage
///
/// Preserves:
/// - Built-in Bevy resources (Time, AssetServer, etc.)
/// - RenderDevice and render infrastructure
/// - Plugin state
pub fn clear_entities_and_resources(world: &mut World) {
    use bevy::ecs::entity::Entity;

    // Despawn ALL entities
    // For JupyBevy, we want a complete clean slate
    let all_entities: Vec<Entity> = world
        .query::<bevy::ecs::world::EntityRef>()
        .iter(world)
        .map(|e| e.id())
        .collect();

    if is_verbose() {
        eprintln!("[clear_scene] Despawning {} entities", all_entities.len());
    }

    for entity in all_entities {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }

    // Clear custom Python resources
    if is_verbose() {
        eprintln!("[clear_scene] Clearing custom resources");
    }
    clear_custom_resources(world, is_verbose());

    if is_verbose() {
        eprintln!("[clear_scene] Scene cleared successfully");
    }
}

/// Perform the actual reload by incrementing generation and optionally clearing entities.
///
/// Uses a validate-then-commit pattern: the Python loader runs BEFORE the generation
/// is incremented, so if loading fails (syntax error, import error, etc.), the old
/// generation's systems keep running and the app doesn't freeze.
fn perform_reload(
    world: &mut World,
    loader_func: Py<PyAny>,
    mode: ReloadMode,
    error_state: Arc<Mutex<Vec<PyErr>>>,
) -> PyResult<()> {
    use pyo3::exceptions::PyRuntimeError;

    if is_verbose() {
        eprintln!("🔄 [Hot Reload] Starting {:?} reload", mode);
    }

    // Get the current generation BEFORE incrementing (needed for temp app and rollback)
    let old_generation = {
        let reload_res = world.resource::<HotReloadResource>();
        reload_res.state.current_generation()
    };

    // PHASE 1: VALIDATE — Call the Python loader BEFORE incrementing generation.
    // If this fails (syntax error, import error, runtime error), old systems keep running.
    let next_generation = old_generation + 1;
    let (
        _pending_systems,
        _pending_resources,
        _pending_messages,
        _pending_observers,
        _pending_plugins,
    ) = match Python::attach(
        |py| -> PyResult<(
            Vec<(PyStage, Vec<Py<PyAny>>)>,
            Vec<Py<PyAny>>,
            Vec<Py<PyType>>,
            Vec<Py<PyAny>>,
            Vec<String>,
        )> {
            use crate::app::app::PyApp;

            // Create a temporary app with what WILL be the new generation
            let temp_app = PyApp::create_reload_temp(next_generation);
            let temp_app_py = Py::new(py, temp_app)?;

            // loader_func returns the create_app function
            let create_app_bound = loader_func.bind(py).call0()?;

            // Call create_app(app=temp_app) to collect definitions
            let _result_app = create_app_bound.call1((temp_app_py.clone_ref(py),))?;

            // Extract systems, resources, message types, observers, and plugin names
            let temp_app_ref = temp_app_py.borrow(py);
            let pending_systems = temp_app_ref.take_pending_systems();
            let pending_resources = temp_app_ref.take_pending_resources();
            let pending_messages = temp_app_ref.take_pending_messages();
            let pending_observers = temp_app_ref.take_pending_observers();
            let pending_plugins = temp_app_ref.take_pending_plugins();

            Ok((
                pending_systems,
                pending_resources,
                pending_messages,
                pending_observers,
                pending_plugins,
            ))
        },
    ) {
        Ok(result) => result,
        Err(e) => {
            // Loader failed — old generation keeps running, no disruption
            let error_msg = format!("Reload loader failed: {}", e);
            eprintln!("❌ [Hot Reload] {} — old systems still running", error_msg);
            Python::attach(|py| e.print(py));

            // Track failure in stats and ReloadResult
            let current_time = world
                .get_resource::<bevy::time::Time>()
                .map(|t| t.elapsed_secs_f64())
                .unwrap_or(0.0);
            let current_frame = world
                .get_resource::<bevy::diagnostic::FrameCount>()
                .map(|f| f.0)
                .unwrap_or(0);
            if let Some(mut stats) = world.get_resource_mut::<HotReloadStats>() {
                stats.reload_count += 1;
                stats.last_mode = Some(mode);
                stats.last_reload_failed = true;
                stats.last_failure_reason = Some(error_msg.clone());
                stats.last_reload_time = current_time;
                stats.last_reload_frame = current_frame;
            }
            let mut result = world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
            result.failed = true;
            result.failure_reason = Some(error_msg.clone());
            result.running_previous_generation = true;

            return Err(PyRuntimeError::new_err(error_msg));
        }
    };

    // PHASE 2: COMMIT — Loader succeeded. Now increment generation (point of no return).
    let new_generation = {
        let reload_res = world.resource::<HotReloadResource>();
        reload_res.state.increment_generation();
        reload_res.state.current_generation()
    };

    if is_verbose() {
        eprintln!("   → New generation: {}", new_generation);
    }

    // Update the world's generation resource
    {
        let mut gen_res = world.resource_mut::<HotReloadGeneration>();
        gen_res.update();
    }

    // Update hot reload statistics for the overlay
    if let Some(time) = world.get_resource::<bevy::time::Time>() {
        let current_time = time.elapsed_secs_f64();
        let current_frame = world
            .get_resource::<bevy::diagnostic::FrameCount>()
            .map(|f| f.0)
            .unwrap_or(0);
        if let Some(mut stats) = world.get_resource_mut::<HotReloadStats>() {
            stats.last_mode = Some(mode);
            stats.last_reload_time = current_time;
            stats.last_reload_frame = current_frame;
            stats.reload_count += 1;
        }
    }

    // Clear profiler stats on reload to start fresh timing measurements
    if let Some(profiler) = world.get_resource::<SystemProfiler>() {
        profiler.clear();
        if is_verbose() {
            eprintln!("   → Cleared system profiler stats");
        }
    }

    // For Full reload: clear entities, custom resources, and assets
    // For Partial reload: keep everything, only update systems
    if mode == ReloadMode::Full {
        // Clear only user-spawned entities (marked with HotReloadable)
        // Preserve Bevy's internal entities (window, renderer, etc. from plugins)
        use bevy::ecs::{entity::Entity, query::With};

        // Collect ALL entities with HotReloadable marker
        // We need to collect all at once before despawning to avoid iterator invalidation
        let mut query = world.query_filtered::<Entity, With<HotReloadable>>();
        let all_hotreloadable: Vec<Entity> = query.iter(world).collect();

        if is_verbose() {
            eprintln!("   → Despawning {} user entities", all_hotreloadable.len());
        }

        // Despawn all entities that still exist
        // Some may already be despawned as children of previously despawned parents
        for entity in all_hotreloadable {
            // Check if entity still exists before trying to despawn
            // (children are auto-despawned when parents are despawned)
            if world.get_entity(entity).is_ok() {
                world.despawn(entity);
            }
        }

        // Clear only programmatically-created assets (from assets.add()).
        // File-loaded assets (from asset_server.load()) are preserved so that
        // Startup systems can re-call asset_server.load() and get back a valid handle
        // pointing to already-loaded data (GLB scenes, images, etc.).
        // Without this, the AssetServer deduplicates by path and returns the old handle,
        // but the asset data was already cleared — resulting in invisible models.
        if is_verbose() {
            eprintln!("   → Clearing programmatic assets (preserving file-loaded)");
        }

        clear_programmatic_assets::<Mesh>(world, is_verbose(), "meshes");
        clear_programmatic_assets::<Image>(world, is_verbose(), "images");
        clear_programmatic_assets::<StandardMaterial>(world, is_verbose(), "materials");
        clear_programmatic_assets::<AnimationClip>(world, is_verbose(), "animation clips");
        clear_programmatic_assets::<AnimationGraph>(world, is_verbose(), "animation graphs");
        clear_programmatic_assets::<Scene>(world, is_verbose(), "scenes");

        // Clear all custom Python resources
        // Built-in resources (Time, AssetServer, etc.) are preserved
        if is_verbose() {
            eprintln!("   → Clearing custom resources");
        }
        clear_custom_resources(world, is_verbose());
    }

    // Call the loader to get new systems and resources, then add them to our running app
    let (pending_systems, pending_resources, pending_messages, pending_observers, pending_plugins) =
        Python::attach(
            |py| -> PyResult<(
                Vec<(PyStage, Vec<Py<PyAny>>)>,
                Vec<Py<PyAny>>,
                Vec<Py<PyType>>,
                Vec<Py<PyAny>>,
                Vec<String>,
            )> {
                use crate::app::app::PyApp;

                // Create a temporary app in reload mode (skips plugins, stores systems/resources)
                let temp_app = PyApp::create_reload_temp(new_generation);
                let temp_app_py = Py::new(py, temp_app)?;

                // loader_func returns the create_app function
                let create_app_bound = loader_func.bind(py).call0()?;

                // Call create_app(app=temp_app) to collect definitions
                let _result_app = create_app_bound.call1((temp_app_py.clone_ref(py),))?;

                // Extract systems, resources, message types, observers, and plugin names
                let temp_app_ref = temp_app_py.borrow(py);
                let pending_systems = temp_app_ref.take_pending_systems();
                let pending_resources = temp_app_ref.take_pending_resources();
                let pending_messages = temp_app_ref.take_pending_messages();
                let pending_observers = temp_app_ref.take_pending_observers();
                let pending_plugins = temp_app_ref.take_pending_plugins();

                Ok((
                    pending_systems,
                    pending_resources,
                    pending_messages,
                    pending_observers,
                    pending_plugins,
                ))
            },
        )?;

    // Reset reload result tracking (clear previous failure state on successful load)
    {
        let mut result = world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
        result.escalated = false;
        result.escalation_reason = None;
        result.failed = false;
        result.failure_reason = None;
        result.running_previous_generation = false;
        result.plugins_added = None;
        result.plugins_removed = None;
        result.systems_removed = None;
        result.actual_mode = Some(match mode {
            ReloadMode::Full => pybevy_core::ReloadRequestMode::Full,
            ReloadMode::Partial => pybevy_core::ReloadRequestMode::Partial,
        });
    }

    // Auto-escalation: detect if Partial reload needs to be Full
    let mut mode = mode;
    if mode == ReloadMode::Partial {
        let has_startup_systems = pending_systems.iter().any(|(stage, systems)| {
            !systems.is_empty()
                && matches!(
                    stage,
                    PyStage::Startup | PyStage::PreStartup | PyStage::PostStartup
                )
        });
        let has_resources = !pending_resources.is_empty();
        let has_observers = !pending_observers.is_empty();

        if has_startup_systems || has_resources || has_observers {
            let reason = if has_startup_systems && has_resources {
                "new Startup systems and resources detected"
            } else if has_startup_systems {
                "new Startup systems detected"
            } else if has_resources {
                "new resources detected"
            } else {
                "observers modified"
            };

            eprintln!("⬆️ [Hot Reload] Escalating Partial → Full: {reason}");

            mode = ReloadMode::Full;

            // Perform full cleanup that was skipped earlier
            {
                use bevy::ecs::{entity::Entity, query::With};
                let mut query = world.query_filtered::<Entity, With<HotReloadable>>();
                let all_hotreloadable: Vec<Entity> = query.iter(world).collect();

                if is_verbose() {
                    eprintln!(
                        "   → Despawning {} hot-reloadable entities (escalated)",
                        all_hotreloadable.len()
                    );
                }
                for entity in all_hotreloadable {
                    world.despawn(entity);
                }
            }

            // Clear programmatic assets (preserve file-loaded)
            clear_programmatic_assets::<Mesh>(world, is_verbose(), "meshes");
            clear_programmatic_assets::<Image>(world, is_verbose(), "images");
            clear_programmatic_assets::<StandardMaterial>(world, is_verbose(), "materials");
            clear_programmatic_assets::<AnimationClip>(world, is_verbose(), "animation clips");
            clear_programmatic_assets::<AnimationGraph>(world, is_verbose(), "animation graphs");
            clear_programmatic_assets::<Scene>(world, is_verbose(), "scenes");

            clear_custom_resources(world, is_verbose());

            // Update reload stats to reflect the escalation
            if let Some(mut stats) = world.get_resource_mut::<HotReloadStats>() {
                stats.last_mode = Some(ReloadMode::Full);
            }

            // Store escalation result for MCP to read
            if let Some(mut result) = world.get_resource_mut::<pybevy_core::ReloadResult>() {
                result.escalated = true;
                result.escalation_reason = Some(reason.to_string());
                result.actual_mode = Some(pybevy_core::ReloadRequestMode::Full);
            }
        }
    }

    // Plugin delta detection: compare current plugins against known set
    if !pending_plugins.is_empty() {
        let new_plugin_set: std::collections::HashSet<String> =
            pending_plugins.into_iter().collect();

        // Compute delta and update tracker in one borrow scope
        let (added, removed) = {
            if let Some(mut tracker) = world.get_resource_mut::<PluginTracker>() {
                if tracker.known_plugins.is_empty() {
                    // First reload: record the initial plugin set
                    tracker.known_plugins = new_plugin_set;
                    (Vec::new(), Vec::new())
                } else {
                    let added: Vec<_> = new_plugin_set
                        .difference(&tracker.known_plugins)
                        .cloned()
                        .collect();
                    let removed: Vec<_> = tracker
                        .known_plugins
                        .difference(&new_plugin_set)
                        .cloned()
                        .collect();
                    tracker.known_plugins = new_plugin_set;
                    (added, removed)
                }
            } else {
                (Vec::new(), Vec::new())
            }
        };

        if !added.is_empty() || !removed.is_empty() {
            if !added.is_empty() {
                eprintln!(
                    "⚠️ [Hot Reload] New plugins detected (restart may be required): {:?}",
                    added
                );
            }
            if !removed.is_empty() {
                eprintln!(
                    "⚠️ [Hot Reload] Plugins removed (restart required to take effect): {:?}",
                    removed
                );
            }

            // Store delta in ReloadResult for MCP
            if let Some(mut result) = world.get_resource_mut::<pybevy_core::ReloadResult>() {
                if !added.is_empty() {
                    result.plugins_added = Some(added);
                }
                if !removed.is_empty() {
                    result.plugins_removed = Some(removed);
                }
            }
        }
    }

    // For Full reload: insert resources
    // For Partial reload: skip resource insertion (keep existing resources)
    if mode == ReloadMode::Full {
        Python::attach(|py| -> PyResult<()> {
            use crate::ecs::world::PyWorld;

            PyWorld::with_temporary(world, py, |py_world| {
                for resource in pending_resources {
                    let resource_bound = resource.into_bound(py);
                    py_world.insert_resource(py, resource_bound)?;
                }
                Ok(())
            })
        })?;
    }

    // Re-register message types with updated Python class pointers.
    // During reload, Python classes are recreated with new PyTypeObject pointers.
    // The MessageRegistry maps type pointers → slot numbers. We add the new pointers
    // as aliases so both old pointers (held by MCP tool dispatchers) and new pointers
    // (used by reloaded systems' MessageReader[T]) resolve to the same slot.
    if !pending_messages.is_empty() {
        Python::attach(|py| -> PyResult<()> {
            use crate::ecs::messages::MessageRegistry;

            for msg_type in &pending_messages {
                let bound = msg_type.bind(py);
                let type_ptr = bound.as_type_ptr();

                // Check if already registered with this pointer (no-op if same class reused)
                let already_registered = world
                    .get_resource::<MessageRegistry>()
                    .map_or(false, |reg| reg.get(type_ptr).is_some());

                if !already_registered {
                    let class_name = bound
                        .getattr("__name__")
                        .and_then(|n| n.extract::<String>())
                        .unwrap_or_default();

                    // Add new pointer as alias for the existing registration (keeps old pointer too)
                    if let Some(mut registry) = world.get_resource_mut::<MessageRegistry>() {
                        if registry.alias_by_name(
                            type_ptr,
                            &class_name,
                            msg_type.clone_ref(py),
                            new_generation,
                        ) && is_verbose()
                        {
                            eprintln!(
                                "   → Aliased message '{}' with new type pointer",
                                class_name
                            );
                        }
                    }
                }
            }
            Ok(())
        })?;
    }

    // Re-register observers during Full reload.
    // Old observers are cleared and new ones registered from the reloaded Python code.
    // This must happen BEFORE Startup runs, because Startup may trigger events
    // that observers should catch.
    if mode == ReloadMode::Full && !pending_observers.is_empty() {
        Python::attach(|py| -> PyResult<()> {
            use crate::ecs::observer_registry::ObserverRegistry;

            // Clear old observers and collect their entity IDs for despawning
            if let Some(mut registry) = world.get_resource_mut::<ObserverRegistry>() {
                let old_entities = registry.clear_all();
                // Drop the borrow before despawning
                drop(registry);

                // Despawn old observer entities
                for entity in old_entities {
                    if world.get_entity(entity).is_ok() {
                        world.despawn(entity);
                    }
                }
            }

            // Register new observers
            for observer_func in &pending_observers {
                let func_bound = observer_func.bind(py);
                ObserverRegistry::register_observer(py, &func_bound, world)?;
            }

            if is_verbose() {
                eprintln!("   → Re-registered {} observers", pending_observers.len());
            }

            Ok(())
        })?;
    }

    // System delta detection: compare new system names against previous generation
    {
        let new_system_names: std::collections::HashSet<String> = Python::attach(|py| {
            let mut names = std::collections::HashSet::new();
            for (_stage, systems) in &pending_systems {
                for sys in systems {
                    let sys_bound = sys.bind(py);
                    // Handle ChainedSystems: extract names from inner tuple
                    if let Ok(chained) = sys_bound.extract::<PyChainedSystems>() {
                        for inner in chained.systems.bind(py).iter() {
                            if let Ok(name) = inner.getattr("__name__") {
                                if let Ok(s) = name.extract::<String>() {
                                    names.insert(s);
                                }
                            }
                        }
                    } else if let Ok(name) = sys_bound.getattr("__name__") {
                        if let Ok(s) = name.extract::<String>() {
                            names.insert(s);
                        }
                    }
                }
            }
            names
        });

        if let Some(mut registry) = world.get_resource_mut::<DynamicSystemRegistry>() {
            let removed = registry.detect_system_delta(new_system_names);
            if !removed.is_empty() {
                eprintln!(
                    "⚠️ [Hot Reload] Systems removed/renamed (stale schedule entries remain, use run_scene to clear): {:?}",
                    removed
                );
                if let Some(mut result) = world.get_resource_mut::<pybevy_core::ReloadResult>() {
                    result.systems_removed = Some(removed);
                }
            }
        }
    }

    // Clear the system parameter cache BEFORE creating new DynamicSystems.
    // CPython's allocator may reuse the same memory address for new function objects
    // after the old ones are freed during module re-execution. If the cache still holds
    // params keyed by that address, DynamicSystem::new() would get stale parameters
    // (e.g., old query signatures) instead of re-parsing the new function's annotations.
    crate::ecs::dynamic_system::clear_system_param_cache();

    // Collect system handles for the registry (enables gutting old-gen systems)
    let mut system_handles: Vec<DynamicSystemHandle> = Vec::new();

    // Add systems to the running app's schedules via World::schedule_scope
    for (stage, systems) in pending_systems {
        // Skip if no systems to add
        if systems.is_empty() {
            continue;
        }

        if is_verbose() {
            eprintln!("   → Adding {} systems to {:?}", systems.len(), stage);
        }

        // Ensure Last schedule exists before trying to add systems to it
        // This is needed because ScheduleRunnerPlugin doesn't create the Last schedule
        if stage == PyStage::Last {
            use bevy::ecs::schedule::{Schedule, ScheduleLabel, Schedules};
            let label = Last.intern();
            let mut schedules = world.resource_mut::<Schedules>();
            if !schedules.contains(label) {
                if is_verbose() {
                    eprintln!("   → Adding Last schedule (not present during reload)");
                }
                schedules.insert(Schedule::new(label));
            }
        }

        // Use schedule_scope to get mutable access to the schedule
        // We need to match on stage type since each schedule label is a different type
        // Macro to handle schedule-specific system addition
        macro_rules! add_systems_to_schedule {
            ($schedule:ident, $system_stage:ident, $run_condition:expr) => {
                world.schedule_scope($schedule, |_world, schedule| {
                    for system_func in systems {
                        let result = Python::attach(|py| -> PyResult<()> {
                            let system_bound = system_func.bind(py);

                            // Check if this is a ChainedSystems object
                            if let Ok(chained) = system_bound.extract::<PyChainedSystems>() {
                                // Handle chained systems
                                let systems_tuple = chained.systems.bind(py);
                                let mut dynamic_systems = Vec::new();

                                for sys in systems_tuple.iter() {
                                    let dynamic_system = DynamicSystem::new(
                                        sys.unbind(),
                                        new_generation,
                                        error_state.clone(),
                                        SystemStage::$system_stage,
                                    )?;
                                    system_handles.push(dynamic_system.handle());
                                    dynamic_systems.push(dynamic_system);
                                }

                                if dynamic_systems.is_empty() {
                                    return Err(PyRuntimeError::new_err("Empty chained systems"));
                                }

                                let configs: Vec<ScheduleConfigs<_>> = dynamic_systems
                                    .into_iter()
                                    .map(|sys| sys.run_if($run_condition))
                                    .collect();

                                let chained = ScheduleConfigs::Configs {
                                    configs,
                                    collective_conditions: Vec::new(),
                                    metadata: Chain::Chained(Default::default()),
                                };

                                schedule.add_systems(chained);
                            } else {
                                // Regular system function
                                let dynamic_system = DynamicSystem::new(
                                    system_func,
                                    new_generation,
                                    error_state.clone(),
                                    SystemStage::$system_stage,
                                )?;
                                system_handles.push(dynamic_system.handle());
                                schedule.add_systems(dynamic_system.run_if($run_condition));
                            }
                            Ok(())
                        });

                        if let Err(e) = result {
                            eprintln!(
                                "❌ [Hot Reload] Failed to add system to {:?} schedule: {}",
                                stage, e
                            );
                            eprintln!("   Reload cancelled to prevent broken app state.");
                            let error_state_clone = error_state.clone();
                            let mut error_lock = lock_or_recover(&error_state_clone);
                            error_lock.push(PyRuntimeError::new_err(format!(
                                "Hot reload failed: Could not add system to {:?} schedule: {}",
                                stage, e
                            )));
                            return;
                        }
                    }
                });
            };
        }

        // Note: Even in Partial mode, we add Startup systems with the new generation
        // (so they exist for future reloads), we just don't execute them
        match stage {
            PyStage::Startup => {
                add_systems_to_schedule!(Startup, Startup, startup_or_reload(new_generation));
            }
            PyStage::PreStartup => {
                add_systems_to_schedule!(PreStartup, Startup, startup_or_reload(new_generation));
            }
            PyStage::PostStartup => {
                add_systems_to_schedule!(PostStartup, Startup, startup_or_reload(new_generation));
            }
            PyStage::Main => {
                add_systems_to_schedule!(Main, UpdateOrLast, generation_matches(new_generation));
            }
            PyStage::First => {
                add_systems_to_schedule!(First, UpdateOrLast, generation_matches(new_generation));
            }
            PyStage::PreUpdate => {
                add_systems_to_schedule!(
                    PreUpdate,
                    UpdateOrLast,
                    generation_matches(new_generation)
                );
            }
            PyStage::Update => {
                add_systems_to_schedule!(Update, UpdateOrLast, generation_matches(new_generation));
            }
            PyStage::PostUpdate => {
                add_systems_to_schedule!(
                    PostUpdate,
                    UpdateOrLast,
                    generation_matches(new_generation)
                );
            }
            PyStage::Last => {
                add_systems_to_schedule!(Last, UpdateOrLast, generation_matches(new_generation));
            }
            PyStage::FixedFirst => {
                add_systems_to_schedule!(
                    FixedFirst,
                    UpdateOrLast,
                    generation_matches(new_generation)
                );
            }
            PyStage::FixedPreUpdate => {
                add_systems_to_schedule!(
                    FixedPreUpdate,
                    UpdateOrLast,
                    generation_matches(new_generation)
                );
            }
            PyStage::FixedUpdate => {
                add_systems_to_schedule!(
                    FixedUpdate,
                    UpdateOrLast,
                    generation_matches(new_generation)
                );
            }
            PyStage::FixedPostUpdate => {
                add_systems_to_schedule!(
                    FixedPostUpdate,
                    UpdateOrLast,
                    generation_matches(new_generation)
                );
            }
            PyStage::FixedLast => {
                add_systems_to_schedule!(
                    FixedLast,
                    UpdateOrLast,
                    generation_matches(new_generation)
                );
            }
            PyStage::SimTick => {
                add_systems_to_schedule!(SimTick, UpdateOrLast, generation_matches(new_generation));
            }
        }

        // Remove the old match arms below this line

        // Check if we stored an error during system addition and propagate it
        {
            let error_lock = lock_or_recover(&error_state);
            if let Some(err) = error_lock.last() {
                crate::ecs::dynamic_system::clear_system_param_cache();
                return Err(PyRuntimeError::new_err(err.to_string()));
            }
        }
    }

    // Snapshot the error timestamp before running Startup, so we can detect
    // if a Startup system wrote a new error during schedule execution.
    let pre_startup_error_ts = world
        .get_resource::<pybevy_core::LastSystemError>()
        .map(|e| e.timestamp_secs)
        .unwrap_or(0.0);

    // For Full reload: run Startup schedule to initialize new generation
    // For Partial reload: skip Startup (keep existing initialization)
    if mode == ReloadMode::Full {
        // Only run Startup if it exists (it might not exist in minimal apps)
        if world.resource::<Schedules>().contains(Startup) {
            if is_verbose() {
                eprintln!("   → Running Startup schedule");
            }

            // Snapshot entities before Startup so we can clean up on failure
            let pre_startup_entities: std::collections::HashSet<bevy::ecs::entity::Entity> = {
                use bevy::ecs::{entity::Entity, query::With};
                let mut query = world.query_filtered::<Entity, With<HotReloadable>>();
                query.iter(world).collect()
            };

            // Run Startup schedule with panic safety
            let startup_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                world.run_schedule(Startup);
            }));

            // Mark Startup as already run regardless of success/panic
            world.resource::<HotReloadGeneration>().mark_startup_run();

            // Handle Startup failure: rollback to old generation
            if let Err(panic_payload) = startup_result {
                let panic_msg = if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "unknown panic".to_string()
                };

                eprintln!(
                    "⚠️ [Hot Reload] Startup panicked: {} — rolling back to generation {}",
                    panic_msg, old_generation
                );

                // Clean up entities created during partial Startup execution
                {
                    use bevy::ecs::{entity::Entity, query::With};
                    let mut query = world.query_filtered::<Entity, With<HotReloadable>>();
                    let post_entities: Vec<Entity> = query.iter(world).collect();
                    let mut cleaned = 0;
                    for entity in post_entities {
                        if !pre_startup_entities.contains(&entity) {
                            if world.get_entity(entity).is_ok() {
                                world.despawn(entity);
                                cleaned += 1;
                            }
                        }
                    }
                    if is_verbose() && cleaned > 0 {
                        eprintln!(
                            "   → Cleaned up {} entities created during failed Startup",
                            cleaned
                        );
                    }
                }

                // Rollback generation so old systems resume
                {
                    let reload_res = world.resource::<HotReloadResource>();
                    reload_res.state.set_generation(old_generation);
                }
                {
                    let mut gen_res = world.resource_mut::<HotReloadGeneration>();
                    gen_res.current = old_generation;
                }

                // Allow old Startup to re-run (recreate the scene from old code)
                {
                    let gen_res = world.resource::<HotReloadGeneration>();
                    if let Ok(mut set) = gen_res.startup_run_for_generations.lock() {
                        set.remove(&old_generation);
                    }
                }

                // Track failure
                let error_msg = format!("Startup panicked: {}", panic_msg);
                if let Some(mut stats) = world.get_resource_mut::<HotReloadStats>() {
                    stats.last_reload_failed = true;
                    stats.last_failure_reason = Some(error_msg.clone());
                }
                let mut result =
                    world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
                result.failed = true;
                result.failure_reason = Some(error_msg.clone());
                result.running_previous_generation = true;

                // Clear system param cache even on failure to free stale Py<PyAny> refs
                crate::ecs::dynamic_system::clear_system_param_cache();

                return Err(PyRuntimeError::new_err(error_msg));
            }
        } else if is_verbose() {
            eprintln!("   → Skipping Startup schedule (not present in app)");
        }
    } else if is_verbose() {
        eprintln!("   → Skipping Startup schedule (Partial mode)");
    }

    // Clear last error on successful reload — but only if no new error was
    // written during the Startup schedule (a system error would have updated
    // the timestamp via DynamicSystem's error handler).
    let startup_had_error = world
        .get_resource::<pybevy_core::LastSystemError>()
        .is_some_and(|e| e.error.is_some() && e.timestamp_secs > pre_startup_error_ts);

    if !startup_had_error {
        if let Some(mut last_error) = world.get_resource_mut::<pybevy_core::LastSystemError>() {
            last_error.error = None;
            last_error.traceback = None;
        }
    }

    // Clear failure state on success
    if let Some(mut stats) = world.get_resource_mut::<HotReloadStats>() {
        stats.last_reload_failed = false;
        stats.last_failure_reason = None;
    }

    // Prune old entries from startup_run_for_generations to bound memory growth
    {
        let gen_res = world.resource::<HotReloadGeneration>();
        if let Ok(mut set) = gen_res.startup_run_for_generations.lock() {
            set.retain(|&g| g >= new_generation.saturating_sub(2));
        }
    }

    // Register system handles for this generation and gut old-generation systems
    if !system_handles.is_empty() {
        if let Some(mut registry) = world.get_resource_mut::<DynamicSystemRegistry>() {
            for handle in system_handles {
                registry.register(new_generation, handle);
            }

            // Gut systems from old generations to release their Python references.
            // Keep KEEP_ALIVE_GENERATIONS worth of systems for potential rollback.
            let keep_after = new_generation.saturating_sub(KEEP_ALIVE_GENERATIONS);
            registry.cleanup_old_generations(keep_after);

            if is_verbose() {
                eprintln!(
                    "   → Registered system handles for gen {} and gutted systems older than gen {}",
                    new_generation, keep_after
                );
            }
        }
    }

    // Prune old MessageRegistry entries to release stale Python type pointers
    if let Some(mut msg_registry) = world.get_resource_mut::<MessageRegistry>() {
        let keep_after = new_generation.saturating_sub(KEEP_ALIVE_GENERATIONS);
        msg_registry.prune_old_generations(keep_after);
    }

    // Clear again to free any Py<PyAny> refs cached during this reload cycle
    crate::ecs::dynamic_system::clear_system_param_cache();

    // Trigger Python garbage collection to reclaim cycles from released Py<PyAny> refs
    Python::attach(|py| {
        if let Ok(gc) = py.import("gc") {
            let _ = gc.call_method0("collect");
        }
    });

    // Capture memory snapshot for this reload
    {
        let rss_mb = get_current_rss_mb(world);
        let gc_objects = get_python_gc_objects();
        let schedule_systems = count_schedule_systems(world);

        if let Some(mut profile) = world.get_resource_mut::<MemoryProfile>() {
            profile.capture_snapshot(new_generation, rss_mb, gc_objects, schedule_systems);

            if is_verbose() {
                let growth = profile.growth_mb(rss_mb);
                let warning = if profile.is_warning(rss_mb) {
                    " ⚠️ WARNING"
                } else {
                    ""
                };
                eprintln!(
                    "   → Memory: {:.1}MB (growth: {:.1}MB, peak: {:.1}MB, gc: {}, systems: {}){}",
                    rss_mb, growth, profile.peak_rss_mb, gc_objects, schedule_systems, warning
                );
            }
        }
    }

    if is_verbose() {
        eprintln!("✅ [Hot Reload] {:?} reload complete\n", mode);
    }

    Ok(())
}

/// Run condition function that checks if the current generation matches the expected one
/// This is used to enable/disable Update and Last systems based on hot reload generation
/// If HotReloadGeneration resource doesn't exist (hot reload not enabled), always returns true
pub fn generation_matches(
    expected_generation: u32,
) -> impl FnMut(Option<bevy::ecs::system::Res<HotReloadGeneration>>) -> bool + Clone {
    move |generation_res: Option<bevy::ecs::system::Res<HotReloadGeneration>>| {
        match generation_res {
            Some(res) => res.current == expected_generation,
            None => true, // No hot reload, all systems run
        }
    }
}

/// Run condition for Startup systems - only runs when generation matches AND hasn't run yet
/// Startup systems should run once during their generation (at app start or after reload)
/// If HotReloadGeneration resource doesn't exist (hot reload not enabled), always returns true
pub fn startup_or_reload(
    expected_generation: u32,
) -> impl FnMut(Option<bevy::ecs::system::Res<HotReloadGeneration>>) -> bool + Clone {
    move |generation_res: Option<bevy::ecs::system::Res<HotReloadGeneration>>| {
        match generation_res {
            Some(res) => {
                // Only run if:
                // 1. Current generation matches expected generation
                // 2. Startup hasn't run yet for this generation
                res.current == expected_generation && !res.has_startup_run(expected_generation)
            }
            None => true, // No hot reload, all systems run
        }
    }
}

/// Marker component for the hot reload overlay text entity
#[derive(Component)]
struct HotReloadOverlayText;

/// Marker component for the hot reload overlay icon entity
#[derive(Component)]
struct HotReloadOverlayIcon;

/// Marker component for the hot reload error text entity
#[derive(Component)]
struct HotReloadErrorText;

/// System that spawns the hot reload overlay UI entity
/// Called immediately when hot reload is enabled
fn spawn_hot_reload_overlay_system(world: &mut World) {
    use bevy::{
        color::Color,
        prelude::{ImageNode, TextColor, TextFont},
        ui::{Node, PositionType, Val},
    };

    let verbose = is_verbose();
    if verbose {
        eprintln!("🎨 [Hot Reload Overlay] Spawning overlay...");
    }

    // Load embedded icon from binary (skip if AssetPlugin not loaded, e.g. in tests)
    static ICON_PNG: &[u8] = include_bytes!("../../assets/icon.png");
    let icon_handle = match image::load_from_memory(ICON_PNG) {
        Ok(dyn_img) => {
            let image =
                Image::from_dynamic(dyn_img, true, bevy::asset::RenderAssetUsages::default());
            match world.get_resource_mut::<Assets<Image>>() {
                Some(mut assets) => Some(assets.add(image)),
                None => {
                    if verbose {
                        eprintln!("   → Skipping icon (Assets<Image> not available)");
                    }
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("   → WARNING: Failed to decode embedded icon: {e}");
            None
        }
    };

    if let Some(handle) = icon_handle {
        let icon_entity = world.spawn((
            ImageNode::new(handle),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(12.0),
                width: Val::Px(48.0),
                height: Val::Px(48.0),
                ..default()
            },
            HotReloadOverlayIcon,
            #[cfg(feature = "mcp")]
            pybevy_control::bridge::InternalOverlayUi,
        ));

        if verbose {
            eprintln!("   → Spawned overlay icon entity: {:?}", icon_entity.id());
        }
    }

    // Spawn a text entity offset to the right of the icon (64px icon + 8px gap = 84px)
    // UI text works with Camera3d - no Camera2d needed
    let entity = world.spawn((
        Text::new("Hot Reload: Gen 0 | Last: -- | Reloads: 0 | Default: Partial (F6) | F5=Full | Mem: 0.0MB | CPU: 0.0% | GPU: -- | VRAM: --"),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgba(0.0, 1.0, 0.0, 1.0)), // Fully opaque green
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(84.0), // 64px icon + 20px padding
            right: Val::Px(12.0),
            ..default()
        },
        HotReloadOverlayText,
        #[cfg(feature = "mcp")]
        pybevy_control::bridge::InternalOverlayUi,
    ));

    if verbose {
        eprintln!("   → Spawned overlay text entity: {:?}", entity.id());
    }

    // Spawn error text entity below the stats overlay
    let error_entity = world.spawn((
        Text::new(""),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 0.3, 0.3, 1.0)), // Red for errors
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(70.0),
            left: Val::Px(84.0),
            right: Val::Px(12.0),
            ..default()
        },
        bevy::prelude::Visibility::Hidden,
        HotReloadErrorText,
        #[cfg(feature = "mcp")]
        pybevy_control::bridge::InternalOverlayUi,
    ));

    if verbose {
        eprintln!(
            "   → Spawned overlay error text entity: {:?}",
            error_entity.id()
        );
    }
}

/// System that updates system stats (memory, CPU, GPU, entities, assets) periodically
fn update_system_stats(world: &mut bevy::ecs::world::World) {
    // Extract resources - we need mutable access to monitor and stats
    let Some(mut monitor) = world.remove_resource::<SystemMonitor>() else {
        return;
    };
    let Some(mut stats) = world.remove_resource::<HotReloadStats>() else {
        world.insert_resource(monitor);
        return;
    };
    let Some(time) = world.get_resource::<bevy::time::Time>() else {
        world.insert_resource(monitor);
        world.insert_resource(stats);
        return;
    };

    let current_time = time.elapsed_secs_f64();

    // Update FPS every frame (lightweight calculation)
    let delta = time.delta_secs();
    if delta > 0.0 {
        let fps = 1.0 / delta;
        stats.fps_current = fps;

        // Add to rolling average buffer
        monitor.fps_history.push_back(fps);
        if monitor.fps_history.len() > 60 {
            monitor.fps_history.pop_front();
        }

        // Calculate rolling average
        if !monitor.fps_history.is_empty() {
            stats.fps_average =
                monitor.fps_history.iter().sum::<f32>() / monitor.fps_history.len() as f32;
        }
    }

    // Update uptime
    stats.uptime_secs = current_time;

    // Update stats every 1 second (respects sysinfo's minimum interval while reducing overhead)
    const UPDATE_INTERVAL: f64 = 1.0;
    if current_time - monitor.last_update >= UPDATE_INTERVAL {
        // Only update process stats if we have a valid PID
        if let Some(pid) = monitor.process_pid {
            // On Linux, must refresh global CPU state before process CPU for accurate measurements
            monitor.system.refresh_cpu_all();

            // Refresh process info with explicit CPU tracking enabled
            use sysinfo::ProcessRefreshKind;
            monitor.system.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[pid]),
                false,
                ProcessRefreshKind::nothing().with_cpu().with_memory(),
            );

            if let Some(process) = monitor.system.process(pid) {
                let new_memory = process.memory() as f64 / 1_048_576.0;
                let per_core_cpu = process.cpu_usage();

                // Divide by number of cores to show fraction of total CPU
                // process.cpu_usage() returns percentage where 100% = 1 full core
                // On a 4-core system, 200% usage = 2 cores = 50% of total CPU
                let num_cores = monitor.system.cpus().len().max(1) as f32;
                let total_cpu = per_core_cpu / num_cores;

                stats.memory_mb = new_memory;
                stats.cpu_percent = total_cpu;
            } else {
                eprintln!("[WARNING] Could not find process {} during update!", pid);
            }
        }

        // Update entity count (O(1) operation)
        stats.entity_count = world.entities().len() as usize;

        // Update asset counts by type (O(1) per type)
        use bevy::{
            animation::{AnimationClip, graph::AnimationGraph},
            asset::Assets,
            image::{Image, TextureAtlasLayout},
            mesh::Mesh,
            pbr::{StandardMaterial, wireframe::WireframeMaterial},
            scene::Scene,
        };

        stats.asset_counts.clear();

        if let Some(assets) = world.get_resource::<Assets<Mesh>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("Mesh".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<Image>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("Image".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<StandardMaterial>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("Material".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<AnimationGraph>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("AnimGraph".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<AnimationClip>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("AnimClip".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<Scene>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("Scene".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<TextureAtlasLayout>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("Atlas".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<WireframeMaterial>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("Wireframe".to_string(), count);
            }
        }

        monitor.last_update = current_time;

        // Capture memory baseline on first stats update (after Startup has run)
        if stats.memory_mb > 0.0 {
            if let Some(mut profile) = world.get_resource_mut::<MemoryProfile>() {
                profile.capture_baseline(stats.memory_mb);
            }
        }
    }

    // Write cross-crate DebugSnapshot for MCP
    // Extract profiler data first (before mutable world access)
    let (update_profiles, startup_profiles) = world
        .get_resource::<SystemProfiler>()
        .map(|p| {
            let up: Vec<_> = p
                .get_top_n_update(10)
                .into_iter()
                .map(|(n, d)| (n, d.as_secs_f64() * 1000.0))
                .collect();
            let sp: Vec<_> = p
                .get_top_n_startup(10)
                .into_iter()
                .map(|(n, d)| (n, d.as_secs_f64() * 1000.0))
                .collect();
            (up, sp)
        })
        .unwrap_or_default();

    let mut asset_counts: Vec<_> = stats
        .asset_counts
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    asset_counts.sort_by(|a, b| a.0.cmp(&b.0));

    // Extract memory profiling data
    let (
        total_schedule_systems,
        python_gc_objects,
        memory_growth_mb,
        memory_peak_mb,
        memory_warning,
        reload_memory_snapshots,
    ) = world
        .get_resource::<MemoryProfile>()
        .map(|profile| {
            let current_rss = stats.memory_mb;
            let snapshots = profile
                .snapshots
                .iter()
                .map(|s| pybevy_core::ReloadMemorySnapshotInfo {
                    generation: s.generation,
                    rss_mb: s.rss_mb,
                    delta_mb: s.delta_mb,
                    gc_objects: s.gc_objects,
                    schedule_systems: s.schedule_systems,
                })
                .collect();
            (
                profile
                    .snapshots
                    .last()
                    .map(|s| s.schedule_systems)
                    .unwrap_or(0),
                profile.snapshots.last().map(|s| s.gc_objects).unwrap_or(0),
                profile.growth_mb(current_rss),
                profile.peak_rss_mb,
                profile.is_warning(current_rss),
                snapshots,
            )
        })
        .unwrap_or_default();

    let snapshot = pybevy_core::DebugSnapshot {
        populated: true,
        reload_count: stats.reload_count,
        last_reload_mode: stats.last_mode.map(|m| match m {
            ReloadMode::Full => "full".to_string(),
            ReloadMode::Partial => "partial".to_string(),
        }),
        reload_failed: stats.last_reload_failed,
        reload_failure_reason: stats.last_failure_reason.clone(),
        memory_mb: stats.memory_mb,
        total_memory_mb: stats.total_memory_mb,
        cpu_percent: stats.cpu_percent,
        cpu_core_count: stats.cpu_core_count,
        fps_average: stats.fps_average,
        fps_current: stats.fps_current,
        uptime_secs: stats.uptime_secs,
        entity_count: stats.entity_count,
        gil_enabled: stats.gil_enabled,
        asset_counts,
        update_profiles,
        startup_profiles,
        total_schedule_systems,
        python_gc_objects,
        memory_growth_mb,
        memory_peak_mb,
        memory_warning,
        reload_memory_snapshots,
    };

    world.insert_resource(snapshot);

    // Re-insert resources
    world.insert_resource(monitor);
    world.insert_resource(stats);
}

/// System that updates the hot reload overlay text
fn render_hot_reload_overlay(
    mut query: bevy::ecs::system::Query<&mut Text, bevy::ecs::query::With<HotReloadOverlayText>>,
    mut error_query: bevy::ecs::system::Query<
        (&mut Text, &mut bevy::prelude::Visibility),
        (
            bevy::ecs::query::With<HotReloadErrorText>,
            bevy::ecs::query::Without<HotReloadOverlayText>,
        ),
    >,
    monitor: Option<bevy::ecs::system::ResMut<SystemMonitor>>,
    stats: Option<bevy::ecs::system::ResMut<HotReloadStats>>,
    profiler: Option<bevy::ecs::system::Res<SystemProfiler>>,
    time: Option<bevy::ecs::system::Res<bevy::time::Time>>,
    last_error: Option<bevy::ecs::system::Res<pybevy_core::LastSystemError>>,
    memory_profile: Option<bevy::ecs::system::Res<MemoryProfile>>,
    memory_visible: Option<bevy::ecs::system::Res<MemoryOverlayVisible>>,
    start_paused: Option<bevy::ecs::system::Res<StartPaused>>,
) {
    // Skip if resources aren't available
    let (Some(mut monitor), Some(mut stats), Some(time)) = (monitor, stats, time) else {
        return;
    };

    let current_time = time.elapsed_secs_f64();

    // Throttle updates to 4 times per second (every 250ms) for readability
    const RENDER_INTERVAL: f64 = 0.25;
    if current_time - monitor.last_render_update < RENDER_INTERVAL {
        return;
    }
    monitor.last_render_update = current_time;

    let is_paused = start_paused.as_ref().is_some_and(|p| p.0);

    for mut text in query.iter_mut() {
        let last_mode_str = if stats.last_reload_failed {
            "FAILED (prev gen)"
        } else {
            match stats.last_mode {
                Some(ReloadMode::Full) => "Full",
                Some(ReloadMode::Partial) => "Partial",
                None => "--",
            }
        };

        // Format uptime (e.g., "1m23s")
        let uptime = format_uptime(stats.uptime_secs);

        // GIL status display
        let gil_str = if stats.gil_enabled {
            "GIL"
        } else {
            "Free-threaded"
        };

        // Format asset counts
        let assets_str = if stats.asset_counts.is_empty() {
            "Assets: --".to_string()
        } else {
            let mut counts: Vec<_> = stats.asset_counts.iter().collect();
            counts.sort_by_key(|(name, _)| *name);
            let formatted = counts
                .into_iter()
                .map(|(name, count)| format!("{}:{}", name, count))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Assets: {}", formatted)
        };

        // Line 1: System info

        let line1 = format!(
            "Reloads: {} | Last: {} | RAM: {:.0}/{:.0}MB | CPU: {:.1}% ({}c) | {} | Up: {} | FPS: {:.0}/{:.0} | Entities: {} | {}",
            stats.reload_count,
            last_mode_str,
            stats.memory_mb,
            stats.total_memory_mb,
            stats.cpu_percent,
            stats.cpu_core_count,
            gil_str,
            uptime,
            stats.fps_average,
            stats.fps_current,
            stats.entity_count,
            assets_str,
        );

        // Line 2: Update/Last systems profile
        let line2 = if let Some(p) = profiler.as_ref() {
            let top5 = p.get_top_n_update(5);
            if !top5.is_empty() {
                let systems = top5
                    .into_iter()
                    .map(|(n, d)| format!("{}({:.2}ms)", n, d.as_secs_f64() * 1000.0))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("Profile: {}", systems)
            } else {
                "Profile: --".to_string()
            }
        } else {
            "Profile: --".to_string()
        };

        // Line 3: Startup systems (first 5s only)
        let line3 = if let Some(p) = profiler.as_ref() {
            if p.should_show_startup(current_time) {
                let top5 = p.get_top_n_startup(5);
                if !top5.is_empty() {
                    let systems = top5
                        .into_iter()
                        .map(|(n, d)| format!("{}({:.2}ms)", n, d.as_secs_f64() * 1000.0))
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("Startup: {}", systems)
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Line 4: Memory profiling (only when F7 toggle is active)
        let line4 = if memory_visible.as_ref().is_some_and(|v| v.0) {
            if let Some(profile) = memory_profile.as_ref() {
                let mut parts = Vec::new();
                if profile.baseline_captured {
                    let growth = profile.growth_mb(stats.memory_mb);
                    let warning = if profile.is_warning(stats.memory_mb) {
                        " WARN"
                    } else {
                        ""
                    };
                    parts.push(format!(
                        "Growth: {:.1}MB | Peak: {:.1}MB{}",
                        growth, profile.peak_rss_mb, warning
                    ));
                }
                // Show last few reload deltas as a mini trend
                let recent: Vec<_> = profile.snapshots.iter().rev().take(5).collect();
                if !recent.is_empty() {
                    let trend = recent
                        .iter()
                        .rev()
                        .map(|s| {
                            let sign = if s.delta_mb >= 0.0 { "+" } else { "" };
                            format!("g{}:{}{:.1}", s.generation, sign, s.delta_mb)
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    parts.push(format!("Trend: [{}]", trend));

                    // Show GC + systems from latest snapshot
                    if let Some(latest) = profile.snapshots.last() {
                        parts.push(format!(
                            "GC: {} | Systems: {}",
                            latest.gc_objects, latest.schedule_systems
                        ));
                    }
                }
                if parts.is_empty() {
                    "Memory: awaiting baseline...".to_string()
                } else {
                    format!("Memory: {}", parts.join(" | "))
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Combine lines (line3/line4 may be empty)
        let mut output = if is_paused {
            format!(
                "PAUSED — press Space to load | F5=Full reload\n{}\n{}",
                line1, line2
            )
        } else {
            format!("{}\n{}", line1, line2)
        };
        if !line3.is_empty() {
            output.push('\n');
            output.push_str(&line3);
        }
        if !line4.is_empty() {
            output.push('\n');
            output.push_str(&line4);
        }
        text.0 = output;
    }

    // Update error text overlay
    for (mut error_text, mut visibility) in error_query.iter_mut() {
        let has_error = last_error.as_ref().and_then(|e| e.error.as_ref()).is_some();

        if let Some(last_err) = last_error.as_ref() {
            if last_err.error.is_some() && last_err.timestamp_secs > stats.last_error_timestamp {
                // Extract the meaningful error line from traceback or error message
                let error_msg = last_err
                    .traceback
                    .as_deref()
                    .and_then(|tb| {
                        // Get the last non-empty line (the actual error)
                        tb.lines().rev().find(|l| !l.trim().is_empty())
                    })
                    .or(last_err.error.as_deref())
                    .unwrap_or("Unknown error");

                let display_msg = if error_msg.len() > 120 {
                    format!("Error: {}...", &error_msg[..117])
                } else {
                    format!("Error: {}", error_msg)
                };

                error_text.0 = display_msg;
                stats.last_error_timestamp = last_err.timestamp_secs;
            }
        }

        *visibility = if has_error {
            bevy::prelude::Visibility::Inherited
        } else {
            bevy::prelude::Visibility::Hidden
        };
    }
}

/// Format uptime in human-readable format (e.g., "1m23s")
fn format_uptime(secs: f64) -> String {
    let total_secs = secs as u32;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if mins > 0 {
        format!("{}m{}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Add hot reload support to an app
/// This function is idempotent - calling it multiple times is safe (subsequent calls are no-ops)
pub fn add_hot_reload_system(
    app: &mut App,
    state: HotReloadState,
    error_state: Arc<Mutex<Vec<PyErr>>>,
) {
    let verbose = is_verbose();

    if verbose {
        eprintln!("🔧 [Hot Reload] add_hot_reload_system called");
    }

    // Check if hot reload is already set up
    if app.world().contains_resource::<HotReloadResource>() {
        if verbose {
            eprintln!("   → Hot reload already initialized, skipping");
        }
        return; // Already initialized, skip
    }

    if verbose {
        eprintln!("   → Initializing hot reload system...");
    }

    // Insert the reload resource
    app.insert_resource(HotReloadResource::new(state.clone(), error_state));

    // Insert the generation tracking resource
    let generation_counter = state.generation();
    app.insert_resource(HotReloadGeneration::new(generation_counter));

    // Insert the DynamicSystem registry for tracking system handles by generation
    app.insert_resource(DynamicSystemRegistry::default());

    // Initialize system monitor
    let mut system = sysinfo::System::new();
    let process_pid = match sysinfo::get_current_pid() {
        Ok(pid) => {
            if verbose {
                eprintln!("   → Monitoring process PID: {}", pid);
            }
            Some(pid)
        }
        Err(e) => {
            eprintln!(
                "   → WARNING: Could not get process PID: {}. Process monitoring disabled.",
                e
            );
            None
        }
    };
    if verbose {
        eprintln!("   → CPU/Memory update interval: 1.0s");
    }

    // Initialize CPU tracking (required on Linux for per-process CPU measurements)
    system.refresh_cpu_all();

    // Initialize memory tracking (required for total_memory() to return correct values)
    system.refresh_memory();

    // Initial process stats - only if we have a valid PID
    let (initial_memory_mb, initial_cpu) = if let Some(pid) = process_pid {
        // Initial process refresh with explicit CPU tracking (establishes baseline for CPU calculation)
        use sysinfo::ProcessRefreshKind;
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[pid]),
            false,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );

        if let Some(process) = system.process(pid) {
            let per_core_cpu = process.cpu_usage();
            let num_cores = system.cpus().len().max(1) as f32;
            let total_cpu = per_core_cpu / num_cores;
            let mem_mb = process.memory() as f64 / 1_048_576.0;
            if verbose {
                eprintln!(
                    "   → Initial stats: Memory={:.1}MB, CPU={:.1}% (total), Cores={}",
                    mem_mb, total_cpu, num_cores
                );
            }
            (mem_mb, total_cpu)
        } else {
            eprintln!("WARNING: Could not find process {}!", pid);
            (0.0, 0.0)
        }
    } else {
        (0.0, 0.0)
    };

    // Calculate additional system info for stats
    let total_memory_mb = system.total_memory() as f64 / 1_048_576.0;
    let cpu_core_count = system.cpus().len();
    let gil_enabled = detect_gil_status();

    if verbose {
        eprintln!(
            "   → System: {:.0}MB RAM, {} CPU cores, GIL: {}",
            total_memory_mb,
            cpu_core_count,
            if gil_enabled { "enabled" } else { "disabled" }
        );
    }

    // Insert the system monitor resource
    // Set last_update to -1.0 to trigger first update on the first frame
    app.insert_resource(SystemMonitor {
        system,
        process_pid,
        last_update: -1.0,
        fps_history: std::collections::VecDeque::with_capacity(60),
        last_render_update: -1.0,
    });

    // Insert the hot reload statistics resource
    app.insert_resource(HotReloadStats {
        last_mode: None,
        last_reload_time: 0.0,
        reload_count: 0,
        default_mode: ReloadMode::Partial, // Default to partial reload for file changes
        memory_mb: initial_memory_mb,
        cpu_percent: initial_cpu,
        fps_average: 0.0,
        fps_current: 0.0,
        total_memory_mb,
        cpu_core_count,
        gil_enabled,
        uptime_secs: 0.0,
        entity_count: 0,
        asset_counts: std::collections::HashMap::new(),
        last_error_timestamp: 0.0,
        last_reload_frame: 0,
        last_reload_failed: false,
        last_failure_reason: None,
    });

    // Insert the system profiler for performance tracking
    // Uses 60-frame rolling average (1 second at 60fps)
    app.insert_resource(SystemProfiler::new(60));
    if verbose {
        eprintln!("   → System profiler enabled (60-frame rolling average)");
    }

    // Insert the memory profiling resource for per-reload tracking
    app.insert_resource(MemoryProfile::default());
    app.insert_resource(MemoryOverlayVisible(false));
    if verbose {
        eprintln!("   → Memory profiler enabled (F7 to toggle overlay)");
    }

    // Insert plugin tracker for delta detection across reloads
    app.insert_resource(PluginTracker::default());

    // Check for --pause flag (communicated via env var)
    let start_paused = env::var("PYBEVY_START_PAUSED")
        .map(|v| v == "1")
        .unwrap_or(false);
    app.insert_resource(StartPaused(start_paused));
    if start_paused {
        // Increment generation so gen-0 user systems are disabled.
        // They won't run until Space triggers a full reload at gen-1.
        state.increment_generation();
        let mut gen_res = app.world_mut().resource_mut::<HotReloadGeneration>();
        gen_res.update();
        if verbose {
            eprintln!("   → Start paused: user systems disabled until Space is pressed");
        }
    }

    // Check for --resolution flag (communicated via env var, e.g. "1920x1080")
    if let Ok(res_str) = env::var("PYBEVY_WINDOW_RESOLUTION") {
        if let Some((width, height)) = parse_resolution(&res_str) {
            // Window entity doesn't exist yet (plugins build during app.run()),
            // so register a PreStartup system to set resolution once it's available.
            app.add_systems(
                PreStartup,
                move |mut query: bevy::ecs::system::Query<
                    &mut bevy::window::Window,
                    bevy::ecs::query::With<bevy::window::PrimaryWindow>,
                >| {
                    if let Ok(mut window) = query.single_mut() {
                        window.resolution.set(width, height);
                    }
                },
            );
            if verbose {
                eprintln!("   → Window resolution will be set to {}x{}", width, height);
            }
        }
    }

    // Insert the Python-accessible hot reload control resource
    // This allows Python systems to request reloads (e.g., on F5 press)
    Python::attach(|py| {
        use bevy::ecs::component::ComponentId;
        use pyo3::types::PyType;

        use crate::ecs::resource_type::{PyResourceStorage, ResourceRegistry};

        let control = PyHotReloadControl::new(state.clone());
        let py_control = match Py::new(py, control) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "WARNING: Failed to create HotReloadControl: {}. Python reload API disabled.",
                    e
                );
                return;
            }
        };

        // Get the type pointer for HotReloadControl
        // PyHotReloadControl extends PyResource, so we need to get the Python class type
        let type_obj: Bound<'_, PyType> = py_control.bind(py).get_type();
        let type_ptr = type_obj.as_type_ptr();

        // Ensure ResourceRegistry exists
        if !app.world().contains_resource::<ResourceRegistry>() {
            app.insert_resource(ResourceRegistry::default());
        }

        // Ensure PyResourceStorage exists
        if !app.world().contains_resource::<PyResourceStorage>() {
            app.insert_resource(PyResourceStorage::default());
        }

        // Register a ComponentId for this resource type
        let component_id = {
            let mut registry = app.world_mut().resource_mut::<ResourceRegistry>();
            // Get current registry size before the mutable borrow in or_insert_with
            let next_id = registry.registry.len();
            *registry.registry.entry(type_ptr).or_insert_with(|| {
                // Create a unique ComponentId for this resource
                ComponentId::new(next_id)
            })
        };

        // Store the Python object in PyResourceStorage
        let mut storage = app.world_mut().resource_mut::<PyResourceStorage>();
        storage
            .resources
            .insert(component_id, py_control.into_any());
    });

    // Add the F5 handler and check system to run every frame in Last schedule
    // We use Last so it runs after Update systems, giving a consistent reload point
    // handle_f5_reload_system runs first to detect F5 and set reload mode
    // check_hot_reload_system runs after to process any pending reload
    // update_system_stats updates memory and CPU stats periodically
    // render_hot_reload_overlay renders the status overlay

    // Ensure Last schedule exists (might not exist with minimal plugins like ScheduleRunnerPlugin)
    if !app.world().resource::<Schedules>().contains(Last) {
        if verbose {
            eprintln!("   → Adding Last schedule (not present in app)");
        }
        app.init_schedule(Last);
    }

    app.add_systems(
        Last,
        (
            handle_f5_reload_system,
            check_hot_reload_system,
            update_system_stats,
            render_hot_reload_overlay,
        )
            .chain(),
    );

    // Spawn the overlay UI entity immediately (plugins are already initialized at this point)
    spawn_hot_reload_overlay_system(app.world_mut());
}

/// PyO3 plugin for enabling hot reload functionality
///
/// This plugin sets up the hot reload system, including:
/// - HotReloadControl resource for requesting reloads from Python
/// - HotReloadGeneration resource for tracking reload generations
/// - F5 key handler and reload check systems
///
/// # Example
/// ```python
/// from pybevy.app import App, HotReloadPlugin
///
/// app = App()
/// app.add_plugins(HotReloadPlugin())
/// ```
#[pyclass(name = "HotReloadPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyHotReloadPlugin;

#[pymethods]
impl PyHotReloadPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyHotReloadPlugin, PyPlugin)
    }

    pub fn build(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        app.borrow().with_bevy_app(|bevy_app| {
            // Create the hot reload state
            let state = HotReloadState::new();
            let error_state = Arc::new(Mutex::new(Vec::new()));

            // Add hot reload systems and resources
            add_hot_reload_system(bevy_app, state, error_state);

            Ok(())
        })
    }
}
