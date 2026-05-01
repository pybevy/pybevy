use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::ThreadId,
};

use bevy::{
    app::{
        App, AppExit, First, FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate,
        Last, Main, MainScheduleOrder, PostStartup, PostUpdate, PreStartup, PreUpdate, Startup,
        Update,
    },
    ecs::{
        message::MessageWriter,
        resource::Resource,
        schedule::{Chain, ExecutorKind, IntoScheduleConfigs, ScheduleConfigs, Schedules},
        system::Res,
        world::World,
    },
    log::LogPlugin,
};
use pybevy_core::{PyMessage, PyPlugin as PyPluginBase, plugin::plugin_registry};
use pybevy_reload::{HotReloadGeneration, SystemStage, generation_matches, startup_or_reload};
use pyo3::{
    IntoPyObjectExt,
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    ffi::PyTypeObject,
    prelude::*,
    types::{PyList, PyModule, PyTuple, PyType},
};

use crate::{
    app::{
        PyStage, SimTick,
        app_exit::PyAppExit,
        chained_systems::PyChainedSystems,
        error_messages,
        hot_reload::{
            bindings::{PyAppReloadState, add_hot_reload_system},
            cleanup::clear_entities_and_resources,
            state::HotReloadState,
        },
        plugin::{PyPlugin, PyPluginGroup},
        plugins::{PyDefaultPlugins, PyPluginGroupBuilder},
    },
    ecs::{
        conditional_system::PyConditionalSystem,
        dynamic_condition::DynamicCondition,
        dynamic_system::{DynamicSystem, clear_system_param_cache},
        messages::MessageRegistry,
        observer_registry::ObserverRegistry,
        state::{
            PyNextState, PyOnEnterSchedule, PyOnExitSchedule, PyOnTransitionSchedule, PyState,
            StateScheduleLabel, TransitionScheduleLabel, apply_state_transitions,
        },
        world::PyWorld,
    },
};

const DEFAULT_LOG_FILTER: &str = "bevy=warn,wgpu=error,naga=warn,winit=warn";

/// Global flag to track whether LogPlugin has been initialized
/// LogPlugin sets up a global logger/tracing subscriber, which can only be set once per process
/// This prevents the "already set" error when creating multiple App instances (e.g., in tests)
static LOG_PLUGIN_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Global counter for generating unique app IDs
/// Each PyApp instance gets a unique ID to index into the thread-local HashMap
static NEXT_APP_ID: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// Thread-local storage for Bevy Apps (since App is !Send due to WinitPlugin)
    /// Each PyApp instance has its own entry indexed by app_id
    /// This allows multiple PyApp instances to coexist (e.g., in Jupyter notebooks)
    static BEVY_APPS: RefCell<HashMap<usize, App>> = RefCell::new(HashMap::new());
}

/// Cleanup function called during Python shutdown via atexit handler.
/// This explicitly clears all Apps from thread-local storage BEFORE Python's
/// thread-local destructors run, preventing crashes when Apps with Python
/// resources try to access already-destroyed PyO3 thread-locals during cleanup.
///
/// CRITICAL: This must be called before Python's TLS destructors run, otherwise
/// resources containing Python objects (custom components, messages) will panic
/// when trying to acquire the GIL or access Python state during drop.
#[pyfunction]
pub(crate) fn cleanup_apps_on_shutdown() {
    BEVY_APPS.with(|apps_cell| {
        // Clear all Apps explicitly while Python is still alive
        // This drops each App in a controlled manner before TLS destruction
        apps_cell.borrow_mut().clear();
    });
}

/// TEST ONLY: Get the count of Apps currently in thread-local storage.
/// Used to verify that atexit cleanup works correctly.
#[pyfunction]
pub(crate) fn _test_get_app_count() -> usize {
    BEVY_APPS.with(|apps_cell| apps_cell.borrow().len())
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
    let mut errors = error_state.lock().unwrap();
    if errors.is_empty() {
        return Ok(());
    }
    let errors = std::mem::take(&mut *errors);
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

fn check_system_errors_and_exit(
    error_check: Res<SystemErrorCheck>,
    mut exit: MessageWriter<AppExit>,
) {
    let lock = error_check.errors.lock().unwrap();
    if !lock.is_empty() {
        exit.write(AppExit::from_code(1));
    }
}

/// Tracks which plugins have been added, using both pointer (fast path) and
/// qualified name (hot-reload resilience). On hot reload, Python classes get new
/// PyTypeObject pointers, so pointer-only checks would miss already-added plugins.
#[derive(Default)]
struct PluginRegistry {
    by_ptr: HashSet<*const PyTypeObject>,
    by_name: HashSet<String>,
}

impl PluginRegistry {
    fn contains(&self, type_ptr: *const PyTypeObject, py: Python) -> bool {
        if self.by_ptr.contains(&type_ptr) {
            return true;
        }
        // Hot-reload path: pointer changed but name matches
        let name = Self::get_qualified_name(type_ptr, py);
        if let Some(ref name) = name
            && self.by_name.contains(name)
        {
            return true;
        }
        false
    }

    fn insert(&mut self, type_ptr: *const PyTypeObject, py: Python) {
        self.by_ptr.insert(type_ptr);
        if let Some(name) = Self::get_qualified_name(type_ptr, py) {
            self.by_name.insert(name);
        }
    }

    fn get_qualified_name(type_ptr: *const PyTypeObject, py: Python) -> Option<String> {
        let py_type =
            unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };
        if let Ok(cls) = py_type.cast::<PyType>() {
            let module = cls.getattr("__module__").ok()?.extract::<String>().ok()?;
            let qualname = cls.getattr("__qualname__").ok()?.extract::<String>().ok()?;
            Some(format!("{}.{}", module, qualname))
        } else {
            None
        }
    }
}

