use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    mem,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::ThreadId,
};

use bevy::{
    app::{App, AppExit, Last, PreStartup},
    ecs::{
        message::MessageWriter,
        resource::Resource,
        schedule::{Chain, ScheduleConfigs, Schedules, SingleThreadedExecutor},
        system::{Local, Res},
        world::World,
    },
    log::LogPlugin,
    time::{Real, Time},
};
use pybevy_core::{
    ActiveSceneModule, AppId, AppLifecycle, AppOperation, AppStoreCore, AppStoreError,
    LastSystemError, PluginIdentity, PyPlugin as PyPluginBase,
    added_plugins::AddedPythonPlugins,
    allocate_id, consume_unstored_id,
    plugin::plugin_registry,
    public_error::{duplicate_plugin_identity, plugin_key_type},
    register_wrapped_reflect_types,
};
use pybevy_ecs::shared::schedule::{
    StateScheduleLabel, TransitionScheduleLabel, configure_standard_schedules,
};
use pybevy_reload::{HotReloadGeneration, PluginTracker, SystemStage, is_verbose};
use pyo3::{
    IntoPyObjectExt, PyTraverseError, PyVisit,
    exceptions::{PyAttributeError, PyRuntimeError, PyTypeError, PyValueError},
    ffi::PyTypeObject,
    prelude::*,
    types::{PyList, PyModule, PyTuple, PyType},
};

use crate::{
    app::{
        PyStage,
        app_exit::materialize_app_exit,
        chained_systems::{PyChainedSystemSets, PyChainedSystems},
        error_messages,
        hot_reload::{
            bindings::{PyAppReloadState, add_hot_reload_system},
            cleanup::clear_entities_and_resources,
            registry::DynamicSystemRegistry,
            runtime_pyo3::{annotate_registration_error, collect_system_names},
            state::HotReloadState,
        },
        plugin::{PyPlugin, PyPluginGroup},
        plugins::{PyDefaultPlugins, PyPluginGroupBuilder},
    },
    ecs::{
        dynamic_system::{
            LastErrorBuffer, SystemErrorBuffer, clear_system_param_cache, lock_or_recover,
        },
        messages::ensure_builtin_message_resources,
        observer_registry::ObserverRegistry,
        python_message::{install_python_message_store, register_python_message},
        resource_type::PyResourceType,
        state::{
            PyNextState, PyOnEnterSchedule, PyOnExitSchedule, PyOnTransitionSchedule, PyState,
            canonicalize_state_schedule_label, canonicalize_transition_schedule_label,
            ensure_state_transition_system_registered, insert_state_machine_resources,
            state_member_type,
        },
        system_config::{
            InstalledSystemSetConfigs, build_scheduled_system, build_set_config,
            system_set_config_identity,
        },
        system_interpreter::ObserverRuntimeSinks,
        world::PyWorld,
    },
};

const DEFAULT_LOG_FILTER: &str = "bevy=warn,wgpu=error,naga=warn,winit=warn";

/// Global flag to track whether LogPlugin has been initialized
/// LogPlugin sets up a global logger/tracing subscriber, which can only be set once per process
/// This prevents the "already set" error when creating multiple App instances (e.g., in tests)
static LOG_PLUGIN_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub(crate) struct PendingStateDefinition {
    pub state_type: Py<PyType>,
    pub initial_state: Py<PyAny>,
}

pub(crate) struct PendingStateSystems {
    pub schedule: Py<PyAny>,
    pub systems: Vec<Py<PyAny>>,
}

thread_local! {
    /// Thread-local storage for Bevy Apps, which are not Send when they own winit.
    static BEVY_APPS: RefCell<AppStoreCore> = RefCell::new(AppStoreCore::new());
}

/// Cleanup function called during Python shutdown via atexit handler.
/// This explicitly drains all active Apps from thread-local storage BEFORE Python's
/// thread-local destructors run, preventing crashes when Apps with Python
/// resources try to access already-destroyed PyO3 thread-locals during cleanup.
///
/// CRITICAL: This must be called before Python's TLS destructors run, otherwise
/// resources containing Python objects (custom components, messages) will panic
/// when trying to acquire the GIL or access Python state during drop.
#[pyfunction]
pub(crate) fn cleanup_apps_on_shutdown() {
    let outcome = BEVY_APPS.with(|apps_cell| apps_cell.borrow_mut().drain_active());
    let (apps, borrowed) = outcome.into_parts();
    drop(apps);
    for (app_id, operation) in borrowed {
        eprintln!("WARNING: App {app_id} is still executing {operation} during shutdown");
    }
}

/// TEST ONLY: Get the count of Apps currently in thread-local storage.
/// Used to verify that atexit cleanup works correctly.
#[pyfunction]
pub(crate) fn _test_get_app_count() -> usize {
    BEVY_APPS.with(|apps_cell| apps_cell.borrow().active_count())
}

/// TEST ONLY: Force immediate cleanup of all Apps in thread-local storage.
/// This simulates the cleanup that should happen via atexit handler.
/// Used to test that cleanup works correctly with Python resources.
#[pyfunction]
pub(crate) fn _test_force_cleanup() {
    cleanup_apps_on_shutdown();
}

/// Raise collected system errors: single error preserves its type,
/// multiple errors are wrapped in an ExceptionGroup.
fn raise_collected_errors(py: Python<'_>, error_state: &Arc<Mutex<Vec<PyErr>>>) -> PyResult<()> {
    let errors = {
        let mut error_guard = lock_or_recover(error_state);
        std::mem::take(&mut *error_guard)
    };
    if errors.is_empty() {
        return Ok(());
    }
    if errors.len() == 1 {
        return Err(errors.into_iter().next().unwrap());
    }
    // Multiple errors: create ExceptionGroup
    let exceptions = PyList::empty(py);
    for err in errors {
        exceptions.append(err.value(py))?;
    }
    let builtins = PyModule::import(py, "builtins")?;
    let exc_group_type = builtins.getattr("ExceptionGroup")?;
    let group = exc_group_type.call1(("system errors", exceptions))?;
    Err(PyErr::from_value(group))
}

/// Bevy resource that shares the error state Arc with the `run()` loop.
/// A `Last`-schedule system checks this and sends `AppExit` when errors are present
/// so that `app.run()` exits and we can raise the error in Python.
#[derive(Resource)]
struct SystemErrorCheck {
    errors: Arc<Mutex<Vec<PyErr>>>,
}

#[derive(Resource)]
struct MaxFrames(u64);

fn exit_after_max_frames(
    max_frames: Res<MaxFrames>,
    mut completed_frames: Local<u64>,
    mut exit: MessageWriter<AppExit>,
) {
    *completed_frames += 1;
    if *completed_frames >= max_frames.0 {
        exit.write(AppExit::Success);
    }
}

fn max_frames_from_env() -> PyResult<Option<u64>> {
    let value = match std::env::var("PYBEVY_MAX_FRAMES") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(PyValueError::new_err(
                "PYBEVY_MAX_FRAMES must be a positive integer",
            ));
        }
    };
    let frames = value
        .parse::<u64>()
        .map_err(|_| PyValueError::new_err("PYBEVY_MAX_FRAMES must be a positive integer"))?;
    if frames == 0 {
        return Err(PyValueError::new_err(
            "PYBEVY_MAX_FRAMES must be a positive integer",
        ));
    }
    Ok(Some(frames))
}

fn check_system_errors_and_exit(
    error_check: Res<SystemErrorCheck>,
    mut exit: MessageWriter<AppExit>,
) {
    let lock = lock_or_recover(&error_check.errors);
    if !lock.is_empty() {
        exit.write(AppExit::from_code(1));
    }
}

/// Move the most recent buffered Python system error into `LastSystemError` for
/// MCP. Runs in `Last`. The timestamp is read here at drain time, not when the
/// error occurred, so it can lag one frame; acceptable for MCP display and it
/// keeps `run_unsafe`'s parallel error path free of any world access.
pub(crate) fn drain_last_system_error(world: &mut World) {
    let buffered = {
        let Some(buf) = world.get_resource::<LastErrorBuffer>() else {
            return;
        };
        let mut guard = buf.buffer.lock().unwrap_or_else(|p| p.into_inner());
        guard.take()
    };
    let Some(err) = buffered else {
        return;
    };
    let timestamp = world
        .get_resource::<Time<Real>>()
        .map(|t| t.elapsed_secs_f64())
        .unwrap_or(0.0);
    if let Some(mut last_error) = world.get_resource_mut::<LastSystemError>() {
        last_error.error = Some(err.error);
        last_error.traceback = err.traceback;
        last_error.timestamp_secs = timestamp;
    }
}

