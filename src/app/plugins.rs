use std::{
    any::TypeId,
    mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{Arc, Mutex},
};

#[cfg(any(all(unix, not(target_os = "horizon")), windows))]
use bevy::{
    DefaultPlugins,
    app::{App, PluginGroupBuilder, ScheduleRunnerPlugin, TaskPoolPlugin},
    audio::AudioPlugin,
    diagnostic::FrameCountPlugin,
    image::ImagePlugin,
    log::LogPlugin,
    prelude::PluginGroup,
    render::RenderPlugin,
    time::TimePlugin,
    window::{Window, WindowPlugin},
    winit::WinitPlugin,
    world_serialization::WorldSerializationPlugin,
};
use pybevy_core::{
    DefaultPluginKind, NativePluginSlot, PluginGroupCallback, PluginGroupMembers,
    PluginGroupPlacement, PyPlugin, default_plugin_slots,
    plugin::plugin_registry,
    public_error::{
        PLUGIN_CLASS_REQUIRED, PLUGIN_GROUP_BUILD_RESULT, PLUGIN_GROUP_REQUIRED,
        PLUGIN_GROUP_START_TYPE, PLUGIN_GROUP_TARGET_MISSING, PLUGIN_INSTANCE_REQUIRED,
        missing_group_plugin, plugin_build_error,
    },
};
use pybevy_render::wgpu_error_handler::WgpuErrorHandlerPlugin;
use pybevy_window::window::DEFAULT_APP_TITLE;
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyRuntimeError, PyTypeError},
    prelude::*,
    types::PyType,
};

use super::{
    app::PyApp,
    plugin::{PyPluginGroup, build_plugin_group},
};
use crate::assets::configured_asset_plugin;

fn finish_plugin_group(name: &str, finish: impl FnOnce() -> PyResult<()>) -> PyResult<()> {
    match catch_unwind(AssertUnwindSafe(finish)) {
        Ok(result) => result,
        Err(payload) => {
            let detail = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| {
                    payload
                        .downcast_ref::<&str>()
                        .map(|message| message.to_string())
                })
                .unwrap_or_else(|| "native plugin build panicked".to_string());
            Err(PyRuntimeError::new_err(plugin_build_error(name, detail)))
        }
    }
}

fn default_native_builder() -> PluginGroupBuilder {
    DefaultPlugins
        .set(configured_asset_plugin())
        .set(WindowPlugin {
            primary_window: Some(Window {
                title: DEFAULT_APP_TITLE.into(),
                name: Some("pybevy".into()),
                ..Default::default()
            }),
            ..Default::default()
        })
        .disable::<LogPlugin>()
}

fn default_slots() -> Vec<NativePluginSlot> {
    let native = default_native_builder();
    default_plugin_slots!()
        .into_iter()
        .filter(|slot| slot.contains(&native))
        .collect()
}

fn default_kind_slot(kind: DefaultPluginKind) -> NativePluginSlot {
    match kind {
        DefaultPluginKind::Audio => NativePluginSlot::of::<AudioPlugin>(),
        DefaultPluginKind::Image => NativePluginSlot::of::<ImagePlugin>(),
        DefaultPluginKind::Render => NativePluginSlot::of::<RenderPlugin>(),
        DefaultPluginKind::TaskPool => NativePluginSlot::of::<TaskPoolPlugin>(),
        DefaultPluginKind::Window => NativePluginSlot::of::<WindowPlugin>(),
        DefaultPluginKind::Winit => NativePluginSlot::of::<WinitPlugin>(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MemberKey {
    Native(TypeId),
    Python(usize),
    DefaultTail,
}

enum MemberValue {
    Native(NativePluginSlot),
    DefaultNative(NativePluginSlot),
    Python(Py<PyAny>),
    DefaultTail,
}

impl MemberValue {
    fn clone_ref(&self, py: Python<'_>) -> Self {
        match self {
            Self::Native(slot) => Self::Native(*slot),
            Self::DefaultNative(slot) => Self::DefaultNative(*slot),
            Self::Python(value) => Self::Python(value.clone_ref(py)),
            Self::DefaultTail => Self::DefaultTail,
        }
    }
}

fn member_key(plugin_type: &Bound<'_, PyType>) -> PyResult<MemberKey> {
    if !plugin_type.is_subclass_of::<PyPlugin>()? {
        return Err(PyTypeError::new_err(PLUGIN_CLASS_REQUIRED));
    }
    Ok(
        match plugin_registry::get_by_py_type(plugin_type.as_type_ptr()) {
            Some(bridge) => MemberKey::Native(
                bridge
                    .default_plugin_kind()
                    .map(default_kind_slot)
                    .map(|slot| slot.type_id)
                    .unwrap_or_else(|| bridge.native_type_id()),
            ),
            None => MemberKey::Python(plugin_type.as_type_ptr() as usize),
        },
    )
}

fn missing_member(plugin_type: &Bound<'_, PyType>) -> PyResult<PyErr> {
    Ok(PyRuntimeError::new_err(missing_group_plugin(
        plugin_type.name()?,
    )))
}

#[pyclass(name = "PluginGroupBuilder", module = "pybevy.app", extends = PyPluginGroup, skip_from_py_object)]
pub struct PyPluginGroupBuilder {
    name: String,
    members: PluginGroupMembers<MemberKey, MemberValue>,
    default_seed: bool,
}

impl Clone for PyPluginGroupBuilder {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            name: self.name.clone(),
            members: self.members.map_values(|value| value.clone_ref(py)),
            default_seed: self.default_seed,
        })
    }
}

