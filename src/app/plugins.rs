use std::collections::{HashMap, HashSet};

use bevy::{
    DefaultPlugins,
    app::{App, PluginGroupBuilder, ScheduleRunnerPlugin, TaskPoolPlugin},
    audio::AudioPlugin,
    diagnostic::FrameCountPlugin,
    image::ImagePlugin,
    log::LogPlugin,
    prelude::{Plugin, PluginGroup},
    render::{
        RenderPlugin,
        settings::{RenderCreation, WgpuSettings},
    },
    time::TimePlugin,
    window::{Window, WindowPlugin},
    winit::{WinitPlugin, WinitSettings},
};
use pybevy_core::{
    PluginGroupAddition, PluginGroupPlacement, PyPlugin, plugin::add_plugin_if_missing,
};
use pybevy_render::{plugin::PyRenderPlugin, wgpu_error_handler::WgpuErrorHandlerPlugin};
use pybevy_window::{prelude::PyWindowPlugin, window::DEFAULT_APP_TITLE};
use pybevy_winit::plugin::PyWinitPlugin;
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError},
    ffi::PyTypeObject,
    prelude::*,
    types::PyType,
};

use super::{
    app::PyApp,
    plugin::PyPluginGroup,
    plugin_config::{PluginConfigType, plugin_config_type, try_plugin_config_type},
};
use crate::assets::configured_asset_plugin;

fn default_window_plugin() -> WindowPlugin {
    WindowPlugin {
        primary_window: Some(Window {
            title: DEFAULT_APP_TITLE.into(),
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
    pub fn new() -> PyClassInitializer<Self> {
        (PyDefaultPlugins, PyPluginGroup).into()
    }

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
                    .disable::<LogPlugin>(),
            );
            bevy_app.add_plugins(WgpuErrorHandlerPlugin);

            // Register the WorldInstanceReady observer so MessageReader[WorldInstanceReady]
            // works without requiring an explicit WorldSerializationPlugin() addition.
            bevy_app.add_observer(pybevy_world_serialization::world_instance_ready_bridge);
            Ok(())
        })
    }
}

struct AddedPlugin {
    addition: PluginGroupAddition<PluginConfigType, Py<PyAny>>,
    config_type: Option<PluginConfigType>,
}

/// Thread-safe wrapper for Python type pointers used as plugin identifiers
///
/// Safety: These pointers are only used for comparison/hashing (opaque identifiers),
/// never dereferenced, so they are safe to send between threads.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PluginTypeId(*const PyTypeObject);

impl PluginTypeId {
    pub(crate) fn as_ptr(self) -> *const PyTypeObject {
        self.0
    }
}

unsafe impl Send for PluginTypeId {}
unsafe impl Sync for PluginTypeId {}

#[pyclass(name = "PluginGroupBuilder", extends = PyPluginGroup, skip_from_py_object)]
pub struct PyPluginGroupBuilder {
    configured_plugins: HashMap<PluginConfigType, Py<PyAny>>,
    disabled_plugins: HashSet<PluginConfigType>,
    added_plugins: Vec<AddedPlugin>,
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
                .map(|added| AddedPlugin {
                    addition: PluginGroupAddition::new(
                        added.addition.placement,
                        added.addition.plugin.clone_ref(py),
                    ),
                    config_type: added.config_type,
                })
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
    fn with_source_type(mut self, source_type: *const PyTypeObject) -> Self {
        self.source_type = Some(PluginTypeId(source_type));
        self
    }
}

#[pymethods]
impl PyPluginGroupBuilder {
    pub fn set(&self, py: Python, plugin: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        if !plugin.is_instance_of::<PyPlugin>() {
            return Err(PyTypeError::new_err(
                "Argument to .set() must be a Plugin instance",
            ));
        }

        let plugin_type = plugin.get_type();
        let config_type = plugin_config_type(&plugin_type)?;

        // Clone to new instance (immutable builder pattern)
        let mut new_builder = self.clone();
        new_builder
            .configured_plugins
            .insert(config_type, plugin.unbind());

        Py::new(py, (new_builder, PyPluginGroup))
    }