fn plugin_qualified_name(type_ptr: *const PyTypeObject, py: Python) -> Option<String> {
    // SAFETY: registered type pointers live for the interpreter lifetime.
    let py_type = unsafe { Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };
    if let Ok(cls) = py_type.cast::<PyType>() {
        let module = cls.getattr("__module__").ok()?.extract::<String>().ok()?;
        let qualname = cls.getattr("__qualname__").ok()?.extract::<String>().ok()?;
        Some(format!("{}.{}", module, qualname))
    } else {
        None
    }
}

fn plugin_instance_key(
    plugin: &Bound<'_, PyAny>,
    qualified_name: &str,
) -> PyResult<Option<String>> {
    let key = match plugin.getattr("__pybevy_plugin_key__") {
        Ok(key) => key,
        Err(error) if error.is_instance_of::<PyAttributeError>(plugin.py()) => return Ok(None),
        Err(error) => return Err(error),
    };
    if key.is_none() {
        return Ok(None);
    }
    key.extract::<String>().map(Some).map_err(|_| {
        let received_type = key
            .get_type()
            .name()
            .and_then(|name| name.extract::<String>())
            .unwrap_or_else(|_| "unknown".to_string());
        PyTypeError::new_err(plugin_key_type(qualified_name, received_type))
    })
}

fn app_store_error(error: AppStoreError) -> PyErr {
    match error {
        AppStoreError::Borrowed(operation) => {
            PyRuntimeError::new_err(format!("App is already executing {operation}"))
        }
        AppStoreError::Consumed => {
            PyRuntimeError::new_err("Cannot perform operation after run() has been called")
        }
        AppStoreError::Missing(_) | AppStoreError::Removed => PyRuntimeError::new_err(
            "App is not initialized. This may occur if the app was accessed from a \
             different thread, or if there was an error during app initialization.",
        ),
        other => PyRuntimeError::new_err(other.to_string()),
    }
}

fn begin_main_app_operation(
    app_id: AppId,
    operation: AppOperation,
) -> PyResult<MainAppOperationGuard> {
    let app = BEVY_APPS
        .with(|apps_cell| apps_cell.borrow_mut().begin_operation(app_id, operation))
        .map_err(app_store_error)?;
    Ok(MainAppOperationGuard {
        app_id,
        app: Some(app),
    })
}

struct MainAppOperationGuard {
    app_id: AppId,
    app: Option<App>,
}

impl MainAppOperationGuard {
    fn app_mut(&mut self) -> &mut App {
        self.app
            .as_mut()
            .expect("Main App operation guard owns its App until drop")
    }

    fn finish_consumed(mut self) {
        let app_id = self.app_id;
        let app = self.app.take().expect("consuming operation owns one App");
        let result =
            BEVY_APPS.with(|apps_cell| apps_cell.borrow_mut().finish_operation_consumed(app_id));
        if let Err(error) = result {
            mem::forget(app);
            mem::forget(self);
            panic!("failed to consume App {app_id}: {error}");
        }
        drop(app);
        mem::forget(self);
    }
}

impl Drop for MainAppOperationGuard {
    fn drop(&mut self) {
        let app = self
            .app
            .take()
            .expect("Main App operation guard cannot restore twice");
        let restored =
            BEVY_APPS.with(|apps_cell| apps_cell.borrow_mut().restore_operation(self.app_id, app));
        if let Err(error) = restored {
            let (state_error, app) = error.into_parts();
            mem::forget(app);
            panic!("failed to restore App {}: {state_error}", self.app_id);
        }
    }
}

#[pyclass(name = "App", module = "pybevy.app", unsendable)]
pub struct PyApp {
    /// Unique identifier for this PyApp instance.
    app_id: AppId,

    /// Thread ID where this PyApp was created
    /// Used to detect cross-thread drops and prevent memory leaks
    creation_thread: ThreadId,

    /// Registry of plugin types that have been added (by pointer for fast lookup,
    /// by name for hot-reload resilience when Python classes get new type pointers)
    plugin_registry: RefCell<AddedPythonPlugins>,

    /// Shared error state for collecting system errors (parameter + execution)
    /// Arc allows sharing with DynamicSystem instances, Mutex for thread-safe access
    system_error: Arc<Mutex<Vec<PyErr>>>,

    /// Shared slot holding the most recent Python system error's message/traceback.
    /// Cloned into every DynamicSystem and into the world's `LastErrorBuffer`
    /// resource; the `Last`-schedule drain moves it into `LastSystemError`.
    system_error_buffer: SystemErrorBuffer,

    /// Last exit requested by an update or returned by the consuming runner.
    ///
    /// This is kept outside the Bevy App so `should_exit()` remains available
    /// after `run()` moves the App into its runner and consumes the store slot.
    last_exit: Arc<Mutex<Option<AppExit>>>,

    /// Hot reload state for development mode
    /// Allows CLI watcher to trigger reloads
    hot_reload_state: HotReloadState,

    /// Flag indicating this is a temporary app for hot reload system extraction
    /// When true, plugin additions are skipped
    is_reload_temp: Cell<bool>,

    /// Storage for system definitions during hot reload
    /// When is_reload_temp=true, systems are stored here instead of added to Bevy
    pending_systems: RefCell<Vec<(PyStage, Vec<Py<PyAny>>)>>,

    /// System-set configurations collected while loading a reload generation.
    pending_set_configs: RefCell<Vec<(PyStage, Vec<Py<PyAny>>)>>,

    /// Storage for resource instances during hot reload
    /// When is_reload_temp=true, resources are stored here instead of added to Bevy
    pending_resources: RefCell<Vec<Py<PyAny>>>,

    /// State declarations collected from init_state/insert_state during reload.
    pending_states: RefCell<Vec<PendingStateDefinition>>,

    /// State-schedule systems collected during reload for generation-aware registration.
    pending_state_systems: RefCell<Vec<PendingStateSystems>>,

    /// Storage for message types during hot reload
    /// When is_reload_temp=true, message types are stored here for re-registration
    pending_messages: RefCell<Vec<Py<PyType>>>,

    /// Storage for observer functions during hot reload
    /// When is_reload_temp=true, observer functions are stored here for re-registration
    pending_observers: RefCell<Vec<Py<PyAny>>>,

    /// Stable plugin identities used to seed and compare hot-reload definitions.
    pending_plugins: RefCell<Vec<PluginIdentity>>,

    pending_system_names: RefCell<HashSet<String>>,

    /// Whether @entrypoint decorator has been applied
    /// run() requires this unless PYBEVY_TESTING env var is set
    entrypoint_set: Cell<bool>,
}

impl PyApp {
    /// Check the authoritative store lifecycle before an adapter-only operation.
    fn ensure_active(&self) -> PyResult<()> {
        if self.is_reload_temp.get() {
            return Ok(());
        }
        BEVY_APPS
            .with(|apps_cell| apps_cell.borrow().state(self.app_id))
            .and_then(|state| match state {
                AppLifecycle::Active => Ok(()),
                AppLifecycle::Borrowed(operation) => Err(AppStoreError::Borrowed(operation)),
                AppLifecycle::Consumed => Err(AppStoreError::Consumed),
                AppLifecycle::Removed => Err(AppStoreError::Removed),
            })
            .map_err(app_store_error)
    }

    /// Helper to get SystemStage for profiling based on PyStage
    fn get_system_stage(stage: PyStage) -> SystemStage {
        if stage.is_startup() {
            SystemStage::Startup
        } else {
            SystemStage::UpdateOrLast
        }
    }

    /// Parse a candidate system before a reload can clear or otherwise mutate the live world.
    fn preflight_reload_system(
        py: Python<'_>,
        system: &Bound<'_, PyAny>,
        generation: u32,
        error_state: &Arc<Mutex<Vec<PyErr>>>,
        error_buffer: &SystemErrorBuffer,
        system_stage: SystemStage,
        is_startup: bool,
    ) -> PyResult<()> {
        if let Ok(chained) = system.extract::<PyChainedSystems>() {
            for item in chained.systems.bind(py).iter() {
                let _ = build_scheduled_system(
                    &item,
                    generation,
                    error_state.clone(),
                    error_buffer.clone(),
                    system_stage,
                    is_startup,
                )
                .map_err(|error| annotate_registration_error(py, &item, error))?;
            }
        } else {
            let _ = build_scheduled_system(
                system,
                generation,
                error_state.clone(),
                error_buffer.clone(),
                system_stage,
                is_startup,
            )
            .map_err(|error| annotate_registration_error(py, system, error))?;
        }
        Ok(())
    }

    fn begin_operation(&self, operation: AppOperation) -> PyResult<MainAppOperationGuard> {
        self.ensure_active()?;
        begin_main_app_operation(self.app_id, operation)
    }

    /// Extract the App while a bridge may inspect Python plugin state.
    pub(crate) fn with_bevy_app_operation<F, R>(&self, operation: AppOperation, f: F) -> PyResult<R>
    where
        F: FnOnce(&mut App) -> PyResult<R>,
    {
        let mut guard = self.begin_operation(operation)?;
        f(guard.app_mut())
    }