#[pyclass(name = "App", unsendable)]
pub struct PyApp {
    /// Unique identifier for this PyApp instance
    /// Used to index into the thread-local BEVY_APPS HashMap
    app_id: usize,

    /// Thread ID where this PyApp was created
    /// Used to detect cross-thread drops and prevent memory leaks
    creation_thread: ThreadId,

    /// Tracks whether the app has been consumed by run()
    /// true = consumed, false = active
    /// Uses Cell for interior mutability without synchronization overhead
    /// (safe because PyO3's GIL ensures single-threaded access)
    is_consumed: Cell<bool>,

    /// Registry of plugin types that have been added (by pointer for fast lookup,
    /// by name for hot-reload resilience when Python classes get new type pointers)
    plugin_registry: RefCell<PluginRegistry>,

    /// Shared error state for collecting system errors (parameter + execution)
    /// Arc allows sharing with DynamicSystem instances, Mutex for thread-safe access
    system_error: Arc<Mutex<Vec<PyErr>>>,

    /// Hot reload state for development mode
    /// Allows CLI watcher to trigger reloads
    hot_reload_state: HotReloadState,

    /// Flag indicating this is a temporary app for hot reload system extraction
    /// When true, plugin additions are skipped
    is_reload_temp: Cell<bool>,

    /// Storage for system definitions during hot reload
    /// When is_reload_temp=true, systems are stored here instead of added to Bevy
    pending_systems: RefCell<Vec<(PyStage, Vec<Py<PyAny>>)>>,

    /// Storage for resource instances during hot reload
    /// When is_reload_temp=true, resources are stored here instead of added to Bevy
    pending_resources: RefCell<Vec<Py<PyAny>>>,

    /// Storage for message types during hot reload
    /// When is_reload_temp=true, message types are stored here for re-registration
    pending_messages: RefCell<Vec<Py<PyType>>>,

    /// Storage for observer functions during hot reload
    /// When is_reload_temp=true, observer functions are stored here for re-registration
    pending_observers: RefCell<Vec<Py<PyAny>>>,

    /// Storage for plugin info during hot reload
    /// When is_reload_temp=true, plugin names are recorded for delta detection
    pending_plugins: RefCell<Vec<String>>,

    /// Whether @entrypoint decorator has been applied
    /// run() requires this unless PYBEVY_TESTING env var is set
    entrypoint_set: Cell<bool>,
}

impl PyApp {
    /// Helper method to check if app has been consumed
    fn ensure_active(&self) -> PyResult<()> {
        if self.is_consumed.get() {
            return Err(PyRuntimeError::new_err(
                "Cannot perform operation after run() has been called",
            ));
        }
        Ok(())
    }

    /// Helper to get SystemStage for profiling based on PyStage
    fn get_system_stage(stage: PyStage) -> SystemStage {
        match stage {
            PyStage::Startup | PyStage::PreStartup | PyStage::PostStartup => SystemStage::Startup,
            _ => SystemStage::UpdateOrLast,
        }
    }

    /// Helper method to get mutable reference to the app from thread-local storage
    /// Returns a clear error if the app is not available
    fn get_app_mut<'a>(&self, apps: &'a mut HashMap<usize, App>) -> PyResult<&'a mut App> {
        apps.get_mut(&self.app_id).ok_or_else(|| {
            if self.is_consumed.get() {
                PyRuntimeError::new_err("Cannot perform operation after run() has been called")
            } else {
                PyRuntimeError::new_err(
                    "App is not initialized. This may occur if the app was accessed from a \
                    different thread, or if there was an error during app initialization.",
                )
            }
        })
    }

    /// Internal method for plugins to access the Bevy App
    /// This allows plugins to add Bevy plugins, resources, etc.
    pub(crate) fn with_bevy_app<F, R>(&self, f: F) -> PyResult<R>
    where
        F: FnOnce(&mut App) -> PyResult<R>,
    {
        self.ensure_active()?;

        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = self.get_app_mut(&mut apps)?;
            f(app)
        })
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

        // Generate a unique app_id even for temp apps (though they don't store a Bevy App)
        let app_id = NEXT_APP_ID.fetch_add(1, Ordering::SeqCst);

        PyApp {
            app_id,
            creation_thread: std::thread::current().id(),
            is_consumed: Cell::new(false),
            plugin_registry: RefCell::new(PluginRegistry::default()),
            system_error: Arc::new(Mutex::new(Vec::new())),
            hot_reload_state: temp_state,
            is_reload_temp: Cell::new(true),
            pending_systems: RefCell::new(Vec::new()),
            pending_resources: RefCell::new(Vec::new()),
            pending_messages: RefCell::new(Vec::new()),
            pending_observers: RefCell::new(Vec::new()),
            pending_plugins: RefCell::new(Vec::new()),
            entrypoint_set: Cell::new(false),
        }
    }

    /// Extract pending systems from a temp reload app
    /// This is called after create_app() has been called on the temp app
    pub(crate) fn take_pending_systems(&self) -> Vec<(PyStage, Vec<Py<PyAny>>)> {
        self.pending_systems.borrow_mut().drain(..).collect()
    }

    /// Extract pending resources from a temp reload app
    /// This is called after create_app() has been called on the temp app
    pub(crate) fn take_pending_resources(&self) -> Vec<Py<PyAny>> {
        self.pending_resources.borrow_mut().drain(..).collect()
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

    /// Extract pending plugin names from a temp reload app
    /// Used for plugin delta detection during hot reload
    pub(crate) fn take_pending_plugins(&self) -> Vec<String> {
        self.pending_plugins.borrow_mut().drain(..).collect()
    }

    /// Ensure the state transition system is registered (called from init_state/insert_state)
    fn ensure_state_transition_system_registered(&self) -> PyResult<()> {
        static REGISTERED_APPS: Mutex<Option<HashSet<usize>>> = Mutex::new(None);

        // Check if this app has already registered the system
        {
            let mut registered = REGISTERED_APPS.lock().unwrap();
            if registered.is_none() {
                *registered = Some(HashSet::new());
            }

            if registered.as_ref().unwrap().contains(&self.app_id) {
                return Ok(());
            }

            // Mark this app as registered
            registered.as_mut().unwrap().insert(self.app_id);
        }

        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = self.get_app_mut(&mut apps)?;

            // Wrap the apply_state_transitions function in a closure for PreUpdate
            let system_fn = move |py: Python, world: &mut World| apply_state_transitions(py, world);

            // Add state transition processing to PreUpdate so it runs
            // after First (where transitions may be queued) and before Update
            if !app.world().resource::<Schedules>().contains(PreUpdate) {
                app.init_schedule(PreUpdate);
            }
            app.add_systems(PreUpdate, move |world: &mut World| {
                Python::attach(|py| {
                    if let Err(e) = system_fn(py, world) {
                        eprintln!("State transition error: {}", e);
                    }
                });
            });

            Ok::<(), PyErr>(())
        })?;

        Ok(())
    }
}

