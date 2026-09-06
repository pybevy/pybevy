//! Native Bevy plugin for integrating Python systems into Rust Bevy applications.
//!
//! This module provides a `PyBevyPlugin` that can be added to a native Rust Bevy app,
//! allowing you to write game systems in Python while the main application is in Rust.
//!
//! # Example
//!
//! ```no_run
//! use bevy::prelude::*;
//! use pybevy::PyBevyPlugin;
//! use pybevy::PyStage;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(PyBevyPlugin::new("my_game.systems"))
//!         .run();
//! }
//! ```
//!
//! Then in your Python module (`my_game/systems.py`):
//!
//! ```python
//! from pybevy import Query, Transform, Time, Mut, Quat
//!
//! def rotate_cubes(query: Query[Mut[Transform]], time: Time):
//!     for transform in query:
//!         transform.rotation *= Quat.from_rotation_y(time.delta_secs())
//! ```

#[cfg(feature = "native-hot-reload")]
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(feature = "native-hot-reload")]
use bevy::prelude::{Res, Resource};
use bevy::{
    app::{App, Last, Plugin},
    ecs::schedule::{IntoScheduleConfigs, Schedules},
    log::warn,
};
use pybevy_core::{
    ComponentBridge, ensure_asset_access_registry, register_wrapped_reflect_types,
    registry::global_registry,
};
#[cfg(feature = "native-hot-reload")]
use pybevy_reload::FileWatcher;
use pybevy_reload::{
    HotReloadGeneration, ReloadGenerationSet, SystemStage, generation_matches, startup_or_reload,
};
use pyo3::{exceptions::PyImportError, prelude::*, types::PyList};

use crate::{
    _pybevy,
    app::{
        PyStage,
        hot_reload::{
            state::{HotReloadResource, HotReloadState},
            systems::{check_hot_reload_system, handle_f5_reload_system},
        },
    },
    ecs::system_interpreter::{MainDynamicSystem as DynamicSystem, new_main_system},
};

/// Global flag to ensure Python is initialized only once
static PYTHON_INITIALIZED: OnceLock<()> = OnceLock::new();

/// Initialize the Python interpreter (called automatically, safe to call multiple times).
///
/// Registers `_pybevy` in the init table and injects it into `sys.modules` as
/// `pybevy._pybevy` so that relative imports (`from . import _pybevy`) in the
/// pybevy package use the same type objects as this binary.
///
/// If the Python interpreter was already initialized (e.g. by pyo3's `auto-initialize`
/// feature in tests), `append_to_inittab!` cannot be used. In that case we create
/// the `_pybevy` module manually via [`init_module`] and register it in `sys.modules`.
fn ensure_python_initialized() {
    PYTHON_INITIALIZED.get_or_init(|| {
        // Safety: Py_IsInitialized is a read-only query with no side effects.
        let already_initialized = unsafe { pyo3::ffi::Py_IsInitialized() != 0 };

        if !already_initialized {
            pyo3::append_to_inittab!(_pybevy);
            Python::initialize();
        }

        // The pybevy package uses relative imports: `from . import _pybevy`
        // which resolves to `pybevy._pybevy`. Pre-populate sys.modules so
        // Python finds the in-process module instead of the .so on disk.
        Python::attach(|py| {
            let module = if !already_initialized {
                py.import("_pybevy")
                    .expect("Failed to import _pybevy from inittab")
            } else {
                // Python was already initialized (e.g. auto-initialize in tests).
                // append_to_inittab! can't be called after init, so create the
                // module manually and populate it with init_module.
                let m = PyModule::new(py, "_pybevy").expect("Failed to create _pybevy module");
                crate::init_module(&m).expect("Failed to populate _pybevy module");
                let sys = py.import("sys").expect("Failed to import sys");
                sys.getattr("modules")
                    .expect("Failed to get sys.modules")
                    .set_item("_pybevy", &m)
                    .expect("Failed to register _pybevy in sys.modules");
                m
            };
            let sys = py.import("sys").expect("Failed to import sys");
            let modules = sys.getattr("modules").expect("Failed to get sys.modules");
            modules
                .set_item("pybevy._pybevy", module)
                .expect("Failed to register pybevy._pybevy in sys.modules");
        });
    });
}