    /// Compatibility entrypoint for Main-owned native plugin/configuration
    /// bridges. These bridges may inspect Python values, so they default to
    /// the conservative `BridgeBuild` extraction classification.
    pub(crate) fn with_bevy_app<F, R>(&self, f: F) -> PyResult<R>
    where
        F: FnOnce(&mut App) -> PyResult<R>,
    {
        self.with_bevy_app_operation(AppOperation::BridgeBuild, f)
    }

    /// Internal method to create a temporary app instance for hot reload
    /// This app will skip plugin additions and only collect system definitions
    ///
    /// Note: This is called from Rust code during hot reload, not from Python.
    /// The generation parameter is the NEW generation that systems should be registered with.
    pub(crate) fn create_reload_temp(generation: u32) -> Self {
        // Create a temporary HotReloadState with the specified generation
        // This ensures systems added via this temp app get registered with the correct generation
        let temp_state = HotReloadState::with_generation(generation);

        let app_id = consume_unstored_id(allocate_id().expect("App ID space exhausted"));

        PyApp {
            app_id,
            creation_thread: std::thread::current().id(),
            plugin_registry: RefCell::new(AddedPythonPlugins::default()),
            system_error: Arc::new(Mutex::new(Vec::new())),
            system_error_buffer: Arc::new(Mutex::new(None)),
            last_exit: Arc::new(Mutex::new(None)),
            hot_reload_state: temp_state,
            is_reload_temp: Cell::new(true),
            pending_systems: RefCell::new(Vec::new()),
            pending_set_configs: RefCell::new(Vec::new()),
            pending_resources: RefCell::new(Vec::new()),
            pending_states: RefCell::new(Vec::new()),
            pending_state_systems: RefCell::new(Vec::new()),
            pending_messages: RefCell::new(Vec::new()),
            pending_observers: RefCell::new(Vec::new()),
            pending_plugins: RefCell::new(Vec::new()),
            pending_system_names: RefCell::new(HashSet::new()),
            entrypoint_set: Cell::new(false),
        }
    }

    /// Extract pending systems from a temp reload app
    /// This is called after create_app() has been called on the temp app
    pub(crate) fn take_pending_systems(&self) -> Vec<(PyStage, Vec<Py<PyAny>>)> {
        self.pending_systems.borrow_mut().drain(..).collect()
    }

    pub(crate) fn take_pending_set_configs(&self) -> Vec<(PyStage, Vec<Py<PyAny>>)> {
        self.pending_set_configs.borrow_mut().drain(..).collect()
    }

    /// Extract pending resources from a temp reload app
    /// This is called after create_app() has been called on the temp app
    pub(crate) fn take_pending_resources(&self) -> Vec<Py<PyAny>> {
        self.pending_resources.borrow_mut().drain(..).collect()
    }

    pub(crate) fn take_pending_states(&self) -> Vec<PendingStateDefinition> {
        self.pending_states.borrow_mut().drain(..).collect()
    }

    pub(crate) fn take_pending_state_systems(&self) -> Vec<PendingStateSystems> {
        self.pending_state_systems.borrow_mut().drain(..).collect()
    }

    /// Extract pending message types from a temp reload app
    /// These need to be re-registered on the real World after reload
    pub(crate) fn take_pending_messages(&self) -> Vec<Py<PyType>> {
        self.pending_messages.borrow_mut().drain(..).collect()
    }

    /// Extract pending observer functions from a temp reload app
    /// These need to be re-registered on the real World after reload
    pub(crate) fn take_pending_observers(&self) -> Vec<Py<PyAny>> {
        self.pending_observers.borrow_mut().drain(..).collect()
    }

    /// Extract plugin identities for hot-reload baseline or delta detection.
    pub(crate) fn take_pending_plugins(&self) -> Vec<PluginIdentity> {
        self.pending_plugins.borrow_mut().drain(..).collect()
    }

    /// Ensure the state transition system is registered (called from init_state/insert_state)
    fn ensure_state_transition_system_registered(&self) -> PyResult<()> {
        self.with_bevy_app(|app| {
            ensure_state_transition_system_registered(app.world_mut());
            Ok::<(), PyErr>(())
        })?;

        Ok(())
    }
}

