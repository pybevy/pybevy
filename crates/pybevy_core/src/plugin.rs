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

use bevy::app::{App, Plugin};
use pybevy_storage::DefaultPluginKind;
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
#[pyclass(name = "Plugin", subclass, frozen, skip_from_py_object)]
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

/// Add a unique native plugin unless its type is already present in the app.
///
/// Bevy's ordinary `add_plugins` path panics for duplicate unique plugins. Python
/// wrappers use this helper where a plugin can also arrive through a native
/// plugin group whose members are not visible to Python-side deduplication.
pub fn add_plugin_if_missing<P: Plugin>(app: &mut App, plugin: P) -> bool {
    if app.is_plugin_added::<P>() {
        return false;
    }
    app.add_plugins(plugin);
    true
}

/// Bridge trait for building plugins into Bevy apps.
///
/// Feature crates implement this trait for their plugin types.
/// The main crate's `PyApp.add_plugins` looks up bridges by Python type
/// and calls `build` with `&mut bevy::app::App`.
///
/// Trait for Python plugin wrappers to implement their build logic.
///
/// Implement this on your `#[pyclass]` struct, then use `#[pyplugin(BevyPlugin)]`
/// to generate the bridge and inventory registration automatically.
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

    /// Return whether the wrapped Bevy plugin is already installed in this app.
    fn is_added(&self, app: &App) -> bool;

    /// Plugin name for error messages and debugging.
    fn name(&self) -> &'static str;

    /// Identify wrappers for configurable members of Bevy's `DefaultPlugins` group.
    ///
    /// Most native plugins are not configurable group members and return `None`.
    fn default_plugin_kind(&self) -> Option<DefaultPluginKind> {
        None
    }
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

    fn register_arc(
        &mut self,
        type_id: TypeId,
        py_ptr: *const PyTypeObject,
        bridge: Arc<dyn PluginBridge>,
    ) {
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

fn register_plugin_bridge_arc_in(registry: &RwLock<PluginRegistry>, bridge: Arc<dyn PluginBridge>) {
    // Resolving the Python type may acquire the GIL. Do it before locking the
    // registry so a GIL-owning lookup cannot deadlock against registration.
    let type_id = bridge.py_type_id();
    let py_ptr = bridge.py_type_ptr();

    let mut guard = registry.write().expect("Plugin registry poisoned");
    guard.register_arc(type_id, py_ptr, bridge);
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
        register_plugin_bridge_arc(Arc::new(bridge));
    }

    /// Register a pre-wrapped Arc plugin bridge (used by inventory auto-registration)
    pub fn register_plugin_bridge_arc(bridge: Arc<dyn PluginBridge>) {
        register_plugin_bridge_arc_in(get_registry(), bridge);
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

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;

    #[derive(Default)]
    struct ProbePlugin;

    impl Plugin for ProbePlugin {
        fn build(&self, app: &mut App) {
            app.insert_resource(ProbeResource);
        }
    }

    #[derive(bevy::prelude::Resource)]
    struct ProbeResource;

    #[test]
    fn add_plugin_if_missing_skips_an_installed_plugin() {
        let mut app = App::new();

        assert!(add_plugin_if_missing(&mut app, ProbePlugin));
        assert!(app.world().contains_resource::<ProbeResource>());
        assert!(!add_plugin_if_missing(&mut app, ProbePlugin));
    }

    struct RegistryLockProbeBridge {
        registry: Arc<RwLock<PluginRegistry>>,
        checked: Arc<AtomicBool>,
    }

    impl PluginBridge for RegistryLockProbeBridge {
        fn py_type_id(&self) -> TypeId {
            TypeId::of::<Self>()
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            let _guard = self
                .registry
                .try_read()
                .expect("plugin registry must be unlocked while resolving a Python type");
            self.checked.store(true, Ordering::Release);
            std::ptr::null()
        }

        fn py_type<'py>(&self, py: Python<'py>) -> Bound<'py, PyType> {
            py.get_type::<PyPlugin>()
        }

        fn build(&self, _py_plugin: &Bound<'_, PyAny>, _app: &mut App) -> PyResult<()> {
            Ok(())
        }

        fn is_added(&self, _app: &App) -> bool {
            false
        }

        fn name(&self) -> &'static str {
            "RegistryLockProbe"
        }
    }

    #[test]
    fn plugin_type_is_resolved_before_registry_write_lock() {
        let registry = Arc::new(RwLock::new(PluginRegistry::new()));
        let checked = Arc::new(AtomicBool::new(false));
        let bridge = Arc::new(RegistryLockProbeBridge {
            registry: registry.clone(),
            checked: checked.clone(),
        });

        register_plugin_bridge_arc_in(&registry, bridge);

        assert!(checked.load(Ordering::Acquire));
    }
}