impl PyPluginGroupBuilder {
    fn empty(name: String) -> Self {
        Self {
            name,
            members: PluginGroupMembers::default(),
            default_seed: false,
        }
    }
    fn into_python(self, py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (self, PyPluginGroup))
    }

    fn add_at(
        &self,
        py: Python<'_>,
        plugin: Bound<'_, PyAny>,
        placement: PluginGroupPlacement<MemberKey>,
    ) -> PyResult<Py<Self>> {
        if !plugin.is_instance_of::<PyPlugin>() {
            return Err(PyTypeError::new_err(PLUGIN_INSTANCE_REQUIRED));
        }
        let key = member_key(&plugin.get_type())?;
        let mut builder = self.clone();
        if builder
            .members
            .add(key, MemberValue::Python(plugin.unbind()), placement)
            .is_err()
        {
            return Err(PyRuntimeError::new_err(PLUGIN_GROUP_TARGET_MISSING));
        }
        builder.into_python(py)
    }

    fn toggle(
        &self,
        py: Python<'_>,
        plugin_type: &Bound<'_, PyType>,
        enabled: bool,
    ) -> PyResult<Py<Self>> {
        let mut builder = self.clone();
        if !builder
            .members
            .set_enabled(&member_key(plugin_type)?, enabled)
        {
            return Err(missing_member(plugin_type)?);
        }
        builder.into_python(py)
    }
}

#[pymethods]
impl PyPluginGroupBuilder {
    #[staticmethod]
    pub fn start(py: Python<'_>, group_type: Bound<'_, PyType>) -> PyResult<Py<Self>> {
        if !group_type.is_subclass_of::<PyPluginGroup>()? {
            return Err(PyTypeError::new_err(PLUGIN_GROUP_START_TYPE));
        }
        Self::empty(group_type.call_method0("name")?.extract::<String>()?).into_python(py)
    }