#[pymethods]
impl PyApp {
    /// Report the declarations a reload generation is still holding.
    ///
    /// A reload-temp App collects the scene's systems, resources, messages and
    /// observers before `create_app` drains them. Each of those reaches its
    /// defining module's dict, which can reach this App again, so without a
    /// traverse an entrypoint that raises mid-collection leaks the whole scene
    /// generation. `hot_reload_state` is deliberately not traversed: its loader
    /// lives behind a shared `Arc`/`Mutex` with several Rust owners, and
    /// `__traverse__` must neither double-count nor block (docs/safety.md).
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        // Skip any field mid-mutation: traverse must not panic or block.
        if let Ok(staged) = self.pending_systems.try_borrow() {
            for (_, systems) in staged.iter() {
                for system in systems {
                    visit.call(system)?;
                }
            }
        }
        if let Ok(staged) = self.pending_set_configs.try_borrow() {
            for (_, configs) in staged.iter() {
                for config in configs {
                    visit.call(config)?;
                }
            }
        }
        if let Ok(resources) = self.pending_resources.try_borrow() {
            for resource in resources.iter() {
                visit.call(resource)?;
            }
        }
        if let Ok(states) = self.pending_states.try_borrow() {
            for state in states.iter() {
                visit.call(&state.state_type)?;
                visit.call(&state.initial_state)?;
            }
        }
        if let Ok(staged) = self.pending_state_systems.try_borrow() {
            for entry in staged.iter() {
                visit.call(&entry.schedule)?;
                for system in &entry.systems {
                    visit.call(system)?;
                }
            }
        }
        if let Ok(messages) = self.pending_messages.try_borrow() {
            for message in messages.iter() {
                visit.call(message)?;
            }
        }
        if let Ok(observers) = self.pending_observers.try_borrow() {
            for observer in observers.iter() {
                visit.call(observer)?;
            }
        }
        Ok(())
    }

    /// Internal regression-test diagnostic. Includes Bevy infrastructure and
    /// retired hot-reload systems still present in schedule graphs.
    fn _debug_schedule_system_count(&self) -> PyResult<usize> {
        self.with_bevy_app_operation(AppOperation::WorldCallback, |app| {
            Ok(pybevy_reload::count_schedule_systems(app.world()))
        })
    }

    /// Internal regression-test diagnostic for interpreter identity aliases.
    fn _debug_hot_reload_alias_counts(&self) -> PyResult<(usize, usize)> {
        self.with_bevy_app_operation(AppOperation::WorldCallback, |app| {
            let world = app.world();
            let components = world
                .get_resource::<pybevy_core::custom_component::CustomComponentRegistry>()
                .map_or(0, |registry| registry.alias_count());
            let resources = world
                .get_resource::<pybevy_core::custom_resource::CustomResourceRegistry>()
                .map_or(0, |registry| registry.alias_count());
            Ok((components, resources))
        })
    }

    #[new]
    fn new() -> PyResult<Self> {
        let allocated_app_id = allocate_id().map_err(app_store_error)?;

        // Create and initialize the Bevy App immediately
        let mut app = App::new();

        // Add LogPlugin only if it hasn't been initialized yet
        // LogPlugin sets up a global logger/tracing subscriber, which can only be set once per process
        // Using compare_exchange ensures thread-safe check-and-set operation
        if LOG_PLUGIN_INITIALIZED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            app.add_plugins(LogPlugin {
                filter: DEFAULT_LOG_FILTER.to_string(),
                ..Default::default()
            });
        }

        configure_standard_schedules(&mut app);
        install_python_message_store(&mut app);

        // Reflect-register all bridged bevy types so MCP/editor tooling can
        // resolve them by name even without bevy's reflect_auto_register
        register_wrapped_reflect_types(app.world());

        // Pre-insert the MCP error resource and its off-world buffer, then register
        // the drain that moves buffered errors into it each frame. Pre-inserting
        // keeps the parallel error path in run_unsafe free of structural inserts.
        let system_error_buffer: SystemErrorBuffer = Arc::new(Mutex::new(None));
        let system_error = Arc::new(Mutex::new(Vec::new()));
        app.insert_resource(LastSystemError::default());
        app.insert_resource(LastErrorBuffer {
            buffer: system_error_buffer.clone(),
        });
        app.insert_resource(ObserverRuntimeSinks {
            error_state: system_error.clone(),
            error_buffer: system_error_buffer.clone(),
        });
        app.add_systems(Last, drain_last_system_error);

        // Fill any absent built-in message buffers after plugins have built.
        app.add_systems(PreStartup, ensure_builtin_message_resources);

        let app_id = BEVY_APPS
            .with(|apps_cell| apps_cell.borrow_mut().insert_with_id(allocated_app_id, app))
            .map_err(app_store_error)?;

        Ok(PyApp {
            app_id,
            creation_thread: std::thread::current().id(),
            plugin_registry: RefCell::new(AddedPythonPlugins::default()),
            system_error,
            system_error_buffer,
            last_exit: Arc::new(Mutex::new(None)),
            hot_reload_state: HotReloadState::new(),
            is_reload_temp: Cell::new(false),
            pending_systems: RefCell::new(Vec::new()),
            pending_set_configs: RefCell::new(Vec::new()),
            pending_resources: RefCell::new(Vec::new()),
            pending_states: RefCell::new(Vec::new()),
            pending_state_systems: RefCell::new(Vec::new()),
            pending_messages: RefCell::new(Vec::new()),
            pending_observers: RefCell::new(Vec::new()),
            pending_plugins: RefCell::new(Vec::new()),
            pending_system_names: RefCell::new(HashSet::new()),
            entrypoint_set: Cell::new(false),
        })
    }
    #[pyo3(signature = (schedule, *systems))]
    pub fn add_systems(
        pyself: PyRef<'_, Self>,
        py: Python,
        schedule: Bound<'_, PyAny>,
        systems: Bound<'_, PyTuple>,
    ) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;

        if !pyself.is_reload_temp.get() {
            let mut names = pyself.pending_system_names.borrow_mut();
            for system in systems.iter() {
                collect_system_names(&system, &mut names);
            }
        }

        // Parse schedule parameter - can be either PyStage or state schedule label
        enum ScheduleType {
            Stage(PyStage),
            OnEnter(StateScheduleLabel),
            OnExit(StateScheduleLabel),
            OnTransition(TransitionScheduleLabel),
        }

        let schedule_type = if let Ok(stage) = schedule.extract::<PyStage>() {
            ScheduleType::Stage(stage)
        } else if let Ok(on_enter) = schedule.cast::<PyOnEnterSchedule>() {
            let label = on_enter.borrow().to_bevy_label(py)?;
            ScheduleType::OnEnter(label)
        } else if let Ok(on_exit) = schedule.cast::<PyOnExitSchedule>() {
            let label = on_exit.borrow().to_bevy_label(py)?;
            ScheduleType::OnExit(label)
        } else if let Ok(on_transition) = schedule.cast::<PyOnTransitionSchedule>() {
            let label = on_transition.borrow().to_bevy_label(py)?;
            ScheduleType::OnTransition(label)
        } else {
            return Err(PyTypeError::new_err(
                pybevy_core::public_error::ADD_SYSTEMS_SCHEDULE_TYPE,
            ));
        };

        let error_state = pyself.system_error.clone();
        let error_buffer = pyself.system_error_buffer.clone();
        let current_generation = pyself.hot_reload_state.current_generation();

        // Handle state schedules separately (OnEnter/OnExit/OnTransition)
        match schedule_type {
            ScheduleType::OnEnter(_) | ScheduleType::OnExit(_) | ScheduleType::OnTransition(_) => {
                if pyself.is_reload_temp.get() {
                    for system in systems.iter() {
                        let _ = build_scheduled_system(
                            &system,
                            current_generation,
                            error_state.clone(),
                            error_buffer.clone(),
                            SystemStage::UpdateOrLast,
                            false,
                        )
                        .map_err(|error| annotate_registration_error(py, &system, error))?;
                    }
                    let system_funcs = systems.iter().map(Bound::unbind).collect();
                    pyself
                        .pending_state_systems
                        .borrow_mut()
                        .push(PendingStateSystems {
                            schedule: schedule.unbind(),
                            systems: system_funcs,
                        });
                    return Ok(pyself.into());
                }

                pyself.with_bevy_app(|app| {
                    let schedule_type = match schedule_type {
                        ScheduleType::OnEnter(label) => ScheduleType::OnEnter(
                            canonicalize_state_schedule_label(app.world(), label),
                        ),
                        ScheduleType::OnExit(label) => ScheduleType::OnExit(
                            canonicalize_state_schedule_label(app.world(), label),
                        ),
                        ScheduleType::OnTransition(label) => ScheduleType::OnTransition(
                            canonicalize_transition_schedule_label(app.world(), label),
                        ),
                        ScheduleType::Stage(stage) => ScheduleType::Stage(stage),
                    };

                    // Initialize the schedule with single-threaded executor.
                    // State schedules are run via world.run_schedule() from within
                    // a Python-attached context; multi-threaded would deadlock on GIL.

                    macro_rules! init_state_schedule {
                        ($app:expr, $lbl:expr) => {
                            if !$app.world().resource::<Schedules>().contains($lbl.clone()) {
                                $app.init_schedule($lbl.clone());
                                $app.world_mut()
                                    .resource_mut::<Schedules>()
                                    .get_mut($lbl.clone())
                                    .ok_or_else(|| {
                                        PyRuntimeError::new_err(format!(
                                            "Failed to initialize state schedule {:?}",
                                            $lbl
                                        ))
                                    })?
                                    .set_executor(SingleThreadedExecutor::new());
                            }
                        };
                    }

                    match &schedule_type {
                        ScheduleType::OnEnter(lbl) => init_state_schedule!(app, lbl),
                        ScheduleType::OnExit(lbl) => init_state_schedule!(app, lbl),
                        ScheduleType::OnTransition(lbl) => init_state_schedule!(app, lbl),
                        _ => unreachable!(),
                    }

                    // Add each system to the schedule.
                    for system in systems.iter() {
                        let (config, _) = build_scheduled_system(
                            &system,
                            current_generation,
                            error_state.clone(),
                            error_buffer.clone(),
                            SystemStage::UpdateOrLast, // State systems treated like Update
                            false,
                        )?;

                        match &schedule_type {
                            ScheduleType::OnEnter(lbl) => app.add_systems(lbl.clone(), config),
                            ScheduleType::OnExit(lbl) => app.add_systems(lbl.clone(), config),
                            ScheduleType::OnTransition(lbl) => app.add_systems(lbl.clone(), config),
                            _ => unreachable!(),
                        };
                    }

                    Ok::<(), PyErr>(())
                })?;

                Ok(pyself.into())
            }
            ScheduleType::Stage(stage) => {
                // Macro to add systems to the correct schedule with automatic schedule initialization
                macro_rules! add_to_schedule {
                    ($app:expr, $stage:expr, $system:expr) => {{
                        let label = $stage.intern_label();
                        if !$app.world().resource::<Schedules>().contains(label) {
                            $app.init_schedule(label);
                        }
                        $app.add_systems(label, $system);
                    }};
                }

                // If this is a temp reload app, just store the systems for later extraction
                if pyself.is_reload_temp.get() {
                    let mut system_funcs = Vec::new();
                    for system in systems.iter() {
                        let system_list: Vec<Bound<PyAny>> = match system.try_iter() {
                            Ok(iter) => iter.collect::<Result<Vec<_>, _>>()?,
                            Err(_) => vec![system.clone()],
                        };
                        for sys in system_list {
                            Self::preflight_reload_system(
                                py,
                                &sys,
                                current_generation,
                                &error_state,
                                &error_buffer,
                                Self::get_system_stage(stage),
                                stage.is_startup(),
                            )?;
                            system_funcs.push(sys.unbind());
                        }
                    }
                    pyself
                        .pending_systems
                        .borrow_mut()
                        .push((stage, system_funcs));
                    return Ok(pyself.into());
                }
                // Continue with the rest of Stage handling below...

                // Add systems directly to the app with generation-based run conditions
                pyself.with_bevy_app(|app| {
                    for system in systems {
                        // Check if this is a ChainedSystems object
                        if let Ok(chained) = system.extract::<PyChainedSystems>() {
                            // Handle chained systems
                            let system_stage = Self::get_system_stage(stage);

                            // Create a complete config for each system in the chain.
                            let py = system.py();
                            let systems_tuple = chained.systems.bind(py);
                            let mut configs = Vec::new();

                            for sys in systems_tuple.iter() {
                                let (config, _) = build_scheduled_system(
                                    &sys,
                                    current_generation,
                                    error_state.clone(),
                                    error_buffer.clone(),
                                    system_stage,
                                    stage.is_startup(),
                                )?;
                                configs.push(config);
                            }

                            if configs.is_empty() {
                                return Err(PyRuntimeError::new_err("Empty chained systems"));
                            }

                            let chained = ScheduleConfigs::Configs {
                                configs,
                                collective_conditions: Vec::new(),
                                metadata: Chain::Chained(Default::default()),
                            };

                            add_to_schedule!(app, stage, chained);
                        } else {
                            // Handle regular systems (not chained)
                            let system_list: Vec<Bound<PyAny>> = match system.try_iter() {
                                Ok(iter) => iter.collect::<Result<Vec<_>, _>>()?,
                                Err(_) => vec![system],
                            };

                            for sys in system_list {
                                let system_stage = Self::get_system_stage(stage);
                                let (config, _) = build_scheduled_system(
                                    &sys,
                                    current_generation,
                                    error_state.clone(),
                                    error_buffer.clone(),
                                    system_stage,
                                    stage.is_startup(),
                                )?;
                                add_to_schedule!(app, stage, config);
                            }
                        }
                    }
                    Ok(())
                })?;
                Ok(pyself.into())
            }
        }
    }

    #[pyo3(signature = (*plugins))]
    pub fn add_plugins(
        pyself: Py<Self>,
        py: Python,
        plugins: Bound<'_, PyTuple>,
    ) -> PyResult<Py<PyAny>> {
        pyself.borrow(py).ensure_active()?;

        // Get the App as a Bound object to pass to plugin.build()
        let app_bound = pyself.bind(py);

        // Handle two cases:
        // 1. add_plugins(PluginA(), PluginB()) - multiple args passed directly
        // 2. add_plugins((PluginA(), PluginB())) - single tuple arg
        let plugins_to_add = if plugins.len() == 1 {
            // Check if the single argument is itself a tuple
            let first_item = plugins.get_item(0)?;
            if first_item.is_instance_of::<PyTuple>() {
                // Case 2: User passed a tuple, extract it
                first_item.extract::<Bound<'_, PyTuple>>()?
            } else {
                // Case 1: Single plugin
                plugins.clone()
            }
        } else {
            // Case 1: Multiple plugins passed as separate args
            plugins.clone()
        };

        // Iterate through all plugins and call their build() method
        for plugin_arg in plugins_to_add.iter() {
            // Check if it's a type (class) or an instance
            let (plugin_instance, plugin_type) = if plugin_arg.is_instance_of::<PyType>() {
                // It's a class, instantiate it
                let plugin_type: Bound<'_, PyType> = plugin_arg.extract()?;
                (plugin_arg.call0()?, plugin_type)
            } else {
                // It's already an instance, get its type
                let plugin_type = plugin_arg.get_type();
                (plugin_arg, plugin_type)
            };

            // Validate that the instance is actually a Plugin or PluginGroup
            // Check both PyPlugin types:
            // - crate::app::plugin::PyPlugin for main crate plugins
            // - pybevy_core::PyPlugin (PyPluginBase) for feature crate plugins
            let is_plugin = plugin_instance.is_instance_of::<PyPlugin>()
                || plugin_instance.is_instance_of::<PyPluginBase>();
            let is_plugin_group = plugin_instance.is_instance_of::<PyPluginGroup>();

            if !is_plugin && !is_plugin_group {
                // Get the MRO (Method Resolution Order) to show inheritance chain
                let mro = plugin_type
                    .getattr("__mro__")
                    .and_then(|mro_tuple| {
                        let mro_names: Vec<String> = mro_tuple
                            .try_iter()?
                            .map(|t_result| {
                                t_result.and_then(|t: Bound<'_, PyAny>| {
                                    t.getattr("__name__")
                                        .and_then(|n: Bound<'_, PyAny>| n.extract::<String>())
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(mro_names.join(" -> "))
                    })
                    .unwrap_or_else(|_| "unknown".to_string());

                let plugin_name = plugin_type
                    .name()
                    .and_then(|n| n.extract::<String>())
                    .unwrap_or_else(|_| "UnknownPlugin".to_string());

                return Err(PyTypeError::new_err(
                    error_messages::plugin_not_a_plugin_error(&plugin_name, &mro),
                ));
            }

            // Check if the plugin has the @plugin decorator (only for Plugin, not PluginGroup)
            // Skip this check for built-in Rust plugins (they don't have Python decorators)
            if is_plugin && !is_plugin_group {
                let is_builtin_plugin = plugin_type
                    .getattr("__module__")
                    .ok()
                    .and_then(|m| m.extract::<String>().ok())
                    .map(|module| module.starts_with("pybevy.") || module == "builtins")
                    .unwrap_or(false);

                if !is_builtin_plugin {
                    let has_decorator = plugin_type
                        .getattr("__pybevy_plugin_decorated__")
                        .ok()
                        .and_then(|marker| marker.is_truthy().ok())
                        .unwrap_or(false);

                    if !has_decorator {
                        let plugin_name = plugin_type
                            .name()
                            .and_then(|n| n.extract::<String>())
                            .unwrap_or_else(|_| "UnknownPlugin".to_string());

                        return Err(PyTypeError::new_err(
                            error_messages::plugin_missing_decorator_error(&plugin_name),
                        ));
                    }
                }
            }

            let short_name = plugin_type
                .name()
                .and_then(|n| n.extract::<String>())
                .unwrap_or_else(|_| "UnknownPlugin".to_string());

            // Check if this plugin has already been added (important for hot reload)
            let type_ptr = plugin_type.as_ptr() as *const PyTypeObject;
            let type_key = type_ptr as usize;
            let qualified_name =
                plugin_qualified_name(type_ptr, py).unwrap_or_else(|| short_name.clone());
            let instance_key = if is_plugin && !is_plugin_group {
                plugin_instance_key(&plugin_instance, &qualified_name)?
            } else {
                None
            };
            let identity = PluginIdentity::new(qualified_name.clone(), instance_key);
            let is_reload = pyself.borrow(py).is_reload_temp.get();
            let bridge = plugin_registry::get_by_py_type(type_ptr);
            let native_already_added = if is_reload {
                false
            } else if let Some(bridge) = bridge.as_ref() {
                pyself
                    .borrow(py)
                    .with_bevy_app_operation(AppOperation::BridgeBuild, |bevy_app| {
                        Ok(bridge.is_added(bevy_app))
                    })?
            } else {
                false
            };
            let python_already_added = {
                let app_borrow = pyself.borrow(py);
                app_borrow
                    .plugin_registry
                    .borrow()
                    .contains(type_key, &identity)
            };

            if native_already_added || python_already_added {
                if let Some(key) = identity.instance_key() {
                    return Err(PyRuntimeError::new_err(duplicate_plugin_identity(
                        identity.qualified_name(),
                        key,
                    )));
                }
                // Skip this plugin - it was already added in a previous generation
                // This prevents "RecreationAttempt" errors with winit and other singleton plugins
                let app_borrow = pyself.borrow(py);
                app_borrow
                    .plugin_registry
                    .borrow_mut()
                    .insert(type_key, identity.clone());
                let mut pending = app_borrow.pending_plugins.borrow_mut();
                if !pending.contains(&identity) {
                    pending.push(identity);
                }
                if is_verbose() {
                    eprintln!("   Skipping already-added plugin: {}", plugin_type.name()?);
                }
                continue;
            }

            // Call the appropriate method based on plugin type:
            // - Plugin: use PluginBridge if registered, otherwise call build(app)
            // - PluginGroupBuilder: build(app)
            // - PluginGroup (like DefaultPlugins): _apply_to_app(app)
            //
            // During reload (is_reload_temp), skip built-in/bridge plugins that need
            // BEVY_APPS access (which temp apps lack), but let custom Python plugins
            // run build() so their systems/resources are captured in pending collections.
            if plugin_instance.is_instance_of::<PyPluginGroupBuilder>() {
                if !is_reload {
                    // PluginGroupBuilder has build(app) that applies configuration
                    plugin_instance.call_method1("build", (app_bound,))?;
                }
            } else if plugin_instance.is_instance_of::<PyDefaultPlugins>() {
                if !is_reload {
                    // DefaultPlugins (and other direct PluginGroups) use _apply_to_app
                    plugin_instance.call_method1("_apply_to_app", (app_bound,))?;
                }
            } else {
                // Regular Plugin
                if !is_reload {
                    // Normal path: try bridge first, then Python build()
                    if let Some(bridge) = bridge.as_ref() {
                        // Use the PluginBridge to build the plugin
                        pyself
                            .borrow(py)
                            .with_bevy_app_operation(AppOperation::BridgeBuild, |bevy_app| {
                                bridge.build(&plugin_instance, bevy_app)
                            })?;
                    } else {
                        // Fall back to Python build(app) method for custom plugins
                        plugin_instance.call_method1("build", (app_bound,))?;
                    }
                } else {
                    // Reload: only run build() for custom Python plugins.
                    // Skip bridge-backed and native Rust plugins because a
                    // collection-only reload wrapper has no live App slot.
                    let has_bridge = bridge.is_some();
                    let is_decorated_python_plugin = plugin_type
                        .getattr("__pybevy_plugin_decorated__")
                        .and_then(|marker| marker.is_truthy())
                        .unwrap_or(false);
                    let is_native = !is_decorated_python_plugin
                        && plugin_type
                            .getattr("__module__")
                            .and_then(|m| m.extract::<String>())
                            .map(|m| {
                                m.starts_with("_pybevy")
                                    || m.starts_with("pybevy.")
                                    || m == "builtins"
                            })
                            .unwrap_or(false);

                    if !has_bridge && !is_native {
                        // Custom Python plugin: call build() to capture
                        // systems/resources in pending collections
                        plugin_instance.call_method1("build", (app_bound,))?;
                    }
                }
            }

            // Register only after a successful build, so a failed add can be retried.
            {
                let app_borrow = pyself.borrow(py);
                let mut registry = app_borrow.plugin_registry.borrow_mut();
                registry.insert(type_key, identity.clone());
                if plugin_instance.is_instance_of::<PyPluginGroupBuilder>() {
                    let builder = plugin_instance.cast_exact::<PyPluginGroupBuilder>()?;
                    if let Some(source_type_id) = builder.borrow().source_type {
                        let source_ptr = source_type_id.as_ptr();
                        if let Some(source_name) = plugin_qualified_name(source_ptr, py) {
                            registry.insert(
                                source_ptr as usize,
                                PluginIdentity::new(source_name, None),
                            );
                        }
                    }
                }
                app_borrow.pending_plugins.borrow_mut().push(identity);
            }
        }

        Ok(pyself.into())
    }

    /// Insert a resource into the app
    pub fn insert_resource(
        pyself: PyRef<'_, Self>,
        py: Python,
        resource: Bound<'_, PyAny>,
    ) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;

        // If this is a temp reload app, store the resource instead of inserting
        if pyself.is_reload_temp.get() {
            pyself
                .pending_resources
                .borrow_mut()
                .push(resource.unbind());
            return Ok(pyself.into());
        }

        pyself.with_bevy_app(|app| {
            PyWorld::with_temporary(app.world_mut(), py, |py_world| {
                py_world.insert_resource(py, resource)?;
                Ok(())
            })
        })?;
        Ok(pyself.into())
    }

    /// Initialize a resource with default values and insert it into the app
    pub fn init_resource(
        pyself: PyRef<'_, Self>,
        py: Python,
        resource: Bound<'_, PyAny>,
    ) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;

        // Full reload clears custom resources, so capture the default instance
        // alongside values supplied through insert_resource for reconstruction.
        if pyself.is_reload_temp.get() {
            let resource_type: Bound<'_, PyType> = resource.extract()?;
            let _ = PyResourceType::try_from((&resource_type, py))?;
            let resource_instance = PyWorld::default_resource_instance(py, &resource_type)?;
            pyself
                .pending_resources
                .borrow_mut()
                .push(resource_instance);
            return Ok(pyself.into());
        }

        pyself.with_bevy_app(|app| {
            PyWorld::with_temporary(app.world_mut(), py, |py_world| {
                py_world.init_resource(py, resource)?;
                Ok(())
            })
        })?;
        Ok(pyself.into())
    }

    /// Register a custom message type
    pub fn add_message(
        pyself: PyRef<'_, Self>,
        py: Python,
        message_type: Bound<'_, PyType>,
    ) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;

        // During hot reload, collect message types for re-registration on the real World
        if pyself.is_reload_temp.get() {
            pyself
                .pending_messages
                .borrow_mut()
                .push(message_type.as_unbound().clone_ref(py));
            return Ok(pyself.into());
        }

        pyself.with_bevy_app(|app| {
            register_python_message(py, app.world_mut(), &message_type, 0)?;

            Ok(())
        })?;
        Ok(pyself.into())
    }

    pub fn add_observer(
        pyself: PyRef<'_, Self>,
        py: Python,
        observer: Bound<'_, PyAny>,
    ) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;

        // Collect observers for re-registration after reload
        if pyself.is_reload_temp.get() {
            pyself
                .pending_observers
                .borrow_mut()
                .push(observer.unbind());
            return Ok(pyself.into());
        }

        pyself.with_bevy_app(|app| {
            let world_mut = app.world_mut();

            ObserverRegistry::register_observer(py, &observer, world_mut)?;

            Ok(())
        })?;
        Ok(pyself.into())
    }

    pub fn init_state(
        pyself: PyRef<'_, Self>,
        py: Python,
        state_type: Bound<'_, PyType>,
    ) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;

        // Validate it's a @state decorated type
        if !state_type.hasattr("__pybevy_state__")? {
            return Err(PyTypeError::new_err(format!(
                "Type '{}' is not a valid state type. Did you forget the @state decorator?",
                state_type.name()?
            )));
        }

        // Get default state (first enum variant)
        let members_dict = state_type.getattr("__members__")?;
        let values = members_dict.call_method0("values")?;
        let mut members = values.try_iter()?;

        let default_state = members
            .next()
            .ok_or_else(|| PyValueError::new_err("State enum has no variants"))??
            .unbind();

        let state_type_clone = state_type.clone();

        if pyself.is_reload_temp.get() {
            pyself
                .pending_states
                .borrow_mut()
                .push(PendingStateDefinition {
                    state_type: state_type_clone.unbind(),
                    initial_state: default_state,
                });
            return Ok(pyself.into());
        }

        pyself.with_bevy_app(|app| {
            // Insert State<S> resource with default value
            let state_resource = PyState::new(py, default_state.clone_ref(py))?;

            // Insert NextState<S> resource (starts as Unchanged)
            let next_state_resource = PyNextState::new(py, state_type_clone.clone().unbind())?;

            insert_state_machine_resources(
                py,
                app.world_mut(),
                state_type_clone.clone().unbind(),
                state_resource,
                next_state_resource,
            )
        })?;

        // Register automatic state transition system
        pyself.ensure_state_transition_system_registered()?;

        Ok(pyself.into())
    }

    pub fn insert_state(
        pyself: PyRef<'_, Self>,
        py: Python,
        initial_state: Py<PyAny>,
    ) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;

        let state_type_unbind = state_member_type(py, initial_state.bind(py))?;

        if pyself.is_reload_temp.get() {
            pyself
                .pending_states
                .borrow_mut()
                .push(PendingStateDefinition {
                    state_type: state_type_unbind,
                    initial_state,
                });
            return Ok(pyself.into());
        }

        pyself.with_bevy_app(|app| {
            // Insert State<S> resource with provided value
            let state_resource = PyState::new(py, initial_state)?;

            // Insert NextState<S> resource (starts as Unchanged)
            let next_state_resource = PyNextState::new(py, state_type_unbind.clone_ref(py))?;

            insert_state_machine_resources(
                py,
                app.world_mut(),
                state_type_unbind.clone_ref(py),
                state_resource,
                next_state_resource,
            )
        })?;

        // Register automatic state transition system
        pyself.ensure_state_transition_system_registered()?;

        Ok(pyself.into())
    }

    /// Get access to the world via a callback
    ///
    /// Usage:
    ///   app.world(lambda world: world.spawn(...))
    pub fn world<'py>(&self, py: Python<'py>, callback: Bound<'py, PyAny>) -> PyResult<()> {
        self.ensure_active()?;

        if self.is_reload_temp.get() {
            return Err(PyRuntimeError::new_err(
                "app.world() is not available in @entrypoint during hot reload. \
                 Use a Startup system instead: app.add_systems(Startup, setup_fn)",
            ));
        }

        let mut guard = self.begin_operation(AppOperation::WorldCallback)?;
        PyWorld::with_temporary(guard.app_mut().world_mut(), py, |py_world| {
            let world_obj = Py::new(py, py_world.duplicate())?;
            callback
                .call1((world_obj.bind(py),))?
                .unbind()
                .into_py_any(py)?;
            Ok(())
        })
    }

    /// Run a system function once immediately on this app's world.
    ///
    /// This is a convenience method equivalent to:
    ///   app.world(lambda w: w.run_system_once(func))
    ///
    /// Usage:
    ///   app.run_system_once(setup)
    pub fn run_system_once<'py>(&self, py: Python<'py>, func: Bound<'py, PyAny>) -> PyResult<()> {
        self.ensure_active()?;

        let mut guard = self.begin_operation(AppOperation::WorldCallback)?;
        PyWorld::with_temporary(guard.app_mut().world_mut(), py, |py_world| {
            let world_obj = Py::new(py, py_world.duplicate())?;
            world_obj.borrow(py).run_system_once(func)?;
            Ok(())
        })
    }

    /// Run multiple system functions once immediately on this app's world.
    ///
    /// PyBevy-specific convenience method (not in Bevy API).
    ///
    /// Usage:
    ///   app._run_systems_once(setup, check, verify)
    #[pyo3(signature = (*funcs))]
    pub fn _run_systems_once<'py>(
        &self,
        py: Python<'py>,
        funcs: &Bound<'py, PyTuple>,
    ) -> PyResult<()> {
        self.ensure_active()?;

        let mut guard = self.begin_operation(AppOperation::WorldCallback)?;
        PyWorld::with_temporary(guard.app_mut().world_mut(), py, |py_world| {
            let world_obj = Py::new(py, py_world.duplicate())?;
            for func in funcs.iter() {
                world_obj.borrow(py).run_system_once(func)?;
            }
            Ok(())
        })
    }

    /// Initialize the app by running startup systems (PyBevy-specific convenience method)
    pub fn initialize(&self, py: Python) -> PyResult<()> {
        self.ensure_active()?;

        let app_id = self.app_id;

        // Release GIL while running initialization (required to avoid deadlock with Python systems)
        py.detach(|| {
            let mut guard = begin_main_app_operation(app_id, AppOperation::Finish)?;
            guard.app_mut().finish();
            guard.app_mut().cleanup();
            Ok::<(), PyErr>(())
        })?;

        Ok(())
    }

    /// Run the app update loop once
    pub fn update(&self, py: Python) -> PyResult<()> {
        self.ensure_active()?;

        // Clear any previous errors before running
        let previous_errors = {
            let mut error_lock = lock_or_recover(&self.system_error);
            std::mem::take(&mut *error_lock)
        };
        drop(previous_errors);

        let app_id = self.app_id;
        let last_exit = self.last_exit.clone();

        // Release GIL while running update (required to avoid deadlock with Python systems)
        py.detach(|| {
            let mut guard = begin_main_app_operation(app_id, AppOperation::Update)?;
            let exit = {
                let app = guard.app_mut();
                app.update();
                app.should_exit()
            };
            if let Some(exit) = exit {
                *lock_or_recover(&last_exit) = Some(exit);
            }
            Ok::<(), PyErr>(())
        })?;

        // Check if any system errors occurred and raise them
        raise_collected_errors(py, &self.system_error)?;

        Ok(())
    }

    pub fn finish(&self, py: Python) -> PyResult<()> {
        self.ensure_active()?;

        let app_id = self.app_id;

        py.detach(|| {
            let mut guard = begin_main_app_operation(app_id, AppOperation::Finish)?;
            guard.app_mut().finish();
            Ok::<(), PyErr>(())
        })?;

        Ok(())
    }

    pub fn cleanup(&self, py: Python) -> PyResult<()> {
        self.ensure_active()?;

        let app_id = self.app_id;

        py.detach(|| {
            let mut guard = begin_main_app_operation(app_id, AppOperation::Cleanup)?;
            guard.app_mut().cleanup();
            Ok::<(), PyErr>(())
        })?;

        Ok(())
    }

    /// Clear the scene by despawning all entities and clearing custom resources.
    ///
    /// This is similar to hot reload's Full mode but without reloading systems.
    /// Useful for JupyBevy to reset the scene when creating a new instance.
    ///
    /// Preserves:
    /// - Built-in Bevy resources (Time, AssetServer, etc.)
    /// - RenderDevice and render infrastructure
    /// - Plugin state
    ///
    /// Clears:
    /// - All entities
    /// - Custom Python resources
    pub fn clear_scene(&self, py: Python) -> PyResult<()> {
        self.ensure_active()?;

        let app_id = self.app_id;

        py.detach(|| {
            let mut guard = begin_main_app_operation(app_id, AppOperation::Cleanup)?;
            clear_entities_and_resources(guard.app_mut().world_mut());
            Ok::<(), PyErr>(())
        })?;

        Ok(())
    }

    fn _mark_entrypoint(&self) -> PyResult<()> {
        self.ensure_active()?;
        self.entrypoint_set.set(true);
        Ok(())
    }

    fn run(&self, py: Python) -> PyResult<()> {
        // Require @entrypoint decorator unless running in test mode
        if !self.entrypoint_set.get() {
            let is_testing = std::env::var("PYBEVY_TESTING").is_ok();
            if !is_testing {
                return Err(PyRuntimeError::new_err(concat!(
                    "App.run() requires the @entrypoint decorator. Example:\n",
                    "\n",
                    "@entrypoint\n",
                    "def main(app: App) -> App:\n",
                    "    return app.add_plugins(DefaultPlugins)\n",
                    "\n",
                    "if __name__ == \"__main__\":\n",
                    "    main().run()\n",
                    "\n",
                    "Under `pybevy mcp` / run_scene, @entrypoint also auto-injects the\n",
                    "plugins that power MCP tools (screenshot, spawn, query); without it\n",
                    "those tools appear to fail silently.\n",
                )));
            }
        }

        match BEVY_APPS.with(|apps_cell| apps_cell.borrow().state(self.app_id)) {
            Ok(AppLifecycle::Consumed) => {
                return Err(PyRuntimeError::new_err("run() has already been called"));
            }
            Ok(AppLifecycle::Active) => {}
            Ok(AppLifecycle::Borrowed(operation)) => {
                return Err(app_store_error(AppStoreError::Borrowed(operation)));
            }
            Ok(AppLifecycle::Removed) => return Err(app_store_error(AppStoreError::Removed)),
            Err(error) => return Err(app_store_error(error)),
        }
        let max_frames = max_frames_from_env()?;

        // Reset Python's SIGINT handler to default before detaching GIL
        // This allows Bevy's native TerminalCtrlCHandlerPlugin to handle Ctrl-C directly
        // without Python intercepting the signal first
        let previous_sigint = match PyModule::import(py, "signal") {
            Ok(signal_module) => {
                let sig_dfl = signal_module.getattr("SIG_DFL")?;
                let sigint = signal_module.getattr("SIGINT")?;
                let previous = signal_module.call_method1("getsignal", (&sigint,))?;
                match signal_module.call_method1("signal", (sigint, sig_dfl)) {
                    // getsignal returns None for handlers not installed from Python;
                    // signal() rejects None, so such handlers are left untouched.
                    Ok(_) if !previous.is_none() => Some(previous.unbind()),
                    Ok(_) => None,
                    Err(e) => {
                        eprintln!("Warning: Failed to reset Python's SIGINT handler: {}", e);
                        None
                    }
                }
            }
            Err(_) => None,
        };

        // Capture app_id before detaching GIL
        let app_id = self.app_id;
        let error_state = self.system_error.clone();
        let last_exit = self.last_exit.clone();

        // Release GIL before running to avoid deadlock with Python systems.
        let run_result = py.detach(|| {
            let mut guard = begin_main_app_operation(app_id, AppOperation::Run)?;
            {
                let app = guard.app_mut();
                if let Some(max_frames) = max_frames {
                    app.insert_resource(MaxFrames(max_frames));
                    app.add_systems(Last, exit_after_max_frames);
                }
                let has_hot_reload = app.world().get_resource::<HotReloadGeneration>().is_some();
                if !has_hot_reload {
                    app.insert_resource(SystemErrorCheck {
                        errors: error_state.clone(),
                    });
                    app.add_systems(Last, check_system_errors_and_exit);
                }
                let exit = app.run();
                *lock_or_recover(&last_exit) = Some(exit);
            }
            guard.finish_consumed();

            // Clear the system parameter cache after the app finishes
            // to prevent stale entries when function objects are recycled
            clear_system_param_cache();
            Ok::<(), PyErr>(())
        });

        if let Some(previous) = previous_sigint {
            let restored = PyModule::import(py, "signal").and_then(|signal_module| {
                let sigint = signal_module.getattr("SIGINT")?;
                signal_module.call_method1("signal", (sigint, previous.bind(py)))?;
                Ok(())
            });
            if let Err(e) = restored {
                eprintln!("Warning: Failed to restore Python's SIGINT handler: {}", e);
            }
        }

        run_result?;

        // After the event loop exits, check for system errors and raise them
        raise_collected_errors(py, &error_state)?;

        Ok(())
    }

    /// Check if a plugin of a given type has been added to the app
    pub fn is_plugin_added(&self, py: Python, plugin_type: Bound<'_, PyType>) -> PyResult<bool> {
        self.ensure_active()?;

        let type_ptr = plugin_type.as_ptr() as *const PyTypeObject;
        let qualified_name = plugin_qualified_name(type_ptr, py).unwrap_or_else(|| {
            plugin_type
                .name()
                .and_then(|name| name.extract::<String>())
                .unwrap_or_else(|_| "UnknownPlugin".to_string())
        });
        let python_added = self
            .plugin_registry
            .borrow()
            .contains_class(type_ptr as usize, &qualified_name);

        if python_added {
            return Ok(true);
        }

        let Some(bridge) = plugin_registry::get_by_py_type(type_ptr) else {
            return Ok(false);
        };

        self.with_bevy_app_operation(AppOperation::BridgeBuild, |bevy_app| {
            Ok(bridge.is_added(bevy_app))
        })
    }

    /// Get the hot reload state for CLI integration
    /// This allows the CLI watcher to signal reloads
    #[getter]
    pub fn _state(&self, py: Python) -> PyResult<Py<PyAppReloadState>> {
        let reload_state = PyAppReloadState::new(self.hot_reload_state.clone());
        Py::new(py, reload_state)
    }

    /// Record the importable scene module used by control `run_code` requests.
    #[pyo3(name = "_set_scene_module")]
    pub fn set_scene_module(pyself: PyRef<'_, Self>, module_name: String) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;
        pyself.with_bevy_app(|app| {
            app.insert_resource(ActiveSceneModule::new(module_name));
            Ok::<(), PyErr>(())
        })?;
        Ok(pyself.into())
    }

    /// Set the hot reload loader function (called by CLI)
    /// The loader should return a function that when called returns create_app function
    #[pyo3(name = "_set_hot_reload_loader")]
    pub fn set_hot_reload_loader(
        pyself: PyRef<'_, Self>,
        loader: Bound<'_, PyAny>,
    ) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;
        pyself.hot_reload_state.set_loader(loader.unbind());
        let initial_plugins: HashSet<PluginIdentity> =
            pyself.take_pending_plugins().into_iter().collect();
        let initial_systems = mem::take(&mut *pyself.pending_system_names.borrow_mut());

        // Add the hot reload system to the app if not already added
        pyself.with_bevy_app(|app| {
            // Add hot reload checking system
            add_hot_reload_system(
                app,
                pyself.hot_reload_state.clone(),
                pyself.system_error.clone(),
            );
            if let Some(mut tracker) = app.world_mut().get_resource_mut::<PluginTracker>() {
                tracker.known_plugins = initial_plugins;
                tracker.baseline_initialized = true;
            }
            if let Some(mut registry) = app.world_mut().get_resource_mut::<DynamicSystemRegistry>()
            {
                registry.set_system_baseline(initial_systems);
            }
            Ok::<(), PyErr>(())
        })?;

        Ok(pyself.into())
    }

    /// Check if the app should exit
    ///
    /// Returns the AppExit value if an exit has been requested, None otherwise.
    /// This allows checking exit status programmatically for conditional logic or tests.
    /// Can be called before or after run() to check the exit status.
    pub fn should_exit(&self, py: Python) -> PyResult<Option<Py<PyAny>>> {
        if let Some(exit) = lock_or_recover(&self.last_exit).clone() {
            return materialize_app_exit(py, &exit).map(Some);
        }

        let exit = BEVY_APPS
            .with(|apps_cell| {
                apps_cell
                    .borrow_mut()
                    .with_app_leaf(self.app_id, |app| app.should_exit())
            })
            .map_err(app_store_error)?;
        if let Some(exit) = &exit {
            *lock_or_recover(&self.last_exit) = Some(exit.clone());
        }
        exit.map(|exit| materialize_app_exit(py, &exit)).transpose()
    }

    /// Initialize a schedule and add it to the app
    ///
    /// Creates an empty schedule with the given label. The schedule can then be used
    /// with add_systems() to add systems to it.
    ///
    /// Usage:
    ///   app.init_schedule(CustomSchedule)
    ///   app.add_systems(CustomSchedule, my_system)
    pub fn init_schedule(pyself: PyRef<'_, Self>, label: PyStage) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;

        if pyself.is_reload_temp.get() {
            return Ok(pyself.into());
        }

        pyself.with_bevy_app(|app| {
            app.init_schedule(label.intern_label());

            Ok(())
        })?;
        Ok(pyself.into())
    }

    /// Run a specific schedule once on the app's world.
    ///
    /// This executes only the systems registered in the given schedule,
    /// without running the full frame update. Useful for RL simulation
    /// workloads where SimTick needs to run independently of rendering.
    ///
    /// Usage:
    ///   app.run_schedule(SimTick)
    pub fn run_schedule(&self, py: Python, stage: PyStage) -> PyResult<()> {
        self.ensure_active()?;

        // Clear any previous errors before running
        let previous_errors = {
            let mut error_lock = lock_or_recover(&self.system_error);
            std::mem::take(&mut *error_lock)
        };
        drop(previous_errors);

        let app_id = self.app_id;

        // Release GIL while running schedule (required to avoid deadlock with Python systems)
        py.detach(|| {
            let mut guard = begin_main_app_operation(app_id, AppOperation::RunSchedule)?;
            stage.run_on_world(guard.app_mut().world_mut());
            Ok::<(), PyErr>(())
        })?;

        // Check if any system errors occurred and raise them
        raise_collected_errors(py, &self.system_error)?;

        Ok(())
    }

    /// Configure system-set hierarchy, ordering, and shared run conditions.
    #[pyo3(signature = (schedule, *sets))]
    pub fn configure_sets(
        pyself: PyRef<'_, Self>,
        schedule: PyStage,
        sets: Bound<'_, PyTuple>,
    ) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;
        if sets.is_empty() {
            return Err(PyValueError::new_err(
                "configure_sets() requires at least one SystemSet or SystemSetConfig",
            ));
        }

        if pyself.is_reload_temp.get() {
            pyself
                .pending_set_configs
                .borrow_mut()
                .push((schedule, sets.iter().map(Bound::unbind).collect()));
            return Ok(pyself.into());
        }

        let generation = pyself.hot_reload_state.current_generation();
        let error_state = pyself.system_error.clone();
        let system_stage = Self::get_system_stage(schedule);
        pyself.with_bevy_app(|app| {
            let label = schedule.intern_label();
            if !app.world().resource::<Schedules>().contains(label) {
                app.init_schedule(label);
            }
            for set in sets.iter() {
                if let Ok(chained) = set.extract::<PyChainedSystemSets>() {
                    let mut configs = Vec::new();
                    let mut identities = Vec::new();
                    for member in chained.sets.bind(set.py()).iter() {
                        identities.push(system_set_config_identity(&member)?);
                        configs.push(build_set_config(
                            &member,
                            generation,
                            error_state.clone(),
                            system_stage,
                        )?);
                    }
                    if configs.is_empty() {
                        return Err(PyValueError::new_err(
                            "ChainedSystemSets requires at least one set",
                        ));
                    }
                    app.configure_sets(
                        label,
                        ScheduleConfigs::Configs {
                            configs,
                            collective_conditions: Vec::new(),
                            metadata: Chain::Chained(Default::default()),
                        },
                    );
                    let mut installed = app
                        .world_mut()
                        .get_resource_or_insert_with(InstalledSystemSetConfigs::default);
                    for identity in identities {
                        installed.insert(schedule, identity);
                    }
                } else {
                    let identity = system_set_config_identity(&set)?;
                    let config =
                        build_set_config(&set, generation, error_state.clone(), system_stage)?;
                    app.configure_sets(label, config);
                    app.world_mut()
                        .get_resource_or_insert_with(InstalledSystemSetConfigs::default)
                        .insert(schedule, identity);
                }
            }
            Ok(())
        })?;
        Ok(pyself.into())
    }
}

