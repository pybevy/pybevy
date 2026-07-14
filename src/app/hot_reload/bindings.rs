use std::{
    env,
    sync::{Arc, Mutex},
};

use bevy::{
    app::{App, Last, MainScheduleOrder, PreStartup},
    ecs::schedule::{IntoScheduleConfigs, ScheduleLabel, Schedules},
};
use pybevy_core::PyPlugin;
use pybevy_reload::{
    HotReloadGeneration, HotReloadStats, MemoryOverlayVisible, MemoryProfile, PluginTracker,
    ReloadMode, StartPaused, SystemMonitor, SystemProfiler, is_verbose, lock_or_recover,
    parse_resolution, render_hot_reload_overlay, spawn_hot_reload_overlay_system,
    update_system_stats,
};
use pyo3::prelude::*;

use super::{
    registry::DynamicSystemRegistry,
    state::{HotReloadResource, HotReloadState},
    systems::{check_hot_reload_system, handle_f5_reload_system},
    util::detect_gil_status,
};
use crate::{
    app::app::PyApp,
    ecs::{
        resource::PyResource,
        resource_type::{PyResourceStorage, register_custom_resource},
    },
};

/// Python-exposed reload state class (for CLI watcher thread)
#[pyclass(name = "AppReloadState")]
pub struct PyAppReloadState {
    state: HotReloadState,
}

impl PyAppReloadState {
    pub fn new(state: HotReloadState) -> Self {
        Self { state }
    }
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

/// Python-exposed hot reload control resource (for in-game systems)
/// This allows Python systems to request reloads dynamically (e.g., on F5 press)
#[pyclass(name = "HotReloadControl", extends = PyResource, frozen, skip_from_py_object)]
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
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[pid]),
            false,
            sysinfo::ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory(),
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
    if let Ok(res_str) = env::var("PYBEVY_WINDOW_RESOLUTION")
        && let Some((width, height)) = parse_resolution(&res_str)
    {
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

    // Insert the Python-accessible hot reload control resource
    // This allows Python systems to request reloads (e.g., on F5 press)
    Python::attach(|py| {
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
        let type_obj: Bound<'_, pyo3::types::PyType> = py_control.bind(py).get_type();
        let type_ptr = type_obj.as_type_ptr();

        // Ensure PyResourceStorage exists
        if !app.world().contains_resource::<PyResourceStorage>() {
            app.insert_resource(PyResourceStorage::default());
        }

        // Use the same stable, qualified-name-aware identity path as all other
        // custom resources. Ad-hoc numeric ComponentIds can collide with Bevy's
        // own component registry.
        let component_id = register_custom_resource(
            app.world_mut(),
            type_ptr,
            type_obj
                .name()
                .map(|name| name.to_string())
                .unwrap_or_else(|_| "HotReloadControl".to_string()),
        );

        // Store the Python object in PyResourceStorage
        let mut storage = app.world_mut().resource_mut::<PyResourceStorage>();
        storage
            .resources
            .insert(component_id, py_control.into_any());
    });

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

    // Capture base entity set NOW — before any user Startup systems run.
    // Every entity that exists at this point is Bevy-internal (plugin init).
    // On Full reload, everything NOT in this set gets despawned.
    if !app
        .world()
        .contains_resource::<pybevy_reload::BaseEntitySet>()
    {
        let entities: std::collections::HashSet<bevy::ecs::entity::Entity> = app
            .world_mut()
            .query::<bevy::ecs::entity::Entity>()
            .iter(app.world())
            .collect();
        if verbose {
            eprintln!(
                "   → Captured BaseEntitySet with {} entities",
                entities.len()
            );
        }
        app.insert_resource(pybevy_reload::BaseEntitySet { entities });

        // Winit creates more engine entities (Monitors, a11y) when the event
        // loop starts, after the snapshot above. Fold everything alive at
        // startup into the baseline from a dedicated schedule that runs
        // before PreStartup, so ordering against user PreStartup systems is
        // deterministic.
        app.init_schedule(ExtendBaseEntitySet);
        app.world_mut()
            .resource_mut::<MainScheduleOrder>()
            .insert_startup_before(PreStartup, ExtendBaseEntitySet);
        app.add_systems(
            ExtendBaseEntitySet,
            pybevy_reload::cleanup::extend_base_entity_set,
        );
    }
}

/// Schedule that runs once before `PreStartup` to fold engine entities
/// created at event-loop start into the `BaseEntitySet` baseline.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct ExtendBaseEntitySet;

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
#[pyclass(name = "HotReloadPlugin", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyHotReloadPlugin;

#[pymethods]
impl PyHotReloadPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyHotReloadPlugin, PyPlugin).into()
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