#[pymethods]
impl PyApp {
    #[new]
    fn new() -> PyResult<Self> {
        // Generate a unique app ID for this instance
        let app_id = NEXT_APP_ID.fetch_add(1, Ordering::SeqCst);

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

        // Initialize SimTick schedule and insert it into the main schedule order
        // Frame order: First → PreUpdate → SimTick → Update → PostUpdate → Last
        app.init_schedule(SimTick);
        app.world_mut()
            .resource_mut::<MainScheduleOrder>()
            .insert_after(PreUpdate, SimTick);

        // Store the app in thread-local HashMap indexed by app_id
        BEVY_APPS.with(|apps_cell| {
            apps_cell.borrow_mut().insert(app_id, app);
        });

        Ok(PyApp {
            app_id,
            creation_thread: std::thread::current().id(),
            is_consumed: Cell::new(false),
            plugin_registry: RefCell::new(PluginRegistry::default()),
            system_error: Arc::new(Mutex::new(Vec::new())),
            hot_reload_state: HotReloadState::new(),
            is_reload_temp: Cell::new(false),
            pending_systems: RefCell::new(Vec::new()),
            pending_resources: RefCell::new(Vec::new()),
            pending_messages: RefCell::new(Vec::new()),
            pending_observers: RefCell::new(Vec::new()),
            pending_plugins: RefCell::new(Vec::new()),
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
                "add_systems() schedule parameter must be Stage, OnEnter(), OnExit(), or OnTransition()",
            ));
        };

        let error_state = pyself.system_error.clone();
        let current_generation = pyself.hot_reload_state.current_generation();