    pub fn disable(&self, py: Python, plugin_type: Bound<'_, PyType>) -> PyResult<Py<Self>> {
        let config_type = plugin_config_type(&plugin_type)?;

        let mut new_builder = self.clone();
        new_builder.disabled_plugins.insert(config_type);

        Py::new(py, (new_builder, PyPluginGroup))
    }

    pub fn add(&self, py: Python, plugin: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        if !plugin.is_instance_of::<PyPlugin>() {
            return Err(PyTypeError::new_err(
                "Argument to .add() must be a Plugin instance",
            ));
        }

        let config_type = try_plugin_config_type(&plugin.get_type());
        let mut new_builder = self.clone();
        new_builder.added_plugins.push(AddedPlugin {
            addition: PluginGroupAddition::new(PluginGroupPlacement::End, plugin.unbind()),
            config_type,
        });

        Py::new(py, (new_builder, PyPluginGroup))
    }

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

        let target_type = plugin_config_type(&target)?;
        let config_type = require_native_group_plugin(&plugin, "add_before")?;

        let mut new_builder = self.clone();
        new_builder.added_plugins.push(AddedPlugin {
            addition: PluginGroupAddition::new(
                PluginGroupPlacement::Before(target_type),
                plugin.unbind(),
            ),
            config_type: Some(config_type),
        });

        Py::new(py, (new_builder, PyPluginGroup))
    }

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

        let target_type = plugin_config_type(&target)?;
        let config_type = require_native_group_plugin(&plugin, "add_after")?;

        let mut new_builder = self.clone();
        new_builder.added_plugins.push(AddedPlugin {
            addition: PluginGroupAddition::new(
                PluginGroupPlacement::After(target_type),
                plugin.unbind(),
            ),
            config_type: Some(config_type),
        });

        Py::new(py, (new_builder, PyPluginGroup))
    }

    pub fn enable(&self, py: Python, plugin_type: Bound<'_, PyType>) -> PyResult<Py<Self>> {
        let config_type = plugin_config_type(&plugin_type)?;

        let mut new_builder = self.clone();
        new_builder.disabled_plugins.remove(&config_type);

        Py::new(py, (new_builder, PyPluginGroup))
    }

    pub fn build(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        app.borrow().with_bevy_app(|bevy_app| {
            // Insert pre-plugin resources (e.g., WinitSettings must exist before WinitPlugin runs)
            self.insert_pre_plugin_resources(app.py(), bevy_app)?;
            let builder = self.apply_to_bevy(app.py())?;
            bevy_app.add_plugins(builder);
            Ok(())
        })?;

        for added in &self.added_plugins {
            if added.config_type.is_none() {
                app.call_method1("add_plugins", (added.addition.plugin.bind(app.py()),))?;
            }
        }

        app.borrow().with_bevy_app(|bevy_app| {
            bevy_app.add_plugins(WgpuErrorHandlerPlugin);
            // Register the WorldInstanceReady observer so MessageReader[WorldInstanceReady]
            // works without requiring an explicit WorldSerializationPlugin() addition.
            bevy_app.add_observer(pybevy_world_serialization::world_instance_ready_bridge);
            Ok(())
        })
    }
}

impl PyPluginGroupBuilder {
    /// Insert resources that must exist before plugins run.
    ///
    /// WinitSettings must be inserted before WinitPlugin::build() because the plugin
    /// uses `init_resource::<WinitSettings>()` which is a no-op if already present.
    fn insert_pre_plugin_resources(&self, py: Python, bevy_app: &mut App) -> PyResult<()> {
        if let Some(plugin_obj) = self.configured_plugins.get(&PluginConfigType::Winit) {
            insert_winit_settings(plugin_obj.bind(py), bevy_app)?;
        }
        for added in &self.added_plugins {
            if added.config_type == Some(PluginConfigType::Winit) {
                insert_winit_settings(added.addition.plugin.bind(py), bevy_app)?;
            }
        }
        Ok(())
    }

    fn apply_to_bevy(&self, py: Python) -> PyResult<PluginGroupBuilder> {
        let mut builder = DefaultPlugins
            .set(configured_asset_plugin())
            .set(default_window_plugin())
            .disable::<LogPlugin>();

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

        for added in &self.added_plugins {
            if let Some(config_type) = added.config_type {
                builder = apply_added_plugin(builder, added, config_type, py)?;
            }
        }

        Ok(builder)
    }
}

