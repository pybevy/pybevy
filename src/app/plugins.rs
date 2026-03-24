use std::collections::{HashMap, HashSet};

use bevy::{
    DefaultPlugins,
    app::{PluginGroupBuilder, ScheduleRunnerPlugin, TaskPoolPlugin},
    diagnostic::FrameCountPlugin,
    prelude::PluginGroup,
    time::TimePlugin,
};
use pybevy_core::PyPlugin;
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyType};

use super::{app::PyApp, plugin::PyPluginGroup, plugin_config::PluginConfigType};
use crate::assets::configured_asset_plugin;

fn default_window_plugin() -> bevy::window::WindowPlugin {
    bevy::window::WindowPlugin {
        primary_window: Some(bevy::window::Window {
            title: pybevy_window::DEFAULT_APP_TITLE.into(),
            name: Some("pybevy".into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[pyclass(name = "DefaultPlugins", extends = PyPluginGroup)]
pub struct PyDefaultPlugins;

#[pymethods]
impl PyDefaultPlugins {
    #[new]
    pub fn new() -> (Self, PyPluginGroup) {
        (PyDefaultPlugins, PyPluginGroup)
    }

    /// Convenience method: equivalent to .build().set(plugin)
    ///
    /// Usage:
    ///   DefaultPlugins.set(WindowPlugin(primary_window=Window(title="Game")))
    pub fn set(&self, py: Python, plugin: Bound<'_, PyAny>) -> PyResult<Py<PyPluginGroupBuilder>> {
        // Create builder and call .set() on it
        // Track that this builder came from DefaultPlugins for duplicate detection
        let source_type =
            py.get_type::<PyDefaultPlugins>().as_ptr() as *const pyo3::ffi::PyTypeObject;
        let (builder_struct, plugin_base) = PyPluginGroupBuilder::new();
        let builder_with_source = builder_struct.with_source_type(source_type);
        let builder = Py::new(py, (builder_with_source, plugin_base))?;
        builder.borrow(py).set(py, plugin)
    }

    /// Build the plugin group, returning a builder for configuration.
    ///
    /// This matches Bevy's `PluginGroup::build()` - returns a `PluginGroupBuilder`
    /// that can be used to enable/disable specific plugins.
    ///
    /// Usage:
    ///   DefaultPlugins.build().disable(AudioPlugin)
    pub fn build(&self, py: Python) -> PyResult<Py<PyPluginGroupBuilder>> {
        // Track that this builder came from DefaultPlugins for duplicate detection
        let source_type =
            py.get_type::<PyDefaultPlugins>().as_ptr() as *const pyo3::ffi::PyTypeObject;
        let (builder_struct, plugin_base) = PyPluginGroupBuilder::new();
        let builder_with_source = builder_struct.with_source_type(source_type);
        Py::new(py, (builder_with_source, plugin_base))
    }

    /// Internal method called by add_plugins() to apply the plugin group to the app.
    ///
    /// This applies the default configuration (all plugins enabled).
    pub fn _apply_to_app(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        app.borrow().with_bevy_app(|bevy_app| {
            bevy_app.add_plugins(
                DefaultPlugins
                    .set(configured_asset_plugin())
                    .set(default_window_plugin())
                    .disable::<bevy::log::LogPlugin>(),
            );
            // Register the SceneInstanceReady observer so MessageReader[SceneInstanceReady]
            // works without requiring an explicit ScenePlugin() addition.
            bevy_app.add_observer(pybevy_scene::scene_instance_ready_bridge);
            Ok(())
        })
    }
}

/// Position for plugin insertion
#[derive(Clone)]
enum PluginPosition {
    End,
    Before,
    After,
}

/// Thread-safe wrapper for Python type pointers used as plugin identifiers
///
/// Safety: These pointers are only used for comparison/hashing (opaque identifiers),
/// never dereferenced, so they are safe to send between threads.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PluginTypeId(*const pyo3::ffi::PyTypeObject);

impl PluginTypeId {
    /// Get the raw type pointer (for insertion into registry)
    pub(crate) fn as_ptr(self) -> *const pyo3::ffi::PyTypeObject {
        self.0
    }
}

unsafe impl Send for PluginTypeId {}
unsafe impl Sync for PluginTypeId {}

/// Builder for configuring plugin groups.
///
/// This is returned by `PluginGroup.build()` and allows configuring which
/// plugins to include/exclude before adding to the app.
#[pyclass(name = "PluginGroupBuilder", extends = PyPluginGroup)]
pub struct PyPluginGroupBuilder {
    configured_plugins: HashMap<PluginConfigType, Py<PyAny>>,
    disabled_plugins: HashSet<PluginConfigType>,
    added_plugins: Vec<(PluginPosition, Py<PyAny>)>,
    /// Tracks the source plugin group type (e.g., DefaultPlugins) to prevent duplicate additions
    /// When DefaultPlugins().build() is called, this stores the DefaultPlugins type pointer
    pub(crate) source_type: Option<PluginTypeId>,
}

impl Clone for PyPluginGroupBuilder {
    fn clone(&self) -> Self {
        Python::attach(|py| PyPluginGroupBuilder {
            configured_plugins: self
                .configured_plugins
                .iter()
                .map(|(k, v)| (*k, v.clone_ref(py)))
                .collect(),
            disabled_plugins: self.disabled_plugins.clone(),
            added_plugins: self
                .added_plugins
                .iter()
                .map(|(pos, plugin)| (pos.clone(), plugin.clone_ref(py)))
                .collect(),
            source_type: self.source_type,
        })
    }
}

impl PyPluginGroupBuilder {
    fn new() -> (Self, PyPluginGroup) {
        (
            PyPluginGroupBuilder {
                configured_plugins: HashMap::new(),
                disabled_plugins: HashSet::new(),
                added_plugins: Vec::new(),
                source_type: None,
            },
            PyPluginGroup,
        )
    }

    /// Set the source plugin group type (for duplicate detection)
    fn with_source_type(mut self, source_type: *const pyo3::ffi::PyTypeObject) -> Self {
        self.source_type = Some(PluginTypeId(source_type));
        self
    }
}

#[pymethods]
impl PyPluginGroupBuilder {
    /// Replace a plugin in the group
    pub fn set(&self, py: Python, plugin: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        if !plugin.is_instance_of::<PyPlugin>() {
            return Err(PyTypeError::new_err(
                "Argument to .set() must be a Plugin instance",
            ));
        }

        let plugin_type = plugin.get_type();
        let config_type = PluginConfigType::from_py_type(py, &plugin_type)?;

        // Clone to new instance (immutable builder pattern)
        let mut new_builder = self.clone();
        new_builder
            .configured_plugins
            .insert(config_type, plugin.unbind());

        Py::new(py, (new_builder, PyPluginGroup))
    }

    /// Disable a specific plugin
    ///
    /// Usage:
    ///   builder.disable(AudioPlugin)
    pub fn disable(&self, py: Python, plugin_type: Bound<'_, PyType>) -> PyResult<Py<Self>> {
        let config_type = PluginConfigType::from_py_type(py, &plugin_type)?;

        let mut new_builder = self.clone();
        new_builder.disabled_plugins.insert(config_type);

        Py::new(py, (new_builder, PyPluginGroup))
    }

    /// Add a plugin to the end
    pub fn add(&self, py: Python, plugin: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        if !plugin.is_instance_of::<PyPlugin>() {
            return Err(PyTypeError::new_err(
                "Argument to .add() must be a Plugin instance",
            ));
        }

        let mut new_builder = self.clone();
        new_builder
            .added_plugins
            .push((PluginPosition::End, plugin.unbind()));

        Py::new(py, (new_builder, PyPluginGroup))
    }

    /// Add plugin before target
    pub fn add_before(
        &self,
        py: Python,
        target: Bound<'_, PyType>,
        plugin: Bound<'_, PyAny>,
    ) -> PyResult<Py<Self>> {
        if !plugin.is_instance_of::<PyPlugin>() {
            return Err(PyTypeError::new_err(
                "Argument to .add_before() must be a Plugin instance",
            ));
        }

        // Validate target is a known plugin type (future: use for ordering)
        let _target_type = PluginConfigType::from_py_type(py, &target)?;

        let mut new_builder = self.clone();
        new_builder
            .added_plugins
            .push((PluginPosition::Before, plugin.unbind()));

        Py::new(py, (new_builder, PyPluginGroup))
    }

    /// Add plugin after target
    pub fn add_after(
        &self,
        py: Python,
        target: Bound<'_, PyType>,
        plugin: Bound<'_, PyAny>,
    ) -> PyResult<Py<Self>> {
        if !plugin.is_instance_of::<PyPlugin>() {
            return Err(PyTypeError::new_err(
                "Argument to .add_after() must be a Plugin instance",
            ));
        }

        // Validate target is a known plugin type (future: use for ordering)
        let _target_type = PluginConfigType::from_py_type(py, &target)?;

        let mut new_builder = self.clone();
        new_builder
            .added_plugins
            .push((PluginPosition::After, plugin.unbind()));

        Py::new(py, (new_builder, PyPluginGroup))
    }

    /// Re-enable a previously disabled plugin
    pub fn enable(&self, py: Python, plugin_type: Bound<'_, PyType>) -> PyResult<Py<Self>> {
        let config_type = PluginConfigType::from_py_type(py, &plugin_type)?;

        let mut new_builder = self.clone();
        new_builder.disabled_plugins.remove(&config_type);

        Py::new(py, (new_builder, PyPluginGroup))
    }

    /// Plugin trait implementation - called by PyApp.add_plugins()
    pub fn build(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        app.borrow().with_bevy_app(|bevy_app| {
            // Insert pre-plugin resources (e.g., WinitSettings must exist before WinitPlugin runs)
            self.insert_pre_plugin_resources(app.py(), bevy_app)?;
            let builder = self.apply_to_bevy(app.py())?;
            bevy_app.add_plugins(builder);
            // Register the SceneInstanceReady observer so MessageReader[SceneInstanceReady]
            // works without requiring an explicit ScenePlugin() addition.
            bevy_app.add_observer(pybevy_scene::scene_instance_ready_bridge);
            Ok(())
        })
    }
}

impl PyPluginGroupBuilder {
    /// Insert resources that must exist before plugins run.
    ///
    /// WinitSettings must be inserted before WinitPlugin::build() because the plugin
    /// uses `init_resource::<WinitSettings>()` which is a no-op if already present.
    fn insert_pre_plugin_resources(
        &self,
        py: Python,
        bevy_app: &mut bevy::app::App,
    ) -> PyResult<()> {
        if let Some(plugin_obj) = self.configured_plugins.get(&PluginConfigType::Winit) {
            let winit_plugin: PyRef<pybevy_window::PyWinitPlugin> = plugin_obj.extract(py)?;
            if let Some(ref settings) = winit_plugin.settings {
                bevy_app.insert_resource(bevy::winit::WinitSettings::from(settings.clone()));
            }
        }
        Ok(())
    }

    /// Internal: Apply configuration to 's PluginGroupBuilder
    fn apply_to_bevy(&self, py: Python) -> PyResult<PluginGroupBuilder> {
        let mut builder = DefaultPlugins
            .set(configured_asset_plugin())
            .set(default_window_plugin())
            .disable::<bevy::log::LogPlugin>();

        // Apply configured plugins
        for (config_type, plugin_obj) in &self.configured_plugins {
            if !self.disabled_plugins.contains(config_type) {
                builder = apply_plugin_configuration(builder, config_type, plugin_obj, py)?;
            }
        }

        // Apply disabled plugins
        for disabled_type in &self.disabled_plugins {
            builder = disable_plugin(builder, disabled_type)?;
        }

        // Apply added plugins (future enhancement)
        // for (position, plugin) in &self.added_plugins {
        //     builder = add_plugin(builder, position, plugin, py)?;
        // }

        Ok(builder)
    }
}

/// Apply a plugin configuration to the builder
fn apply_plugin_configuration(
    builder: PluginGroupBuilder,
    config_type: &PluginConfigType,
    plugin_obj: &Py<PyAny>,
    py: Python,
) -> PyResult<PluginGroupBuilder> {
    match config_type {
        PluginConfigType::Window => {
            let window_plugin: PyRef<pybevy_window::PyWindowPlugin> = plugin_obj.extract(py)?;
            let bevy_plugin = bevy::window::WindowPlugin::try_from(&*window_plugin)?;
            Ok(builder.set(bevy_plugin))
        }
        PluginConfigType::Winit => {
            // WinitPlugin itself has no config fields - WinitSettings is a resource
            // handled by insert_pre_plugin_resources() in build()
            Ok(builder.set(bevy::winit::WinitPlugin::default()))
        }
        PluginConfigType::Render => {
            let render_plugin: PyRef<pybevy_render::PyRenderPlugin> = plugin_obj.extract(py)?;
            let mut wgpu_settings = bevy::render::settings::WgpuSettings::default();
            if let Some(ref pp) = render_plugin.power_preference {
                wgpu_settings.power_preference = (*pp).into();
            }
            let mut bevy_plugin = bevy::render::RenderPlugin {
                render_creation: bevy::render::settings::RenderCreation::Automatic(wgpu_settings),
                ..Default::default()
            };
            if let Some(sync) = render_plugin.synchronous_pipeline_compilation {
                bevy_plugin.synchronous_pipeline_compilation = sync;
            }
            Ok(builder.set(bevy_plugin))
        }
        _ => Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Plugin configuration not yet implemented: {:?}",
            config_type
        ))),
    }
}

/// Disable a plugin from the builder
fn disable_plugin(
    builder: PluginGroupBuilder,
    config_type: &PluginConfigType,
) -> PyResult<PluginGroupBuilder> {
    match config_type {
        PluginConfigType::Window => Ok(builder.disable::<bevy::window::WindowPlugin>()),
        PluginConfigType::Audio => Ok(builder.disable::<bevy::audio::AudioPlugin>()),
        PluginConfigType::Winit => Ok(builder.disable::<bevy::winit::WinitPlugin>()),
        PluginConfigType::Render => Ok(builder.disable::<bevy::render::RenderPlugin>()),
        PluginConfigType::Image => Ok(builder.disable::<bevy::image::ImagePlugin>()),
        PluginConfigType::TaskPool => Ok(builder.disable::<TaskPoolPlugin>()),
    }
}

#[pyclass(name = "MinimalPlugins", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyMinimalPlugins;

#[pymethods]
impl PyMinimalPlugins {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyMinimalPlugins, PyPlugin)
    }

    pub fn build(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        app.borrow().with_bevy_app(|bevy_app| {
            bevy_app.add_plugins((
                TaskPoolPlugin::default(),
                FrameCountPlugin::default(),
                TimePlugin::default(),
                ScheduleRunnerPlugin::default(),
            ));
            Ok(())
        })
    }
}