        // Handle state schedules separately (OnEnter/OnExit/OnTransition)
        match schedule_type {
            ScheduleType::OnEnter(_) | ScheduleType::OnExit(_) | ScheduleType::OnTransition(_) => {
                // For state schedules, add systems without hot reload support for now
                // State schedules don't need generation tracking since they run on transitions
                BEVY_APPS.with(|apps_cell| {
                    let mut apps = apps_cell.borrow_mut();
                    let app = pyself.get_app_mut(&mut apps)?;

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
                                    .set_executor_kind(ExecutorKind::SingleThreaded);
                            }
                        };
                    }

                    match &schedule_type {
                        ScheduleType::OnEnter(lbl) => init_state_schedule!(app, lbl),
                        ScheduleType::OnExit(lbl) => init_state_schedule!(app, lbl),
                        ScheduleType::OnTransition(lbl) => init_state_schedule!(app, lbl),
                        _ => unreachable!(),
                    }

                    // Add each system to the schedule
                    for system in systems.iter() {
                        let dynamic_system = DynamicSystem::new(
                            system.unbind(),
                            current_generation,
                            error_state.clone(),
                            SystemStage::UpdateOrLast, // State systems treated like Update
                        )?;

                        match &schedule_type {
                            ScheduleType::OnEnter(lbl) => {
                                app.add_systems(lbl.clone(), dynamic_system)
                            }
                            ScheduleType::OnExit(lbl) => {
                                app.add_systems(lbl.clone(), dynamic_system)
                            }
                            ScheduleType::OnTransition(lbl) => {
                                app.add_systems(lbl.clone(), dynamic_system)
                            }
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
                        // Init schedule if needed
                        match $stage {
                            PyStage::Main => {
                                if !$app.world().resource::<Schedules>().contains(Main) {
                                    $app.init_schedule(Main);
                                }
                                $app.add_systems(Main, $system);
                            }
                            PyStage::First => {
                                if !$app.world().resource::<Schedules>().contains(First) {
                                    $app.init_schedule(First);
                                }
                                $app.add_systems(First, $system);
                            }
                            PyStage::PreUpdate => {
                                if !$app.world().resource::<Schedules>().contains(PreUpdate) {
                                    $app.init_schedule(PreUpdate);
                                }
                                $app.add_systems(PreUpdate, $system);
                            }
                            PyStage::PostUpdate => {
                                if !$app.world().resource::<Schedules>().contains(PostUpdate) {
                                    $app.init_schedule(PostUpdate);
                                }
                                $app.add_systems(PostUpdate, $system);
                            }
                            PyStage::PreStartup => {
                                if !$app.world().resource::<Schedules>().contains(PreStartup) {
                                    $app.init_schedule(PreStartup);
                                }
                                $app.add_systems(PreStartup, $system);
                            }
                            PyStage::PostStartup => {
                                if !$app.world().resource::<Schedules>().contains(PostStartup) {
                                    $app.init_schedule(PostStartup);
                                }
                                $app.add_systems(PostStartup, $system);
                            }
                            PyStage::Last => {
                                if !$app.world().resource::<Schedules>().contains(Last) {
                                    $app.init_schedule(Last);
                                }
                                $app.add_systems(Last, $system);
                            }
                            PyStage::FixedFirst => {
                                if !$app.world().resource::<Schedules>().contains(FixedFirst) {
                                    $app.init_schedule(FixedFirst);
                                }
                                $app.add_systems(FixedFirst, $system);
                            }
                            PyStage::FixedPreUpdate => {
                                if !$app
                                    .world()
                                    .resource::<Schedules>()
                                    .contains(FixedPreUpdate)
                                {
                                    $app.init_schedule(FixedPreUpdate);
                                }
                                $app.add_systems(FixedPreUpdate, $system);
                            }
                            PyStage::FixedPostUpdate => {
                                if !$app
                                    .world()
                                    .resource::<Schedules>()
                                    .contains(FixedPostUpdate)
                                {
                                    $app.init_schedule(FixedPostUpdate);
                                }
                                $app.add_systems(FixedPostUpdate, $system);
                            }
                            PyStage::FixedLast => {
                                if !$app.world().resource::<Schedules>().contains(FixedLast) {
                                    $app.init_schedule(FixedLast);
                                }
                                $app.add_systems(FixedLast, $system);
                            }
                            PyStage::Startup => {
                                $app.add_systems(Startup, $system);
                            }
                            PyStage::Update => {
                                $app.add_systems(Update, $system);
                            }
                            PyStage::FixedUpdate => {
                                $app.add_systems(FixedUpdate, $system);
                            }
                            PyStage::SimTick => {
                                if !$app.world().resource::<Schedules>().contains(SimTick) {
                                    $app.init_schedule(SimTick);
                                }
                                $app.add_systems(SimTick, $system);
                            }
                        }
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
                BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = pyself.get_app_mut(&mut apps)?;

            for system in systems {
                // Check if this is a ChainedSystems object
                if let Ok(chained) = system.extract::<PyChainedSystems>() {
                    // Handle chained systems
                    let system_stage = Self::get_system_stage(stage);

                    // Create DynamicSystem for each system in the chain
                    let py = system.py();
                    let systems_tuple = chained.systems.bind(py);
                    let mut dynamic_systems = Vec::new();

                    for sys in systems_tuple.iter() {
                        let dynamic_system = DynamicSystem::new(
                            sys.unbind(),
                            current_generation,
                            error_state.clone(),
                            system_stage,
                        )?;
                        dynamic_systems.push(dynamic_system);
                    }

                    // Add run conditions and chain the systems

                    if dynamic_systems.is_empty() {
                        return Err(PyRuntimeError::new_err("Empty chained systems"));
                    }

                    let is_startup = matches!(
                        stage,
                        PyStage::Startup | PyStage::PreStartup | PyStage::PostStartup
                    );

                    // Build chained SystemConfigs directly — supports any number of systems
                    let configs: Vec<ScheduleConfigs<_>> = dynamic_systems
                        .into_iter()
                        .map(|sys| {
                            if is_startup {
                                sys.run_if(startup_or_reload(current_generation))
                            } else {
                                sys.run_if(generation_matches(current_generation))
                            }
                        })
                        .collect();

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
                        // Check if this is a conditional system (run_if)
                        let (system_func, condition_func) = if let Ok(conditional) = sys.extract::<PyConditionalSystem>() {
                            // Extract system and condition from PyConditionalSystem
                            let py = sys.py();
                            (conditional.system.bind(py).clone(), Some(conditional.condition))
                        } else {
                            // Regular system without condition
                            (sys.clone(), None)
                        };

                        // Convert PyStage to SystemStage for profiler
                        let system_stage = Self::get_system_stage(stage);

                        let dynamic_system = DynamicSystem::new(
                            system_func.unbind(),
                            current_generation,
                            error_state.clone(),
                            system_stage,
                        )?;

                        // Always add generation-based run conditions for hot reload support

                        // Add user condition if present
                        if let Some(cond) = condition_func {
                            // Check if the condition has parameters by inspecting it
                            let has_params = Python::attach(|py| -> bool {
                                let inspect = py.import("inspect").ok();
                                if let Some(inspect_mod) = inspect
                                    && let Ok(sig) = inspect_mod.call_method1("signature", (cond.bind(py),))
                                        && let Ok(params) = sig.getattr("parameters")
                                            && let Ok(values) = params.getattr("values")
                                                && let Ok(params_list) = values.call0() {
                                                    return params_list.len().unwrap_or(0) > 0;
                                                }
                                false
                            });

                            if has_params {
                                // Condition has system parameters - use DynamicCondition
                                // (it includes generation checking internally)
                                let dynamic_condition = DynamicCondition::new(
                                    cond,
                                    current_generation,
                                    error_state.clone(),
                                    system_stage,
                                )?;

                                add_to_schedule!(app, stage, dynamic_system.run_if(dynamic_condition));
                            } else {
                                // Simple parameterless condition - use closure (current approach)
                                // Create combined closure that includes both generation check and user condition
                                let is_startup_schedule = matches!(stage, PyStage::Startup | PyStage::PreStartup | PyStage::PostStartup);
                                let expected_gen = current_generation;

                                let combined_condition = move |generation_res: Option<Res<HotReloadGeneration>>| -> bool {
                                    // First check generation
                                    let gen_check = if is_startup_schedule {
                                        // Startup schedules use startup_or_reload logic
                                        match generation_res {
                                            Some(ref res) => {
                                                res.current == expected_gen || res.current == expected_gen + 1
                                            }
                                            None => true,
                                        }
                                    } else {
                                        // Other schedules use generation_matches logic
                                        match generation_res {
                                            Some(ref res) => res.current == expected_gen,
                                            None => true,
                                        }
                                    };

                                    if !gen_check {
                                        return false;
                                    }

                                    // Then check user condition
                                    Python::attach(|py| {
                                        let result = cond.bind(py).call0();
                                        match result {
                                            Ok(obj) => obj.extract::<bool>().unwrap_or_else(|e| {
                                                eprintln!("run_if condition must return bool: {}", e);
                                                false
                                            }),
                                            Err(e) => {
                                                eprintln!("Error calling run_if condition: {}", e);
                                                false
                                            }
                                        }
                                    })
                                };

                                add_to_schedule!(app, stage, dynamic_system.run_if(combined_condition));
                            }
                        } else {
                            // No user condition - just use generation condition
                            let run_condition = match stage {
                                PyStage::Startup | PyStage::PreStartup | PyStage::PostStartup =>
                                    dynamic_system.run_if(startup_or_reload(current_generation)),
                                _ => dynamic_system.run_if(generation_matches(current_generation)),
                            };
                            add_to_schedule!(app, stage, run_condition);
                        }
                    }
                }
            }
            Ok(pyself.into())
        })
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