/// Implement Drop to ensure the Bevy App is properly cleaned up
/// This removes the App from thread-local storage and drops it explicitly
/// to ensure task pools and worker threads are shut down before Python TLS cleanup
impl Drop for PyApp {
    fn drop(&mut self) {
        if !self.is_reload_temp.get() {
            // Check if we're on the same thread where the PyApp was created
            let current_thread = std::thread::current().id();
            if current_thread != self.creation_thread {
                // Cross-thread drop detected - we can't safely access thread-local storage
                // This typically happens during parallel test execution (pytest -n auto)
                // The App will leak in the original thread's TLS, but the process will exit soon
                eprintln!(
                    "WARNING: PyApp (app_id={}) is being dropped on a different thread than where it was created. \
                    The Bevy App may leak in thread-local storage. This is expected during parallel test execution.",
                    self.app_id
                );
                return;
            }

            // Remove the App from thread-local storage by app_id and drop it explicitly
            // Bevy's App::drop() will handle task pool cleanup automatically
            // IMPORTANT: Remove from storage FIRST, then drop OUTSIDE the borrow
            // to avoid RefCell panic if drop triggers Python callbacks that access BEVY_APPS
            // Use try_borrow_mut to handle potential concurrent GC drops gracefully
            let app_to_drop = BEVY_APPS.with(|apps_cell| {
                match apps_cell.try_borrow_mut() {
                    Ok(mut apps) => match apps.remove(self.app_id) {
                        Ok(app) => app,
                        Err(AppStoreError::Borrowed(operation)) => {
                            eprintln!(
                                "WARNING: Could not cleanup PyApp (app_id={}) while it is executing {}.",
                                self.app_id, operation
                            );
                            None
                        }
                        Err(AppStoreError::Missing(_)
                        | AppStoreError::Consumed
                        | AppStoreError::Removed) => None,
                        Err(error) => {
                            eprintln!(
                                "WARNING: Could not cleanup PyApp (app_id={}): {}.",
                                self.app_id, error
                            );
                            None
                        }
                    },
                    Err(_) => {
                        // RefCell already borrowed - this can happen during Python GC
                        // when multiple PyApp objects are being collected simultaneously.
                        // In this case, the App will leak, but we avoid the panic.
                        eprintln!(
                            "WARNING: Could not cleanup PyApp (app_id={}) - RefCell already borrowed. \
                            This may cause a small memory leak but prevents a panic.",
                            self.app_id
                        );
                        None
                    }
                }
            });
            if let Some(app) = app_to_drop {
                drop(app);
            }

            // Clear the system parameter cache to prevent stale entries
            // when function objects are recycled (e.g., across tests)
            clear_system_param_cache();
        }
    }
}