fn require_native_group_plugin(
    plugin: &Bound<'_, PyAny>,
    method: &str,
) -> PyResult<PluginConfigType> {
    try_plugin_config_type(&plugin.get_type()).ok_or_else(|| {
        let type_name = plugin
            .get_type()
            .name()
            .map(|name| name.to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        if pybevy_core::plugin::plugin_registry::has_plugin(plugin.get_type().as_type_ptr()) {
            PyTypeError::new_err(format!(
                "PluginGroupBuilder.{method}() cannot order native plugin '{type_name}' because it has no DefaultPlugins ordering adapter; use add()"
            ))
        } else {
            PyTypeError::new_err(format!(
                "PluginGroupBuilder.{method}() cannot order Python-defined plugin '{type_name}'; use add() or a native PyBevy plugin"
            ))
        }
    })
}

fn insert_winit_settings(plugin: &Bound<'_, PyAny>, bevy_app: &mut App) -> PyResult<()> {
    let winit_plugin: PyRef<PyWinitPlugin> = plugin.extract()?;
    if let Some(ref settings) = winit_plugin.settings {
        bevy_app.insert_resource(WinitSettings::from(settings.clone()));
    }
    Ok(())
}

fn configured_render_plugin(plugin_obj: &Py<PyAny>, py: Python<'_>) -> PyResult<RenderPlugin> {
    let render_plugin: PyRef<PyRenderPlugin> = plugin_obj.extract(py)?;
    let mut wgpu_settings = WgpuSettings::default();
    if let Some(ref pp) = render_plugin.power_preference {
        wgpu_settings.power_preference = (*pp).into();
    }
    let mut bevy_plugin = RenderPlugin {
        render_creation: RenderCreation::Automatic(Box::new(wgpu_settings)),
        ..Default::default()
    };
    if let Some(sync) = render_plugin.synchronous_pipeline_compilation {
        bevy_plugin.synchronous_pipeline_compilation = sync;
    }
    Ok(bevy_plugin)
}

fn apply_plugin_configuration(
    builder: PluginGroupBuilder,
    config_type: &PluginConfigType,
    plugin_obj: &Py<PyAny>,
    py: Python,
) -> PyResult<PluginGroupBuilder> {
    match config_type {
        PluginConfigType::Audio => Ok(builder.set(AudioPlugin::default())),
        PluginConfigType::Image => Ok(builder.set(ImagePlugin::default())),
        PluginConfigType::TaskPool => Ok(builder.set(TaskPoolPlugin::default())),
        PluginConfigType::Window => {
            let window_plugin: PyRef<PyWindowPlugin> = plugin_obj.extract(py)?;
            let bevy_plugin = WindowPlugin::try_from(&*window_plugin)?;
            Ok(builder.set(bevy_plugin))
        }

        PluginConfigType::Winit => {
            // WinitPlugin itself has no config fields - WinitSettings is a resource
            // handled by insert_pre_plugin_resources() in build()
            Ok(builder.set(WinitPlugin::default()))
        }

        PluginConfigType::Render => Ok(builder.set(configured_render_plugin(plugin_obj, py)?)),
    }
}

fn apply_added_plugin(
    builder: PluginGroupBuilder,
    added: &AddedPlugin,
    config_type: PluginConfigType,
    py: Python<'_>,
) -> PyResult<PluginGroupBuilder> {
    match config_type {
        PluginConfigType::Audio => {
            place_plugin(builder, added.addition.placement, AudioPlugin::default())
        }
        PluginConfigType::Image => {
            place_plugin(builder, added.addition.placement, ImagePlugin::default())
        }
        PluginConfigType::TaskPool => {
            place_plugin(builder, added.addition.placement, TaskPoolPlugin::default())
        }
        PluginConfigType::Window => {
            let plugin: PyRef<PyWindowPlugin> = added.addition.plugin.extract(py)?;
            place_plugin(
                builder,
                added.addition.placement,
                WindowPlugin::try_from(&*plugin)?,
            )
        }
        PluginConfigType::Winit => {
            place_plugin(builder, added.addition.placement, WinitPlugin::default())
        }
        PluginConfigType::Render => place_plugin(
            builder,
            added.addition.placement,
            configured_render_plugin(&added.addition.plugin, py)?,
        ),
    }
}

fn place_plugin<P: Plugin>(
    builder: PluginGroupBuilder,
    placement: PluginGroupPlacement<PluginConfigType>,
    plugin: P,
) -> PyResult<PluginGroupBuilder> {
    match placement {
        PluginGroupPlacement::End => Ok(builder.add(plugin)),
        PluginGroupPlacement::Before(target) => place_before(builder, target, plugin),
        PluginGroupPlacement::After(target) => place_after(builder, target, plugin),
    }
}

fn place_before<P: Plugin>(
    builder: PluginGroupBuilder,
    target: PluginConfigType,
    plugin: P,
) -> PyResult<PluginGroupBuilder> {
    let result = match target {
        PluginConfigType::Audio => builder.try_add_before_overwrite::<AudioPlugin, _>(plugin),
        PluginConfigType::Image => builder.try_add_before_overwrite::<ImagePlugin, _>(plugin),
        PluginConfigType::Render => builder.try_add_before_overwrite::<RenderPlugin, _>(plugin),
        PluginConfigType::TaskPool => builder.try_add_before_overwrite::<TaskPoolPlugin, _>(plugin),
        PluginConfigType::Window => builder.try_add_before_overwrite::<WindowPlugin, _>(plugin),
        PluginConfigType::Winit => builder.try_add_before_overwrite::<WinitPlugin, _>(plugin),
    };
    result.map_err(|_| {
        PyRuntimeError::new_err(format!(
            "target plugin '{}' is not present in DefaultPlugins",
            target.public_name()
        ))
    })
}

fn place_after<P: Plugin>(
    builder: PluginGroupBuilder,
    target: PluginConfigType,
    plugin: P,
) -> PyResult<PluginGroupBuilder> {
    let result = match target {
        PluginConfigType::Audio => builder.try_add_after_overwrite::<AudioPlugin, _>(plugin),
        PluginConfigType::Image => builder.try_add_after_overwrite::<ImagePlugin, _>(plugin),
        PluginConfigType::Render => builder.try_add_after_overwrite::<RenderPlugin, _>(plugin),
        PluginConfigType::TaskPool => builder.try_add_after_overwrite::<TaskPoolPlugin, _>(plugin),
        PluginConfigType::Window => builder.try_add_after_overwrite::<WindowPlugin, _>(plugin),
        PluginConfigType::Winit => builder.try_add_after_overwrite::<WinitPlugin, _>(plugin),
    };
    result.map_err(|_| {
        PyRuntimeError::new_err(format!(
            "target plugin '{}' is not present in DefaultPlugins",
            target.public_name()
        ))
    })
}

fn disable_plugin(
    builder: PluginGroupBuilder,
    config_type: &PluginConfigType,
) -> PyResult<PluginGroupBuilder> {
    match config_type {
        PluginConfigType::Audio => Ok(builder.disable::<AudioPlugin>()),
        PluginConfigType::Image => Ok(builder.disable::<ImagePlugin>()),
        PluginConfigType::Render => Ok(builder.disable::<RenderPlugin>()),
        PluginConfigType::TaskPool => Ok(builder.disable::<TaskPoolPlugin>()),
        PluginConfigType::Window => Ok(builder.disable::<WindowPlugin>()),
        PluginConfigType::Winit => Ok(builder.disable::<WinitPlugin>()),
    }
}

#[pyclass(name = "MinimalPlugins", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyMinimalPlugins;

#[pymethods]
impl PyMinimalPlugins {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyMinimalPlugins, PyPlugin).into()
    }

    pub fn build(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        app.borrow().with_bevy_app(|bevy_app| {
            add_plugin_if_missing(bevy_app, TaskPoolPlugin::default());
            add_plugin_if_missing(bevy_app, FrameCountPlugin);
            add_plugin_if_missing(bevy_app, TimePlugin);
            add_plugin_if_missing(bevy_app, ScheduleRunnerPlugin::default());
            Ok(())
        })
    }
}