/// Why a Python system could not be loaded.
///
/// Distinguishes an absent function from a real failure so auto-discovery can
/// skip the former without also swallowing import errors or bad signatures.
enum SystemLoadError {
    /// The module imported cleanly but defines no such function.
    MissingFunction,
    /// Import failure, bad signature, or any other interpreter error.
    Failed(PyErr),
}

/// Report a system that failed to load.
///
/// An auto-discovery probe finding no function is normal and stays silent.
/// An explicitly requested system that is missing is a configuration error, so
/// it is reported rather than silently dropped.
fn report_system_load_error(module: &str, function: &str, optional: bool, error: SystemLoadError) {
    match error {
        SystemLoadError::MissingFunction if optional => {}
        SystemLoadError::MissingFunction => {
            warn!("Module '{module}' has no function '{function}': system not added");
        }
        SystemLoadError::Failed(error) => {
            warn!("Failed to load Python system '{module}.{function}': {error}");
        }
    }
}

/// Builder for configuring Python systems before adding them to a Bevy app
#[derive(Clone)]
pub struct PySystemBuilder {
    module_name: String,
    function_name: String,
    stage: PyStage,
    /// Auto-discovery probe: absence is expected and stays silent.
    optional: bool,
}

impl PySystemBuilder {
    /// Create a new system builder for a Python function
    pub fn new(module_name: impl Into<String>, function_name: impl Into<String>) -> Self {
        Self {
            module_name: module_name.into(),
            function_name: function_name.into(),
            stage: PyStage::Update,
            optional: false,
        }
    }

    /// Set which Bevy schedule stage this system should run in
    pub fn in_stage(mut self, stage: PyStage) -> Self {
        self.stage = stage;
        self
    }

    /// Mark this system as an auto-discovery probe whose absence is not an error
    fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// Build the DynamicSystem with default generation (0) and a fresh error state
    fn build(self) -> Result<(DynamicSystem, PyStage), SystemLoadError> {
        self.build_with(0, Arc::new(Mutex::new(Vec::new())))
    }

    /// Build the DynamicSystem with a specific generation and shared error state
    fn build_with(
        self,
        generation: u32,
        error_state: Arc<Mutex<Vec<PyErr>>>,
    ) -> Result<(DynamicSystem, PyStage), SystemLoadError> {
        ensure_python_initialized();

        Python::attach(|py| {
            // Import the module
            let module = py.import(&self.module_name).map_err(|e| {
                SystemLoadError::Failed(PyImportError::new_err(format!(
                    "Failed to import module '{}': {}",
                    self.module_name, e
                )))
            })?;

            // Get the function
            let func = module
                .getattr(&self.function_name)
                .map_err(|_| SystemLoadError::MissingFunction)?;

            // Create DynamicSystem
            let system_stage = if self.stage.is_startup() {
                SystemStage::Startup
            } else {
                SystemStage::UpdateOrLast
            };

            // Native-plugin systems run on a separate mini-app with no LastSystemError
            // drain; a throwaway buffer keeps the error path off the world.
            let dynamic_system = new_main_system(
                func.unbind(),
                generation,
                error_state,
                Arc::new(Mutex::new(None)),
                system_stage,
            )
            .map_err(SystemLoadError::Failed)?;

            Ok((dynamic_system, self.stage))
        })
    }
}