            // Record plugin name for delta detection during reload
            if pyself.borrow(py).is_reload_temp.get() {
                let name = plugin_type
                    .name()
                    .and_then(|n| n.extract::<String>())
                    .unwrap_or_else(|_| "UnknownPlugin".to_string());
                pyself.borrow(py).pending_plugins.borrow_mut().push(name);
            }

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

            // Check if this plugin has already been added (important for hot reload)
            let type_ptr = plugin_type.as_ptr() as *const PyTypeObject;
            let already_added = {
                let app_borrow = pyself.borrow(py);
                let registry = app_borrow.plugin_registry.borrow();
                registry.contains(type_ptr, py)
            };

            if already_added {
                // Skip this plugin - it was already added in a previous generation
                // This prevents "RecreationAttempt" errors with winit and other singleton plugins
                eprintln!("   Skipping already-added plugin: {}", plugin_type.name()?);
                continue;
            }

            // Register the plugin type in the PyApp struct
            {
                let app_borrow = pyself.borrow(py);
                app_borrow.plugin_registry.borrow_mut().insert(type_ptr, py);
            }

            // If this is a PluginGroupBuilder, also register its source type (e.g., DefaultPlugins)
            // This prevents duplicates when mixing DefaultPlugins and DefaultPlugins().build()
            if plugin_instance.is_instance_of::<PyPluginGroupBuilder>() {
                let builder = plugin_instance.cast_exact::<PyPluginGroupBuilder>()?;
                if let Some(source_type_id) = builder.borrow().source_type {
                    let app_borrow = pyself.borrow(py);
                    app_borrow
                        .plugin_registry
                        .borrow_mut()
                        .insert(source_type_id.as_ptr(), py);
                }
            }