    pub fn contains(&self, plugin_type: Bound<'_, PyType>) -> PyResult<bool> {
        Ok(self.members.contains(&member_key(&plugin_type)?))
    }
    pub fn enabled(&self, plugin_type: Bound<'_, PyType>) -> PyResult<bool> {
        Ok(self.members.enabled(&member_key(&plugin_type)?))
    }
    pub fn set(&self, py: Python<'_>, plugin: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        if !plugin.is_instance_of::<PyPlugin>() {
            return Err(PyTypeError::new_err(PLUGIN_INSTANCE_REQUIRED));
        }
        let plugin_type = plugin.get_type();
        let key = member_key(&plugin_type)?;
        let mut builder = self.clone();
        if builder
            .members
            .set(&key, MemberValue::Python(plugin.clone().unbind()))
            .is_err()
        {
            return Err(missing_member(&plugin_type)?);
        }
        builder.into_python(py)
    }
    pub fn disable(&self, py: Python<'_>, plugin_type: Bound<'_, PyType>) -> PyResult<Py<Self>> {
        self.toggle(py, &plugin_type, false)
    }
    pub fn enable(&self, py: Python<'_>, plugin_type: Bound<'_, PyType>) -> PyResult<Py<Self>> {
        self.toggle(py, &plugin_type, true)
    }
    pub fn add(&self, py: Python<'_>, plugin: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        self.add_at(py, plugin, PluginGroupPlacement::End)
    }
    pub fn add_before(
        &self,
        py: Python<'_>,
        target: Bound<'_, PyType>,
        plugin: Bound<'_, PyAny>,
    ) -> PyResult<Py<Self>> {
        self.add_at(
            py,
            plugin,
            PluginGroupPlacement::Before(member_key(&target)?),
        )
    }
    pub fn add_after(
        &self,
        py: Python<'_>,
        target: Bound<'_, PyType>,
        plugin: Bound<'_, PyAny>,
    ) -> PyResult<Py<Self>> {
        self.add_at(
            py,
            plugin,
            PluginGroupPlacement::After(member_key(&target)?),
        )
    }
    pub fn add_group(&self, py: Python<'_>, group: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        if !group.is_instance_of::<PyPluginGroup>() {
            return Err(PyTypeError::new_err(PLUGIN_GROUP_REQUIRED));
        }
        let result = build_plugin_group(&group)?;
        let incoming = result
            .cast::<Self>()
            .map_err(|_| PyTypeError::new_err(PLUGIN_GROUP_BUILD_RESULT))?
            .borrow();
        let mut builder = self.clone();
        builder
            .members
            .append(incoming.members.map_values(|value| value.clone_ref(py)));
        builder.default_seed |= incoming.default_seed;
        builder.into_python(py)
    }
    pub fn build(pyself: Bound<'_, Self>) -> Bound<'_, Self> {
        pyself
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for member in self.members.members() {
            if let MemberValue::Python(value) = &member.value {
                visit.call(value)?;
            }
        }
        Ok(())
    }

    pub fn finish(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        if app.borrow().is_reload_collection() || !self.default_seed {
            return app.borrow().with_group_configuration(|| {
                execute_members(
                    &app,
                    self.members
                        .members()
                        .iter()
                        .filter(|member| member.enabled)
                        .map(|member| member.value.clone_ref(app.py()))
                        .collect(),
                )
            });
        }
        finish_plugin_group(&self.name, || {
            app.borrow().with_bevy_app(|native| {
                let slots = default_slots();
                let mut builder = default_native_builder();
                for slot in &slots {
                    let intact = self.members.members().iter().any(|member| {
                        member.key == MemberKey::Native(slot.type_id)
                            && member.enabled
                            && matches!(member.value, MemberValue::DefaultNative(_))
                    });
                    if !intact {
                        builder = slot.disable(builder);
                    }
                }
                let failure = Arc::new(Mutex::new(None));
                let mut pending = Vec::new();
                for member in self.members.members() {
                    if !member.enabled {
                        continue;
                    }
                    match &member.value {
                        MemberValue::DefaultNative(slot)
                            if slots
                                .iter()
                                .any(|original| original.type_id == slot.type_id) =>
                        {
                            if !pending.is_empty() {
                                builder = place_members(
                                    builder,
                                    Some((*slot, false)),
                                    &app,
                                    mem::take(&mut pending),
                                    failure.clone(),
                                )?;
                            }
                        }
                        MemberValue::DefaultTail => {
                            if !pending.is_empty() {
                                builder = place_members(
                                    builder,
                                    slots.last().copied().map(|slot| (slot, true)),
                                    &app,
                                    mem::take(&mut pending),
                                    failure.clone(),
                                )?;
                            }
                        }
                        value => pending.push(value.clone_ref(app.py())),
                    }
                }
                if !pending.is_empty() {
                    builder = place_members(builder, None, &app, pending, failure.clone())?;
                }
                let result = catch_unwind(AssertUnwindSafe(|| {
                    native.add_plugins(builder);
                }));
                if let Some(error) = failure.lock().unwrap().take() {
                    return Err(error);
                }
                match result {
                    Ok(()) => Ok(()),
                    Err(payload) => resume_unwind(payload),
                }
            })?;
            app.borrow().with_bevy_app(|native| {
                if native.is_plugin_added::<RenderPlugin>() {
                    native.add_plugins(WgpuErrorHandlerPlugin);
                }
                if native.is_plugin_added::<WorldSerializationPlugin>() {
                    native.add_observer(pybevy_world_serialization::world_instance_ready_bridge);
                    // Bevy's own WorldSerializationPlugin ships in DefaultPlugins, so the
                    // Python wrapper's build never runs for this app.
                    pybevy_world_serialization::install_custom_component_materializer(native);
                }
                Ok(())
            })
        })
    }
}