/// Python source for the native plugin hot reload loader.
///
/// Self-contained: no dependency on the pybevy Python package so that
/// `cargo test` works without a matching install. Loads the scene module
/// via `importlib.util.spec_from_file_location` so that the module is
/// registered in `sys.modules` (unlike `runpy.run_path`, which only
/// returns a globals dict). This makes `import <stem>` work, keeps
/// pickle round-trips functional, and preserves reload identity.
///
/// Protocol (matches `perform_reload` in hot_reload.rs):
///   1. `loader()` → re-imports module from source, returns `configurator`
///   2. `configurator(app)` → calls `app.add_systems(stage, (func,))` for each system
const NATIVE_LOADER_PY: &str = r#"
def _make_native_loader(module_name, systems):
    import importlib, importlib.util, os, sys, sysconfig

    def _flush_user_modules(project_dir):
        """Remove all user project modules from sys.modules."""
        project_prefix = os.path.realpath(project_dir) + os.sep
        stdlib_prefix = os.path.realpath(sysconfig.get_paths()['stdlib']) + os.sep
        to_remove = []
        for name, mod in list(sys.modules.items()):
            if name.startswith(('pybevy.', '_pybevy', 'pybevy')):
                continue
            fpath = getattr(mod, '__file__', None)
            if fpath is None:
                continue
            fpath = os.path.realpath(fpath)
            if not fpath.startswith(project_prefix):
                continue
            if 'site-packages' in fpath or 'dist-packages' in fpath:
                continue
            if fpath.startswith(stdlib_prefix):
                continue
            to_remove.append(name)
        for name in to_remove:
            del sys.modules[name]
        importlib.invalidate_caches()

    def loader():
        spec = importlib.util.find_spec(module_name)
        if spec is None or spec.origin is None:
            raise ImportError(f"Cannot find source for module '{module_name}'")

        # Flush all user modules under the module's parent directory
        project_dir = os.path.dirname(os.path.realpath(spec.origin))
        _flush_user_modules(project_dir)

        # Re-resolve the spec after flushing so importlib re-reads the source
        # file from disk rather than reusing a cached spec.
        fresh_spec = importlib.util.spec_from_file_location(module_name, spec.origin)
        if fresh_spec is None or fresh_spec.loader is None:
            raise ImportError(
                f"Cannot build spec for module '{module_name}' at {spec.origin}"
            )

        module = importlib.util.module_from_spec(fresh_spec)
        # Register in sys.modules BEFORE exec_module so that the module can
        # reference itself during execution (e.g. dataclasses, pickle, and
        # circular intra-module imports all rely on this).
        sys.modules[module_name] = module
        try:
            # Compile the current source directly instead of delegating to
            # exec_module(), which may reuse timestamp-based bytecode for a
            # same-second, same-size hot-reload edit.
            source = fresh_spec.loader.get_source(module_name)
            if source is None:
                raise ImportError(
                    f"Cannot read source for module '{module_name}' at {spec.origin}"
                )
            code = compile(source, fresh_spec.origin, "exec")
            exec(code, module.__dict__)
        except BaseException:
            # If execution fails, leave no half-initialised module behind.
            sys.modules.pop(module_name, None)
            raise

        mod_globals = module.__dict__

        def configurator(app):
            for func_name, stage in systems:
                func = mod_globals.get(func_name)
                if func is not None:
                    app.add_systems(stage, (func,))
            return app
        return configurator

    return loader
"#;

#[cfg(feature = "native-hot-reload")]
#[derive(Resource)]
struct NativeFileWatcher(FileWatcher);

#[cfg(feature = "native-hot-reload")]
fn poll_native_file_watcher(
    watcher: Option<Res<'_, NativeFileWatcher>>,
    reload: Res<'_, HotReloadResource>,
) {
    let Some(watcher) = watcher else {
        return;
    };
    let mut changed_paths = Vec::new();
    loop {
        match watcher.0.try_recv() {
            Ok(Some(batch)) => changed_paths.extend_from_slice(batch.paths()),
            Ok(None) => break,
            Err(error) => {
                eprintln!("[Hot Reload] File watcher error: {error}");
                break;
            }
        }
    }
    if changed_paths.is_empty() {
        return;
    }

    let mode = reload.state.get_default_mode();
    eprintln!(
        "[Hot Reload] File changes detected in {} Python file(s), requesting {:?} reload",
        changed_paths.len(),
        mode
    );
    reload.state.request_reload(mode);
}

/// Native Bevy plugin for integrating Python systems into Rust applications
///
/// This plugin allows you to write Bevy systems in Python and integrate them
/// seamlessly into a native Rust Bevy application.
///
/// # Example
///
/// ```no_run
/// use bevy::prelude::*;
/// use pybevy::PyBevyPlugin;
/// use pybevy::PyStage;
///
/// fn main() {
///     App::new()
///         .add_plugins(DefaultPlugins)
///         .add_plugins(
///             PyBevyPlugin::new("game_logic")
///                 .with_system("update_players", PyStage::Update)
///                 .with_system("spawn_enemies", PyStage::Startup)
///         )
///         .run();
/// }
/// ```
pub struct PyBevyPlugin {
    module_name: String,
    systems: Vec<PySystemBuilder>,
    python_paths: Vec<String>,
    component_bridges: Vec<Arc<dyn ComponentBridge>>,
    hot_reload: bool,
}