            // Call the appropriate method based on plugin type:
            // - Plugin: use PluginBridge if registered, otherwise call build(app)
            // - PluginGroupBuilder: build(app)
            // - PluginGroup (like DefaultPlugins): _apply_to_app(app)
            //
            // During reload (is_reload_temp), skip built-in/bridge plugins that need
            // BEVY_APPS access (which temp apps lack), but let custom Python plugins
            // run build() so their systems/resources are captured in pending collections.
            let is_reload = pyself.borrow(py).is_reload_temp.get();

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
                    if let Some(bridge) = plugin_registry::get_by_py_type(type_ptr) {
                        // Use the PluginBridge to build the plugin
                        pyself
                            .borrow(py)
                            .with_bevy_app(|bevy_app| bridge.build(&plugin_instance, bevy_app))?;
                    } else {
                        // Fall back to Python build(app) method for custom plugins
                        plugin_instance.call_method1("build", (app_bound,))?;
                    }
                } else {
                    // Reload: only run build() for custom Python plugins.
                    // Skip bridge-backed plugins AND native Rust plugins
                    // (both call with_bevy_app which panics on temp apps
                    // because BEVY_APPS is already borrowed by app.update()).
                    let has_bridge = plugin_registry::get_by_py_type(type_ptr).is_some();
                    let is_native = plugin_type
                        .getattr("__module__")
                        .and_then(|m| m.extract::<String>())
                        .map(|m| m.starts_with("_pybevy") || m == "builtins")
                        .unwrap_or(false);

                    if !has_bridge && !is_native {
                        // Custom Python plugin — call build() to capture
                        // systems/resources in pending collections
                        plugin_instance.call_method1("build", (app_bound,))?;
                    }
                }
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

        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = pyself.get_app_mut(&mut apps)?;

            PyWorld::with_temporary(app.world_mut(), py, |py_world| {
                py_world.insert_resource(py, resource)?;
                Ok(pyself.into())
            })
        })
    }

    /// Initialize a resource with default values and insert it into the app
    pub fn init_resource(
        pyself: PyRef<'_, Self>,
        py: Python,
        resource: Bound<'_, PyAny>,
    ) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;

        // Skip during hot reload — resources are preserved or re-inserted via insert_resource
        if pyself.is_reload_temp.get() {
            return Ok(pyself.into());
        }

        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = pyself.get_app_mut(&mut apps)?;

            PyWorld::with_temporary(app.world_mut(), py, |py_world| {
                py_world.init_resource(py, resource)?;
                Ok(pyself.into())
            })
        })
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

        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = pyself.get_app_mut(&mut apps)?;

            MessageRegistry::register_message(py, &message_type, app)?;

            Ok(pyself.into())
        })
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

        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = pyself.get_app_mut(&mut apps)?;
            let world_mut = app.world_mut();

            ObserverRegistry::register_observer(py, &observer, world_mut)?;

            Ok(pyself.into())
        })
    }

    pub fn init_state(
        pyself: PyRef<'_, Self>,
        py: Python,
        state_type: Bound<'_, PyType>,
    ) -> PyResult<Py<PyApp>> {
        pyself.ensure_active()?;

        // States are already registered from initial load; skip during hot reload
        if pyself.is_reload_temp.get() {
            return Ok(pyself.into());
        }

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

        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = pyself.get_app_mut(&mut apps)?;

            // Insert State<S> resource with default value
            let state_resource = PyState::new(py, default_state.clone_ref(py))?;

            // Insert NextState<S> resource (starts as Unchanged)
            let next_state_resource = PyNextState::new(py, state_type_clone.unbind())?;

            // Use PyWorld to insert the resources
            PyWorld::with_temporary(app.world_mut(), py, |py_world| {
                py_world.insert_resource(py, state_resource.bind(py).as_any().clone())?;
                py_world.insert_resource(py, next_state_resource.bind(py).as_any().clone())?;
                Ok(())
            })
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

        // States are already registered from initial load; skip during hot reload
        if pyself.is_reload_temp.get() {
            return Ok(pyself.into());
        }

        // Get state type from the value
        let state_type = initial_state.bind(py).get_type();

        // Validate it's a @state decorated type
        if !state_type.hasattr("__pybevy_state__")? {
            return Err(PyTypeError::new_err(format!(
                "Type '{}' is not a valid state type. Did you forget the @state decorator?",
                state_type.name()?
            )));
        }

        let state_type_unbind = state_type.unbind();

        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = pyself.get_app_mut(&mut apps)?;

            // Insert State<S> resource with provided value
            let state_resource = PyState::new(py, initial_state)?;

            // Insert NextState<S> resource (starts as Unchanged)
            let next_state_resource = PyNextState::new(py, state_type_unbind.clone_ref(py))?;

            // Use PyWorld to insert the resources
            PyWorld::with_temporary(app.world_mut(), py, |py_world| {
                py_world.insert_resource(py, state_resource.bind(py).as_any().clone())?;
                py_world.insert_resource(py, next_state_resource.bind(py).as_any().clone())?;
                Ok(())
            })
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

        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = self.get_app_mut(&mut apps)?;

            PyWorld::with_temporary(app.world_mut(), py, |py_world| {
                let world_obj = Py::new(py, py_world.duplicate())?;
                callback
                    .call1((world_obj.bind(py),))?
                    .unbind()
                    .into_py_any(py)?;
                Ok(())
            })
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

        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = self.get_app_mut(&mut apps)?;

            PyWorld::with_temporary(app.world_mut(), py, |py_world| {
                let world_obj = Py::new(py, py_world.duplicate())?;
                world_obj.borrow(py).run_system_once(func)?;
                Ok(())
            })
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

        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = self.get_app_mut(&mut apps)?;

            PyWorld::with_temporary(app.world_mut(), py, |py_world| {
                let world_obj = Py::new(py, py_world.duplicate())?;
                for func in funcs.iter() {
                    world_obj.borrow(py).run_system_once(func)?;
                }
                Ok(())
            })
        })
    }

    /// Initialize the app by running startup systems (PyBevy-specific convenience method)
    pub fn initialize(&self, py: Python) -> PyResult<()> {
        self.ensure_active()?;

        // Capture app_id and is_consumed before detaching GIL (cannot access self inside detach closure)
        let app_id = self.app_id;
        let is_consumed = self.is_consumed.get();

        // Release GIL while running initialization (required to avoid deadlock with Python systems)
        py.detach(|| {
            // Access the thread-local App
            BEVY_APPS.with(|apps_cell| {
                let mut apps = apps_cell.borrow_mut();
                let app = apps.get_mut(&app_id).ok_or_else(|| {
                    if is_consumed {
                        PyRuntimeError::new_err(
                            "Cannot perform operation after run() has been called",
                        )
                    } else {
                        PyRuntimeError::new_err(
                            "App is not initialized. This may occur if the app was accessed from a \
                            different thread, or if there was an error during app initialization.",
                        )
                    }
                })?;
                // Run the startup schedule
                app.finish();
                app.cleanup();
                Ok::<(), PyErr>(())
            })
        })?;

        Ok(())
    }

    /// Run the app update loop once
    pub fn update(&self, py: Python) -> PyResult<()> {
        self.ensure_active()?;

        // Clear any previous errors before running
        {
            let mut error_lock = self.system_error.lock().unwrap();
            error_lock.clear();
        }

        // Capture app_id and is_consumed before detaching GIL (cannot access self inside detach closure)
        let app_id = self.app_id;
        let is_consumed = self.is_consumed.get();

        // Release GIL while running update (required to avoid deadlock with Python systems)
        py.detach(|| {
            // Access the thread-local App
            BEVY_APPS.with(|apps_cell| {
                let mut apps = apps_cell.borrow_mut();
                let app = apps.get_mut(&app_id).ok_or_else(|| {
                    if is_consumed {
                        PyRuntimeError::new_err(
                            "Cannot perform operation after run() has been called",
                        )
                    } else {
                        PyRuntimeError::new_err(
                            "App is not initialized. This may occur if the app was accessed from a \
                            different thread, or if there was an error during app initialization.",
                        )
                    }
                })?;
                app.update();
                Ok::<(), PyErr>(())
            })
        })?;

        // Check if any system errors occurred and raise them
        raise_collected_errors(py, &self.system_error)?;

        Ok(())
    }

    pub fn finish(&self, py: Python) -> PyResult<()> {
        self.ensure_active()?;

        let app_id = self.app_id;
        let is_consumed = self.is_consumed.get();

        py.detach(|| {
            BEVY_APPS.with(|apps_cell| {
                let mut apps = apps_cell.borrow_mut();
                let app = apps.get_mut(&app_id).ok_or_else(|| {
                    if is_consumed {
                        PyRuntimeError::new_err(
                            "Cannot perform operation after run() has been called",
                        )
                    } else {
                        PyRuntimeError::new_err(
                            "App is not initialized. This may occur if the app was accessed from a \
                            different thread, or if there was an error during app initialization.",
                        )
                    }
                })?;
                app.finish();
                Ok::<(), PyErr>(())
            })
        })?;

        Ok(())
    }

    pub fn cleanup(&self, py: Python) -> PyResult<()> {
        self.ensure_active()?;

        let app_id = self.app_id;
        let is_consumed = self.is_consumed.get();

        py.detach(|| {
            BEVY_APPS.with(|apps_cell| {
                let mut apps = apps_cell.borrow_mut();
                let app = apps.get_mut(&app_id).ok_or_else(|| {
                    if is_consumed {
                        PyRuntimeError::new_err(
                            "Cannot perform operation after run() has been called",
                        )
                    } else {
                        PyRuntimeError::new_err(
                            "App is not initialized. This may occur if the app was accessed from a \
                            different thread, or if there was an error during app initialization.",
                        )
                    }
                })?;
                app.cleanup();
                Ok::<(), PyErr>(())
            })
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
        let is_consumed = self.is_consumed.get();

        py.detach(|| {
            BEVY_APPS.with(|apps_cell| {
                let mut apps = apps_cell.borrow_mut();
                let app = apps.get_mut(&app_id).ok_or_else(|| {
                    if is_consumed {
                        PyRuntimeError::new_err(
                            "Cannot perform operation after run() has been called",
                        )
                    } else {
                        PyRuntimeError::new_err(
                            "App is not initialized. This may occur if the app was accessed from a \
                            different thread, or if there was an error during app initialization.",
                        )
                    }
                })?;

                // Use hot reload infrastructure to clear scene
                clear_entities_and_resources(app.world_mut());

                Ok::<(), PyErr>(())
            })
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
                )));
            }
        }

        // Check if already consumed and mark as consumed
        if self.is_consumed.get() {
            return Err(PyRuntimeError::new_err("run() has already been called"));
        }
        self.is_consumed.set(true);

        // Reset Python's SIGINT handler to default before detaching GIL
        // This allows Bevy's native TerminalCtrlCHandlerPlugin to handle Ctrl-C directly
        // without Python intercepting the signal first
        if let Ok(signal_module) = PyModule::import(py, "signal") {
            let sig_dfl = signal_module.getattr("SIG_DFL")?;
            let sigint = signal_module.getattr("SIGINT")?;
            if let Err(e) = signal_module.call_method1("signal", (sigint, sig_dfl)) {
                eprintln!("Warning: Failed to reset Python's SIGINT handler: {}", e);
            }
        }

        // Capture app_id before detaching GIL
        let app_id = self.app_id;
        let error_state = self.system_error.clone();

        // If hot reload is NOT active, add a Last-schedule system that triggers
        // AppExit when a system error is detected, so app.run() exits promptly.
        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            if let Some(app) = apps.get_mut(&app_id) {
                let has_hot_reload = app.world().get_resource::<HotReloadGeneration>().is_some();
                if !has_hot_reload {
                    app.insert_resource(SystemErrorCheck {
                        errors: error_state.clone(),
                    });
                    app.add_systems(Last, check_system_errors_and_exit);
                }
            }
        });

        // Release GIL before running to avoid deadlock with Python systems
        py.detach(|| {
            // Take the app out of thread_local HashMap and run it (consumes the app)
            // IMPORTANT: Drop the borrow_mut BEFORE app.run() to avoid holding the
            // RefCell for the entire game loop, which would panic if anything
            // (GC finalizers, atexit, other PyApp instances) touches BEVY_APPS.
            let mut app = BEVY_APPS.with(|apps_cell| {
                apps_cell
                    .borrow_mut()
                    .remove(&app_id)
                    .expect("App should exist when run() is called")
            });
            app.run();

            // Clear the system parameter cache after the app finishes
            // to prevent stale entries when function objects are recycled
            clear_system_param_cache();
        });

        // After the event loop exits, check for system errors and raise them
        raise_collected_errors(py, &error_state)?;

        Ok(())
    }

    /// Check if a plugin of a given type has been added to the app
    pub fn is_plugin_added(&self, py: Python, plugin_type: Bound<'_, PyType>) -> PyResult<bool> {
        self.ensure_active()?;

        let type_ptr = plugin_type.as_ptr() as *const PyTypeObject;
        let is_added = self.plugin_registry.borrow().contains(type_ptr, py);

        Ok(is_added)
    }

    /// Get the hot reload state for CLI integration
    /// This allows the CLI watcher to signal reloads
    #[getter]
    pub fn _state(&self, py: Python) -> PyResult<Py<PyAppReloadState>> {
        let reload_state = PyAppReloadState::new(self.hot_reload_state.clone());
        Py::new(py, reload_state)
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

        // Add the hot reload system to the app if not already added
        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = pyself.get_app_mut(&mut apps)?;

            // Add hot reload checking system
            add_hot_reload_system(
                app,
                pyself.hot_reload_state.clone(),
                pyself.system_error.clone(),
            );
            Ok::<(), PyErr>(())
        })?;

        Ok(pyself.into())
    }

    /// Check if the app should exit
    ///
    /// Returns the AppExit value if an exit has been requested, None otherwise.
    /// This allows checking exit status programmatically for conditional logic or tests.
    /// Can be called before or after run() to check the exit status.
    pub fn should_exit(&self, py: Python) -> PyResult<Option<Py<PyAppExit>>> {
        BEVY_APPS.with(|apps_cell| {
            let apps = apps_cell.borrow();
            let app = apps.get(&self.app_id).ok_or_else(|| {
                PyRuntimeError::new_err(
                    "App is not initialized. This may occur if the app was accessed from a \
                    different thread, or if there was an error during app initialization.",
                )
            })?;

            match app.should_exit() {
                Some(exit) => {
                    let py_exit = Py::new(py, (PyAppExit::from(exit), PyMessage))?;
                    Ok(Some(py_exit))
                }
                None => Ok(None),
            }
        })
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

        BEVY_APPS.with(|apps_cell| {
            let mut apps = apps_cell.borrow_mut();
            let app = pyself.get_app_mut(&mut apps)?;

            match label {
                PyStage::Startup => app.init_schedule(Startup),
                PyStage::Update => app.init_schedule(Update),
                PyStage::Last => app.init_schedule(Last),
                PyStage::FixedUpdate => app.init_schedule(FixedUpdate),
                PyStage::Main => app.init_schedule(Main),
                PyStage::First => app.init_schedule(First),
                PyStage::PreUpdate => app.init_schedule(PreUpdate),
                PyStage::PostUpdate => app.init_schedule(PostUpdate),
                PyStage::PreStartup => app.init_schedule(PreStartup),
                PyStage::PostStartup => app.init_schedule(PostStartup),
                PyStage::FixedFirst => app.init_schedule(FixedFirst),
                PyStage::FixedPreUpdate => app.init_schedule(FixedPreUpdate),
                PyStage::FixedPostUpdate => app.init_schedule(FixedPostUpdate),
                PyStage::FixedLast => app.init_schedule(FixedLast),
                PyStage::SimTick => app.init_schedule(SimTick),
            };

            Ok(pyself.into())
        })
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
        {
            let mut error_lock = self.system_error.lock().unwrap();
            error_lock.clear();
        }

        let app_id = self.app_id;
        let is_consumed = self.is_consumed.get();

        // Release GIL while running schedule (required to avoid deadlock with Python systems)
        py.detach(|| {
            BEVY_APPS.with(|apps_cell| {
                let mut apps = apps_cell.borrow_mut();
                let app = apps.get_mut(&app_id).ok_or_else(|| {
                    if is_consumed {
                        PyRuntimeError::new_err(
                            "Cannot perform operation after run() has been called",
                        )
                    } else {
                        PyRuntimeError::new_err(
                            "App is not initialized. This may occur if the app was accessed from a \
                            different thread, or if there was an error during app initialization.",
                        )
                    }
                })?;

                stage.run_on_world(app.world_mut());

                Ok::<(), PyErr>(())
            })
        })?;

        // Check if any system errors occurred and raise them
        raise_collected_errors(py, &self.system_error)?;

        Ok(())
    }

    /// Configure system set ordering and relationships
    ///
    /// STUB: This method is not yet implemented. Full implementation requires:
    /// - System set type wrappers
    /// - Before/after relationship tracking
    /// - Integration with DynamicSystem
    ///
    /// For now, use schedule labels (First, PreUpdate, Update, PostUpdate, Last)
    /// to control execution order at a coarse granularity.
    ///
    /// Future usage will be:
    ///   app.configure_sets(Stage.Update, MySet.before(OtherSet))
    pub fn configure_sets(&self, _schedule: PyStage, _sets: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(PyRuntimeError::new_err(
            "configure_sets() is not yet implemented. Use schedule labels (First, PreUpdate, \
            Update, PostUpdate, Last) for coarse-grained execution ordering.",
        ))
    }
}

/// Implement Drop to ensure the Bevy App is properly cleaned up
/// This removes the App from thread-local storage and drops it explicitly
/// to ensure task pools and worker threads are shut down before Python TLS cleanup
impl Drop for PyApp {
    fn drop(&mut self) {
        // Only drop the App if it hasn't been consumed by run()
        // If run() was called, the App is already fully consumed and cleaned up
        if !self.is_consumed.get() && !self.is_reload_temp.get() {
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
                    Ok(mut apps) => apps.remove(&self.app_id),
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
