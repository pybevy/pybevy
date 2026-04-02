//! Plugin base class and bridge trait for PyBevy
//!
//! This module provides:
//! - `PyPlugin` - Base class for Python-facing plugins
//! - `PluginBridge` - Trait for building plugins into Bevy apps
//! - `plugin_registry` - Global registry for plugin bridges
//!
//! ## Architecture
//!
//! Python-facing plugins (`PyAudioPlugin`, `PyTransformPlugin`, etc.) are configuration
//! containers that can be defined in feature crates. The `PluginBridge` trait handles
//! the actual building of Bevy plugins.
//!
//! Key insight: `PluginBridge::build` takes `&mut bevy::app::App`, NOT `PyApp`.
//! This allows feature crates to implement plugin building without depending on
//! the main crate (which owns `PyApp`), avoiding circular dependencies.

use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, OnceLock, RwLock},
};

use bevy::app::App;
use pyo3::{
    exceptions::PyNotImplementedError,
    ffi::PyTypeObject,
    prelude::*,
    types::{PyDict, PyTuple, PyType},
};

/// Base class for Python-facing plugins.
///
/// Plugins can be implemented in two ways:
/// 1. **Python plugins**: Override `build(self, app)` to add systems, resources, etc.
/// 2. **Bridge plugins**: C-defined plugins (AudioPlugin, etc.) use PluginBridge
///
/// # Example
///
/// ```python
/// class MyPlugin(Plugin):
///     def build(self, app):
///         app.add_systems(Startup, my_system)
/// ```
#[pyclass(name = "Plugin", subclass, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyPlugin;

#[pymethods]
impl PyPlugin {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    pub fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        PyPlugin
    }

    /// Build the plugin by adding systems, resources, etc. to the app.
    ///
    /// This is called by `app.add_plugins()` for each plugin.
    /// Override this method in Python subclasses.
    /// C-defined plugins use PluginBridge instead.
    pub fn build(pyself: Bound<'_, Self>, _app: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(format!(
            "Plugin.build() not implemented for {}",
            pyself.get_type().name()?
        )))
    }
}

impl Default for PyPlugin {
    fn default() -> Self {
        PyPlugin
    }
}

/// Bridge trait for building plugins into Bevy apps.
///
/// Feature crates implement this trait for their plugin types.
/// The main crate's `PyApp.add_plugins` looks up bridges by Python type
/// and calls `build` with `&mut bevy::app::App`.
///
/// # Example
///
/// ```rust,ignore
/// pub struct AudioPluginBridge;
///
/// impl PluginBridge for AudioPluginBridge {
///     fn py_type_ptr(&self) -> *const PyTypeObject {
///         Python::attach(|py| PyAudioPlugin::type_object(py).as_type_ptr())
///     }
///
///     fn build(&self, py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
///         let plugin = py_plugin.extract::<PyRef<PyAudioPlugin>>()?;
///         app.add_plugins(bevy::audio::AudioPlugin::default());
///         Ok(())
///     }
///
///     fn name(&self) -> &'static str { "AudioPlugin" }
/// }
/// ```
/// Trait for Python plugin wrappers to implement their build logic.
///
/// Implement this on your `#[pyclass]` struct, then use `#[plugin_storage(BevyPlugin)]`
/// to generate the bridge and inventory registration automatically.
///
/// ```rust
/// impl PluginBuild for PyWindowPlugin {
///     fn build(py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
///         let config: PyRef<'_, PyWindowPlugin> = py_plugin.extract()?;
///         app.add_plugins(bevy::window::WindowPlugin::try_from(&*config)?);
///         Ok(())
///     }
/// }
/// ```
pub trait PluginBuild {
    fn build(py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()>;
}

pub trait PluginBridge: Send + Sync + 'static {
    /// Get the Rust TypeId of the Python plugin class.
    fn py_type_id(&self) -> TypeId;

    /// Get the Python type pointer for registry lookup.
    fn py_type_ptr(&self) -> *const PyTypeObject;

    /// Get the Python type object.
    fn py_type<'py>(&self, py: Python<'py>) -> Bound<'py, PyType>;

    /// Build the plugin from a Python object into a Bevy App.
    ///
    /// The `py_plugin` contains configuration (e.g., volume settings).
    /// This method creates the appropriate Bevy plugin and adds it to the app.
    fn build(&self, py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()>;

    /// Plugin name for error messages and debugging.
    fn name(&self) -> &'static str;
}

/// Internal registry storage
struct PluginRegistry {
    by_type_id: HashMap<TypeId, Arc<dyn PluginBridge>>,
    by_py_type: HashMap<*const PyTypeObject, Arc<dyn PluginBridge>>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for PluginRegistry {}
unsafe impl Sync for PluginRegistry {}

impl PluginRegistry {
    fn new() -> Self {
        Self {
            by_type_id: HashMap::new(),
            by_py_type: HashMap::new(),
        }
    }

    fn register(&mut self, bridge: impl PluginBridge) {
        self.register_arc(Arc::new(bridge));
    }

    fn register_arc(&mut self, bridge: Arc<dyn PluginBridge>) {
        let type_id = bridge.py_type_id();
        let py_ptr = bridge.py_type_ptr();

        self.by_type_id.insert(type_id, bridge.clone());
        self.by_py_type.insert(py_ptr, bridge);
    }

    fn get_by_py_type(&self, ptr: *const PyTypeObject) -> Option<Arc<dyn PluginBridge>> {
        self.by_py_type.get(&ptr).cloned()
    }
}

static PLUGIN_REGISTRY: OnceLock<RwLock<PluginRegistry>> = OnceLock::new();

fn get_registry() -> &'static RwLock<PluginRegistry> {
    PLUGIN_REGISTRY.get_or_init(|| RwLock::new(PluginRegistry::new()))
}

/// Global plugin registry module.
///
/// Feature crates register their plugin bridges at module init time.
/// The main crate looks up bridges when `app.add_plugins()` is called.
pub mod plugin_registry {
    use super::*;

    /// Register a plugin bridge with the global registry.
    ///
    /// Call this at module init time from feature crates.
    pub fn register_plugin_bridge(bridge: impl PluginBridge) {
        let registry = get_registry();
        let mut guard = registry.write().expect("Plugin registry poisoned");
        guard.register(bridge);
    }

    /// Register a pre-wrapped Arc plugin bridge (used by inventory auto-registration)
    pub fn register_plugin_bridge_arc(bridge: Arc<dyn PluginBridge>) {
        let registry = get_registry();
        let mut guard = registry.write().expect("Plugin registry poisoned");
        guard.register_arc(bridge);
    }

    /// Look up a plugin bridge by Python type pointer.
    ///
    /// Returns `None` if no bridge is registered for this type.
    pub fn get_by_py_type(ptr: *const PyTypeObject) -> Option<Arc<dyn PluginBridge>> {
        let registry = get_registry();
        let guard = registry.read().expect("Plugin registry poisoned");
        guard.get_by_py_type(ptr)
    }

    /// Check if a plugin bridge is registered for the given Python type.
    pub fn has_plugin(ptr: *const PyTypeObject) -> bool {
        let registry = get_registry();
        let guard = registry.read().expect("Plugin registry poisoned");
        guard.by_py_type.contains_key(&ptr)
    }
}