impl PyBevyPlugin {
    /// Create a new PyBevy plugin that will load systems from the specified Python module
    ///
    /// If no systems are specified, the plugin looks for functions named `startup`,
    /// `update`, and `last` and adds the ones it finds to the corresponding schedules.
    /// Use [`Self::with_system`] to specify custom function names.
    pub fn new(module_name: impl Into<String>) -> Self {
        Self {
            module_name: module_name.into(),
            systems: Vec::new(),
            python_paths: Vec::new(),
            component_bridges: Vec::new(),
            hot_reload: false,
        }
    }

    /// Add a directory to Python's `sys.path` for module resolution.
    ///
    /// Call this when the Python module is not in the current working directory.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use pybevy::PyBevyPlugin;
    /// PyBevyPlugin::new("example_systems")
    ///     .with_python_path("examples/rust")
    ///     .with_startup_system("setup");
    /// ```
    pub fn with_python_path(mut self, path: impl Into<String>) -> Self {
        self.python_paths.push(path.into());
        self
    }

    /// Add a specific system function from the Python module
    pub fn with_system(mut self, function_name: impl Into<String>, stage: PyStage) -> Self {
        let builder = PySystemBuilder::new(self.module_name.clone(), function_name).in_stage(stage);
        self.systems.push(builder);
        self
    }

    /// Convenience method to add a system to the Startup stage
    pub fn with_startup_system(self, function_name: impl Into<String>) -> Self {
        self.with_system(function_name, PyStage::Startup)
    }

    /// Convenience method to add a system to the Update stage
    pub fn with_update_system(self, function_name: impl Into<String>) -> Self {
        self.with_system(function_name, PyStage::Update)
    }

    /// Convenience method to add a system to the Last stage
    pub fn with_last_system(self, function_name: impl Into<String>) -> Self {
        self.with_system(function_name, PyStage::Last)
    }

    /// Auto-discover systems in the Python module
    ///
    /// This will attempt to import functions named `startup`, `update`, and `last`
    /// from the module and add them to the corresponding Bevy schedules.
    /// Functions that don't exist will be silently skipped.
    pub fn with_auto_discovery(mut self) -> Self {
        // Try to find standard system functions
        for (func_name, stage) in [
            ("startup", PyStage::Startup),
            ("update", PyStage::Update),
            ("last", PyStage::Last),
        ] {
            let builder = PySystemBuilder::new(self.module_name.clone(), func_name).in_stage(stage);
            self.systems.push(builder);
        }
        self
    }

    /// Enable hot reload support.
    ///
    /// When enabled:
    /// - **F5** triggers a full reload (re-imports Python module, re-runs Startup systems)
    /// - **F6** toggles the default reload mode between Full and Partial
    /// - With the `native-hot-reload` feature, `.py` file changes in `python_paths`
    ///   directories automatically trigger a reload
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use pybevy::PyBevyPlugin;
    /// PyBevyPlugin::new("game")
    ///     .with_python_path("scripts")
    ///     .with_startup_system("setup")
    ///     .with_update_system("update")
    ///     .with_hot_reload()
    /// # ;
    /// ```
    pub fn with_hot_reload(mut self) -> Self {
        self.hot_reload = true;
        self
    }

    /// Register a Rust component for use from Python systems.
    ///
    /// The component must derive `PyComponent` (from `pybevy_macros`), which generates
    /// the necessary bridge implementation.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bevy::prelude::*;
    /// use pybevy::PyBevyPlugin;
    /// use pybevy_macros::PyComponent;
    ///
    /// #[derive(Component, Default, Clone, Debug, PyComponent)]
    /// struct Health {
    ///     value: f32,
    ///     max: f32,
    /// }
    ///
    /// fn main() {
    ///     App::new()
    ///         .add_plugins(DefaultPlugins)
    ///         .add_plugins(
    ///             PyBevyPlugin::new("game_systems")
    ///                 .register_component(HealthBridge)
    ///                 .with_update_system("damage_system")
    ///         )
    ///         .run();
    /// }
    /// ```
    pub fn register_component(mut self, bridge: impl ComponentBridge) -> Self {
        self.component_bridges.push(Arc::new(bridge));
        self
    }
}