fn execute_members(app: &Bound<'_, PyApp>, members: Vec<MemberValue>) -> PyResult<()> {
    for member in members {
        match member {
            MemberValue::Python(value) => {
                app.call_method1("add_plugins", (value,))?;
            }
            MemberValue::Native(slot) | MemberValue::DefaultNative(slot)
                if !app.borrow().is_reload_collection() =>
            {
                finish_plugin_group(slot.order_name(), || {
                    app.borrow().with_bevy_app(|native| {
                        slot.build(native);
                        Ok(())
                    })
                })?;
            }
            _ => {}
        }
    }
    Ok(())
}

struct GroupEnd;

fn place_members(
    builder: PluginGroupBuilder,
    anchor: Option<(NativePluginSlot, bool)>,
    app: &Bound<'_, PyApp>,
    members: Vec<MemberValue>,
    failure: Arc<Mutex<Option<PyErr>>>,
) -> PyResult<PluginGroupBuilder> {
    let owner = app.clone().unbind();
    let callback = move |native: &mut App| {
        Python::attach(|py| {
            let result = owner
                .borrow(py)
                .with_group_callback(native, || execute_members(owner.bind(py), members));
            if let Err(error) = result {
                *failure.lock().unwrap() = Some(error);
                panic!("plugin group member failed");
            }
        });
    };
    match anchor {
        Some((slot, after)) => slot
            .place(builder, Box::new(callback), after)
            .map_err(|_| PyRuntimeError::new_err(missing_group_plugin(slot.name))),
        None => Ok(builder.add(PluginGroupCallback::<GroupEnd, false>::new(callback))),
    }
}

#[pyclass(name = "DefaultPlugins", module = "pybevy.app", extends = PyPluginGroup)]
pub struct PyDefaultPlugins;

#[pymethods]
impl PyDefaultPlugins {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (Self, PyPluginGroup).into()
    }
    pub fn build(&self, py: Python<'_>) -> PyResult<Py<PyPluginGroupBuilder>> {
        let mut builder = PyPluginGroupBuilder::empty("DefaultPlugins".to_string());
        builder.default_seed = true;
        for slot in default_slots() {
            let key = MemberKey::Native(slot.type_id);
            let _ = builder.members.add(
                key.clone(),
                MemberValue::DefaultNative(slot),
                PluginGroupPlacement::End,
            );
            if slot.type_id == TypeId::of::<LogPlugin>() {
                builder.members.set_enabled(&key, false);
            }
        }
        let _ = builder.members.add(
            MemberKey::DefaultTail,
            MemberValue::DefaultTail,
            PluginGroupPlacement::End,
        );
        builder.into_python(py)
    }
    pub fn set(
        &self,
        py: Python<'_>,
        plugin: Bound<'_, PyAny>,
    ) -> PyResult<Py<PyPluginGroupBuilder>> {
        self.build(py)?.borrow(py).set(py, plugin)
    }
    pub fn finish(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        self.build(app.py())?.borrow(app.py()).finish(app)
    }
}

#[pyclass(name = "MinimalPlugins", module = "pybevy.app", extends = PyPluginGroup, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyMinimalPlugins;

#[pymethods]
impl PyMinimalPlugins {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (Self, PyPluginGroup).into()
    }
    pub fn build(&self, py: Python<'_>) -> PyResult<Py<PyPluginGroupBuilder>> {
        let mut builder = PyPluginGroupBuilder::empty("MinimalPlugins".to_string());
        for slot in [
            NativePluginSlot::of::<TaskPoolPlugin>(),
            NativePluginSlot::of::<FrameCountPlugin>(),
            NativePluginSlot::of::<TimePlugin>(),
            NativePluginSlot::of::<ScheduleRunnerPlugin>(),
        ] {
            let _ = builder.members.add(
                MemberKey::Native(slot.type_id),
                MemberValue::Native(slot),
                PluginGroupPlacement::End,
            );
        }
        builder.into_python(py)
    }
    pub fn finish(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        self.build(app.py())?.borrow(app.py()).finish(app)
    }
}