impl Plugin for PyBevyPlugin {
    fn build(&self, app: &mut App) {
        ensure_python_initialized();
        ensure_asset_access_registry(app.world_mut());

        // Reflect-register all bridged bevy types so MCP/editor tooling can
        // resolve them by name even without bevy's reflect_auto_register
        register_wrapped_reflect_types(app.world());

        // Register component bridges in the global registry and inject their
        // Python types into the _pybevy module so Python can import them
        if !self.component_bridges.is_empty() {
            for bridge in &self.component_bridges {
                global_registry::register_component_bridge_arc(Arc::clone(bridge));
            }

            // Inject Python types into the _pybevy module so Python systems can import them:
            //   from pybevy._pybevy import Health
            Python::attach(|py| {
                let module = py
                    .import("_pybevy")
                    .expect("Failed to import _pybevy for type injection");
                for bridge in &self.component_bridges {
                    let py_type = bridge.py_type(py);
                    let name = bridge.name();
                    module.setattr(name, py_type).unwrap_or_else(|e| {
                        eprintln!(
                            "Warning: Failed to inject '{}' into _pybevy module: {}",
                            name, e
                        );
                    });
                }
            });
        }

        // Add custom paths to sys.path
        if !self.python_paths.is_empty() {
            Python::attach(|py| {
                let sys = py.import("sys").expect("Failed to import sys");
                let sys_path = sys.getattr("path").expect("Failed to get sys.path");
                let path = sys_path
                    .cast_into::<PyList>()
                    .expect("sys.path is not a list");
                for p in &self.python_paths {
                    path.insert(0, p).expect("Failed to insert into sys.path");
                }
            });
        }

        // If no systems specified, try auto-discovery
        let systems_to_add = if self.systems.is_empty() {
            vec![
                PySystemBuilder::new(self.module_name.clone(), "startup")
                    .in_stage(PyStage::Startup)
                    .optional(),
                PySystemBuilder::new(self.module_name.clone(), "update")
                    .in_stage(PyStage::Update)
                    .optional(),
                PySystemBuilder::new(self.module_name.clone(), "last")
                    .in_stage(PyStage::Last)
                    .optional(),
            ]
        } else {
            self.systems.clone()
        };

        if self.hot_reload {
            self.build_with_hot_reload(app, &systems_to_add);
        } else {
            self.build_without_hot_reload(app, systems_to_add);
        }
    }
}

impl PyBevyPlugin {
    /// Build without hot reload: current behavior, systems added directly
    fn build_without_hot_reload(&self, app: &mut App, systems_to_add: Vec<PySystemBuilder>) {
        for builder in systems_to_add {
            let (module, function, optional) = (
                builder.module_name.clone(),
                builder.function_name.clone(),
                builder.optional,
            );
            match builder.build() {
                Ok((dynamic_system, stage)) => {
                    add_dynamic_system_to_schedule(app, dynamic_system, stage);
                }
                Err(error) => report_system_load_error(&module, &function, optional, error),
            }
        }
    }

    /// Build with hot reload: sets up reload infrastructure and adds systems with run conditions
    fn build_with_hot_reload(&self, app: &mut App, systems_to_add: &[PySystemBuilder]) {
        eprintln!(
            "[Hot Reload] Enabling hot reload for native plugin '{}'",
            self.module_name
        );

        // Create shared reload state and error state
        let reload_state = HotReloadState::new();
        let error_state: Arc<Mutex<Vec<PyErr>>> = Arc::new(Mutex::new(Vec::new()));

        // Insert hot reload resources
        let generation_counter = reload_state.generation();
        app.insert_resource(HotReloadResource::new(
            reload_state.clone(),
            error_state.clone(),
        ));
        app.insert_resource(HotReloadGeneration::new(generation_counter));

        // Create and register the Python loader function.
        // Loads the scene module via importlib.util so it ends up in
        // sys.modules (see NATIVE_LOADER_PY for details).
        let module_name = self.module_name.clone();
        Python::attach(|py| {
            // Define _make_native_loader in a temporary namespace
            let locals = pyo3::types::PyDict::new(py);
            let code =
                std::ffi::CString::new(NATIVE_LOADER_PY).expect("loader code contains null byte");
            py.run(&code, None, Some(&locals))
                .expect("Failed to define native loader Python code");
            let make_loader = locals
                .get_item("_make_native_loader")
                .expect("_make_native_loader not found")
                .expect("_make_native_loader not found");

            // Build the systems list: [(func_name, PyStage), ...]
            let systems_list: Vec<(String, Py<PyAny>)> = systems_to_add
                .iter()
                .map(|b| {
                    let stage_obj: Py<PyAny> = b
                        .stage
                        .into_pyobject(py)
                        .expect("Failed to convert stage")
                        .into_any()
                        .unbind();
                    (b.function_name.clone(), stage_obj)
                })
                .collect();

            // Call _make_native_loader(module_name, systems) to get the loader closure
            let loader_py = make_loader
                .call1((&module_name, systems_list))
                .expect("Failed to create native loader");

            reload_state.set_loader(loader_py.unbind());
        });

        // Add initial systems with generation-based run conditions
        let initial_generation = 0u32;
        for builder in systems_to_add {
            match builder
                .clone()
                .build_with(initial_generation, error_state.clone())
            {
                Ok((dynamic_system, stage)) => {
                    add_dynamic_system_to_schedule_with_run_condition(
                        app,
                        dynamic_system,
                        stage,
                        initial_generation,
                    );
                }
                Err(error) => report_system_load_error(
                    &builder.module_name,
                    &builder.function_name,
                    builder.optional,
                    error,
                ),
            }
        }

        // Ensure Last schedule exists
        if !app.world().resource::<Schedules>().contains(Last) {
            app.init_schedule(Last);
        }

        #[cfg(feature = "native-hot-reload")]
        if !self.python_paths.is_empty() {
            let paths = self.python_paths.iter().map(PathBuf::from).collect();
            match FileWatcher::with_defaults(paths) {
                Ok(watcher) => {
                    eprintln!(
                        "[Hot Reload] File watcher started for: {:?}",
                        self.python_paths
                    );
                    app.insert_resource(NativeFileWatcher(watcher));
                }
                Err(error) => {
                    eprintln!("[Hot Reload] Warning: Failed to start file watcher: {error}");
                }
            }
        }

        // Add F5/F6 key handler and reload check systems to Last.
        #[cfg(not(feature = "native-hot-reload"))]
        app.add_systems(
            Last,
            (handle_f5_reload_system, check_hot_reload_system).chain(),
        );
        #[cfg(feature = "native-hot-reload")]
        app.add_systems(
            Last,
            (
                handle_f5_reload_system,
                poll_native_file_watcher,
                check_hot_reload_system,
            )
                .chain(),
        );

        eprintln!("[Hot Reload] Native plugin hot reload ready (F5=reload, F6=toggle mode)");
    }
}

/// Add a DynamicSystem to the appropriate Bevy schedule (no run conditions)
fn add_dynamic_system_to_schedule(app: &mut App, dynamic_system: DynamicSystem, stage: PyStage) {
    app.add_systems(stage.intern_label(), dynamic_system);
}

/// Add a DynamicSystem to the appropriate Bevy schedule WITH generation-based run conditions
fn add_dynamic_system_to_schedule_with_run_condition(
    app: &mut App,
    dynamic_system: DynamicSystem,
    stage: PyStage,
    generation: u32,
) {
    let label = stage.intern_label();
    if stage.is_startup() {
        app.add_systems(
            label,
            dynamic_system
                .run_if(startup_or_reload(generation))
                .in_set(ReloadGenerationSet(generation)),
        );
    } else {
        app.add_systems(
            label,
            dynamic_system
                .run_if(generation_matches(generation))
                .in_set(ReloadGenerationSet(generation)),
        );
    }
}
