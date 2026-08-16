use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{
        change_detection::Tick,
        component::ComponentId,
        resource::IsResource,
        world::{CommandQueue, World, unsafe_world_cell::UnsafeWorldCell},
    },
    gizmos::config::{DefaultGizmoConfigGroup, GizmoConfig, GizmoConfigStore},
    prelude::*,
};
use pybevy_core::{
    AccessMode, AssetAccessRegistry, AssetBorrowCounter, FieldStorage,
    ensure_asset_access_registry,
    public_error::{
        ASSET_ACCESS_REGISTRY_MISSING, GIZMOS_PLUGIN_REQUIRED, pipe_input_must_be_first,
        pipe_input_outside_pipe, pipe_target_requires_input, system_resource_not_found,
    },
    registry::global_registry,
    resource_initializer,
};
use pybevy_ecs::shared::{
    access_validation::{self as shared_validation},
    command_queue_helpers::create_commands_from_queue,
    message_store::{MessageRegistryCore, MessageTypeKey},
    param_spec::{
        BackendKeys, ComponentSpec, FilterSpec, KeyResolver, ParamSpec, QuerySpec, ResolvedAsset,
        ResolvedMessage, ResolvedResource, SchedulerAccess, conflict_error_message,
        to_param_accesses,
    },
    parity_trace::ParityRunHandle,
};
use pybevy_gizmos::{config::PyGizmoConfigStore, gizmos::PyGizmos};
use pybevy_reload::{HotReloadGeneration, SystemProfiler};
use pyo3::{
    PyTypeInfo,
    exceptions::{PyRuntimeError, PyTypeError},
    ffi::PyTypeObject,
    prelude::*,
    types::{PyTuple, PyType},
};
use smallvec::SmallVec;

use crate::{
    assets::assets::PyAssets,
    ecs::{
        commands::{CommandErrorSink, PyCommands},
        component_layout::{ComponentStorageType, ComponentStorageTypeExt},
        component_type::{
            ComponentRegistry, PyComponentType, ValidationIdentity, register_component_id,
            register_custom_component,
        },
        filter::QueryFilter,
        helpers::validity_guard::ValidityFlag,
        message::{PyMessageMutator, PyMessageReader, PyMessageWriter, python_message_cursor},
        messages::{CursorStorage, MessageType, MessageWorld, PyMessages},
        mutable::PyMut,
        observer::PyOn,
        python_message::{
            PythonMessageClassTable, PythonMessageStore, resolve_from_cell, resolve_from_world,
        },
        query::{
            query_param::QueryData,
            query_runtime::{CachedQuery, PyQueryIter},
            single_runtime::PySingleQuery,
        },
        resource_type::{PyResourceType, ResourceRegistry},
        state::{PyStateMachineRegistry, is_typed_state_resource, untyped_state_resource_name},
        system::{AssetTypePtr, SystemFunction, SystemParam, SystemParamType},
        view::{cached_view::CachedPyView, view::PyView, view_param::ViewParamType},
        world::PyWorld,
    },
};

/// Helper to lock a mutex, recovering from poison if a thread panicked while holding it.
pub(crate) fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| {
        bevy::log::warn!("Recovered from poisoned DynamicSystemInner mutex");
        poisoned.into_inner()
    })
}

/// Build one validity-bound asset scope from the world-owned native-type
/// registry declared by system initialization.
unsafe fn asset_borrow_counter_from_cell(
    world: UnsafeWorldCell<'_>,
    type_ptr: *const PyTypeObject,
    validity: &ValidityFlag,
    origin: impl Into<Arc<str>>,
) -> PyResult<AssetBorrowCounter> {
    let bridge = global_registry::get_asset_bridge_by_py_type(type_ptr)
        .expect("Assets[T] requires a registered asset bridge");
    // SAFETY: initialization declares a shared read of AssetAccessRegistry for
    // every Assets parameter, and the resource uses interior synchronization.
    let registry = unsafe { world.get_resource::<AssetAccessRegistry>() }
        .ok_or_else(|| PyRuntimeError::new_err(ASSET_ACCESS_REGISTRY_MISSING))?;
    Ok(AssetBorrowCounter::from_scope(registry.new_scope(
        bridge.bevy_type_id(),
        bridge.name(),
        validity.clone(),
        origin,
    )))
}

/// Handle to a DynamicSystem's Python-holding inner state.
/// Used by DynamicSystemRegistry to release Python references from old-generation systems.
pub(crate) type DynamicSystemHandle = pybevy_ecs::shared::system_runtime::SystemHandle<
    crate::ecs::system_interpreter::MainInterpreter,
>;

/// One buffered Python system error awaiting transfer into `LastSystemError`.
/// Kept off the world so `run_unsafe`'s parallel error path performs no
/// structural world mutation; the `Last`-schedule drain moves it into the resource.
pub(crate) struct BufferedSystemError {
    pub(crate) error: String,
    pub(crate) traceback: Option<String>,
}

/// Shared slot holding the most recent buffered system error. Cloned into every
/// DynamicSystem and into the `LastErrorBuffer` resource the drain system reads.
pub(crate) type SystemErrorBuffer = Arc<Mutex<Option<BufferedSystemError>>>;

/// Resource wrapper so the `Last`-schedule drain can reach the shared error
/// buffer through the world without `run_unsafe` ever touching the world on error.
#[derive(Resource)]
pub(crate) struct LastErrorBuffer {
    pub(crate) buffer: SystemErrorBuffer,
}

/// Inner state holding Python references that can be released on demand.
/// When `gut()` is called, all Python references are dropped to allow GC.
pub(crate) struct DynamicSystemInner {
    pub(crate) system_func: Option<SystemFunction>,
    pub(crate) cached_func: Option<Py<PyAny>>,
    pub(crate) cached_generation: u32,
    pub(crate) message_cursor_storage: Vec<crate::ecs::messages::CursorStorage>,
    pub(crate) gutted: bool,
}

// SAFETY: SystemFunction contains Py<PyAny> refs which are Send+Sync when GIL is properly managed.
// DynamicSystemInner is only accessed through Mutex which provides synchronization.
unsafe impl Send for DynamicSystemInner {}
unsafe impl Sync for DynamicSystemInner {}

impl DynamicSystemInner {}

/// Clear the system parameter cache.
/// Called when apps are dropped to prevent stale cache entries.
pub fn clear_system_param_cache() {
    SystemFunction::clear_cache();
}

/// Backend key types for the pyo3 backend (see `pybevy_ecs::shared::param_spec`).
pub(crate) struct MainKeys;

impl BackendKeys for MainKeys {
    type ComponentKey = PyComponentType;
    type ResourceKey = Py<PyType>;
    type AssetKey = AssetTypePtr;
    type MessageKey = MessageType;
}

pub(crate) fn resource_validation_identity(resource_type: &Py<PyType>) -> ValidationIdentity {
    let type_ptr = resource_type.as_ptr().cast::<PyTypeObject>();
    global_registry::get_resource_bridge_by_py_type(type_ptr)
        .map_or(ValidationIdentity::Python(type_ptr as usize), |bridge| {
            ValidationIdentity::Native(bridge.bevy_type_id())
        })
}

pub(crate) fn resource_marker_validation_identity() -> ValidationIdentity {
    ValidationIdentity::Native(TypeId::of::<IsResource>())
}

fn component_spec(
    comp_type: &PyComponentType,
    mutable: bool,
    optional: bool,
    py: Python<'_>,
) -> ComponentSpec<MainKeys> {
    let name = comp_type.display_name(py);
    let scheduler_access = if mutable || component_uses_python_storage(comp_type, py) {
        SchedulerAccess::Exclusive
    } else {
        SchedulerAccess::Shared
    };
    ComponentSpec {
        key: comp_type.clone(),
        label: name.clone(),
        name,
        mutable,
        scheduler_access,
        materializes: true,
        optional,
    }
}

fn component_uses_python_storage(comp_type: &PyComponentType, py: Python<'_>) -> bool {
    let PyComponentType::Custom(type_ptr) = comp_type else {
        return false;
    };
    // SAFETY: query parameters retain every custom class whose type pointer
    // they carry, so the pointer remains live throughout lowering.
    let type_obj =
        unsafe { Bound::from_borrowed_ptr(py, type_ptr.cast_mut().cast::<pyo3::ffi::PyObject>()) };
    type_obj
        .cast::<PyType>()
        .ok()
        .and_then(|class| ComponentStorageType::from_python_class(class).ok())
        .is_none_or(|storage| matches!(storage, ComponentStorageType::PyObject))
}

fn resource_uses_python_storage(resource_type: &Bound<'_, PyType>) -> bool {
    if untyped_state_resource_name(resource_type).is_some()
        || is_typed_state_resource(resource_type)
    {
        return false;
    }
    resource_type
        .getattr("__pybevy_resource_decorated__")
        .ok()
        .and_then(|marker| marker.is_truthy().ok())
        .unwrap_or(false)
}

fn filter_spec(comp_type: &PyComponentType, py: Python<'_>) -> FilterSpec<MainKeys> {
    FilterSpec {
        key: comp_type.clone(),
        label: comp_type.display_name(py),
    }
}

fn tick_filter_component_spec(
    comp_type: &PyComponentType,
    optional: bool,
    py: Python<'_>,
) -> ComponentSpec<MainKeys> {
    let mut spec = component_spec(comp_type, false, optional, py);
    spec.scheduler_access = SchedulerAccess::Shared;
    spec.materializes = false;
    spec
}

/// Lower one parsed parameter into the shared backend-neutral IR.
///
/// Component names and disjointness labels are class-name strings resolved
/// through the caller's interpreter token; resource validation keys are
/// type-object pointers. Both match the pre-IR validation semantics.
pub(crate) fn lower_param_type(ty: &SystemParamType, py: Python<'_>) -> ParamSpec<MainKeys> {
    match ty {
        SystemParamType::PipeInput { .. } => ParamSpec::Input,
        SystemParamType::Query { param: query_param } => {
            let mut spec = QuerySpec::default();
            let mut has_logical_type = false;
            for data in &query_param.data {
                match data {
                    QueryData::Component {
                        ty: comp_type,
                        mutable,
                        optional,
                        logical_type_id,
                    } => {
                        has_logical_type |= logical_type_id.is_some();
                        spec.components
                            .push(component_spec(comp_type, *mutable, *optional, py));
                    }
                    QueryData::Has { ty } => spec.resolve_only.push(*ty),
                    QueryData::AnyOf { items } => {
                        for item in items {
                            has_logical_type |= item.logical_type_id.is_some();
                            spec.components
                                .push(component_spec(&item.ty, item.mutable, true, py));
                        }
                    }
                    QueryData::Entity => {}
                }
            }
            if has_logical_type {
                let logical_map_type = Python::attach(|py| {
                    crate::ecs::logical_type::PyLogicalTypeMap::type_object(py).as_type_ptr()
                });
                spec.components.push(component_spec(
                    &PyComponentType::Dynamic(logical_map_type),
                    false,
                    true,
                    py,
                ));
            }
            for filter in &query_param.filters {
                match filter {
                    QueryFilter::With(with) => {
                        for comp_type in &with.values {
                            spec.with.push(filter_spec(comp_type, py));
                        }
                    }
                    QueryFilter::Without(without) => {
                        for comp_type in &without.values {
                            spec.without.push(filter_spec(comp_type, py));
                        }
                    }
                    QueryFilter::Changed(changed) => {
                        spec.changed.push(filter_spec(&changed.component_type, py));
                    }
                    QueryFilter::Added(added) => {
                        spec.added.push(filter_spec(&added.component_type, py));
                    }
                    QueryFilter::Or(or) => {
                        for branch in &or.values {
                            match branch {
                                QueryFilter::With(filter) => {
                                    spec.resolve_only.extend(filter.values.iter().copied());
                                }
                                QueryFilter::Without(filter) => {
                                    spec.resolve_only.extend(filter.values.iter().copied());
                                }
                                QueryFilter::Changed(filter) => {
                                    spec.components.push(tick_filter_component_spec(
                                        &filter.component_type,
                                        true,
                                        py,
                                    ));
                                }
                                QueryFilter::Added(filter) => {
                                    spec.components.push(tick_filter_component_spec(
                                        &filter.component_type,
                                        true,
                                        py,
                                    ));
                                }
                                QueryFilter::Or(_) => {
                                    unreachable!(
                                        "Or construction accepts only simple query filters"
                                    );
                                }
                            }
                        }
                    }
                }
            }
            ParamSpec::Query(spec)
        }
        SystemParamType::View { param: view_param } => {
            let mut spec = QuerySpec::default();
            for param_type in &view_param.parameters {
                let ViewParamType::Component { comp_type, mutable } = param_type;
                spec.components
                    .push(component_spec(comp_type, *mutable, false, py));
            }
            for comp_type in &view_param.with_filters {
                spec.with.push(filter_spec(comp_type, py));
            }
            for comp_type in &view_param.without_filters {
                spec.without.push(filter_spec(comp_type, py));
            }
            for comp_type in &view_param.changed_filters {
                spec.changed.push(filter_spec(comp_type, py));
            }
            for comp_type in &view_param.added_filters {
                spec.added.push(filter_spec(comp_type, py));
            }
            ParamSpec::View(spec)
        }
        SystemParamType::Resource { type_obj, mutable } => {
            let type_ptr = type_obj.as_ptr() as usize;
            let name = type_obj
                .bind(py)
                .name()
                .map_or_else(|_| "resource".to_string(), |name| name.to_string());
            let python_storage = resource_uses_python_storage(type_obj.bind(py));
            ParamSpec::Res {
                key: type_obj.clone_ref(py),
                vkey: Some(type_ptr),
                name,
                mutable: *mutable,
                scheduler_access: if *mutable || python_storage {
                    SchedulerAccess::Exclusive
                } else {
                    SchedulerAccess::Shared
                },
            }
        }
        SystemParamType::Assets {
            type_ptr,
            wrapper_class: _,
            logical_type_id: _,
            logical_type_name,
            mutable,
        } => {
            // Access is declared under the real asset type: `type_ptr` (e.g.
            // ShaderMaterial) owns the registered AssetBridge, so `asset_id`
            // resolves it to the Assets<T> ComponentId. A `@material` wrapper
            // (e.g. GroundMaterial) has no bridge, so keying access on the
            // wrapper would declare no Assets<T> access at all while the system
            // still reads/writes the underlying collection -- a data race
            // versus native asset systems. Intra-system validation must use
            // that same physical identity: distinct `@material` classes are
            // logical views over one `Assets<ShaderMaterial>` resource, not
            // independent collections.
            let vkey = format!("{:p}", type_ptr.0);
            let name = logical_type_name.clone().unwrap_or_else(|| {
                global_registry::get_asset_bridge_by_py_type(type_ptr.0)
                    .map_or_else(|| "asset".to_string(), |bridge| bridge.name().to_string())
            });
            ParamSpec::Assets {
                key: *type_ptr,
                vkey,
                name,
                mutable: *mutable,
            }
        }
        SystemParamType::World => ParamSpec::World,
        SystemParamType::Commands => ParamSpec::Commands,
        SystemParamType::Gizmos => {
            let type_obj = PyGizmoConfigStore::type_object(py);
            ParamSpec::Gizmos {
                key: type_obj.clone().unbind(),
                name: "GizmoConfigStore".to_string(),
            }
        }
        SystemParamType::Local(_) => ParamSpec::Local,
        SystemParamType::MessageWriter { message_type } => ParamSpec::MessageWriter {
            key: message_type.clone(),
        },
        SystemParamType::MessageReader { message_type } => ParamSpec::MessageReader {
            key: message_type.clone(),
            scheduler_access: if matches!(message_type, MessageType::Custom(_)) {
                SchedulerAccess::Exclusive
            } else {
                SchedulerAccess::Shared
            },
        },
        SystemParamType::MessageMutator { message_type } => ParamSpec::MessageMutator {
            key: message_type.clone(),
        },
        SystemParamType::On { .. } => ParamSpec::Observer,
    }
}

/// Lower a whole signature into the shared IR, in signature order.
pub(crate) fn lower_params(params: &[SystemParam], py: Python<'_>) -> Vec<ParamSpec<MainKeys>> {
    params.iter().map(|p| lower_param_type(&p.ty, py)).collect()
}

/// Validate a function's parameters for conflicting ECS access: double mutable
/// access, mixing mutable and immutable, or a `World` parameter alongside
/// anything else. Regular systems run this via
/// [`validate_system_params`]; observer registration calls it
/// directly, since observers skip the `add_systems` gate.
pub(crate) fn validate_system_params(
    params: &[SystemParam],
    func_name: &str,
    py: Python<'_>,
) -> PyResult<()> {
    validate_system_params_with_input(params, func_name, py, false)
}

pub(crate) fn validate_pipe_target_params(
    params: &[SystemParam],
    func_name: &str,
    py: Python<'_>,
) -> PyResult<()> {
    validate_system_params_with_input(params, func_name, py, true)
}

fn validate_system_params_with_input(
    params: &[SystemParam],
    func_name: &str,
    py: Python<'_>,
    pipe_target: bool,
) -> PyResult<()> {
    let input_indices = params
        .iter()
        .enumerate()
        .filter_map(|(index, param)| {
            matches!(param.ty, SystemParamType::PipeInput { .. }).then_some(index)
        })
        .collect::<Vec<_>>();
    if pipe_target {
        match input_indices.as_slice() {
            [0] => {}
            [] => return Err(PyTypeError::new_err(pipe_target_requires_input(func_name))),
            _ => return Err(PyTypeError::new_err(pipe_input_must_be_first(func_name))),
        }
    } else if !input_indices.is_empty() {
        return Err(PyTypeError::new_err(pipe_input_outside_pipe(func_name)));
    }

    let specs = lower_params(params, py);
    let accesses = to_param_accesses(
        &specs,
        PyComponentType::validation_identity,
        resource_validation_identity,
        resource_marker_validation_identity(),
        MessageType::validation_identity,
    );
    shared_validation::validate_access(&accesses)
        .map_err(|conflict| PyRuntimeError::new_err(conflict_error_message(func_name, &conflict)))
}

/// Resolves pyo3 backend keys against the world during the shared access walk.
///
/// Borrows the system's custom-component cache so custom components are
/// registered on first sight and reused by the runtime (`PyQueryIter::new`
/// reads this cache).
pub(crate) struct MainResolver<'py, 'a> {
    pub(crate) custom_component_ids: &'a mut HashMap<*const PyTypeObject, ComponentId>,
    pub(crate) py: Python<'py>,
}

impl KeyResolver<MainKeys> for MainResolver<'_, '_> {
    fn component_id(&mut self, world: &mut World, key: &PyComponentType) -> Option<ComponentId> {
        Some(match key {
            PyComponentType::Custom(type_ptr) => {
                if let Some(&id) = self.custom_component_ids.get(type_ptr) {
                    id
                } else {
                    let id = register_custom_component(world, *type_ptr, self.py);
                    self.custom_component_ids.insert(*type_ptr, id);
                    id
                }
            }
            _ => register_component_id(world, key, self.custom_component_ids, self.py),
        })
    }

    fn component_scheduler_access(
        &mut self,
        world: &World,
        key: &PyComponentType,
        component_id: ComponentId,
        requested: SchedulerAccess,
        materializes: bool,
        mutable: bool,
    ) -> SchedulerAccess {
        if mutable || !matches!(key, PyComponentType::Custom(_)) {
            return requested;
        }
        if !materializes {
            return SchedulerAccess::Shared;
        }
        let uses_python_storage = world
            .get_resource::<ComponentRegistry>()
            .and_then(|registry| registry.storage_type(component_id))
            .is_none_or(|storage| matches!(storage, ComponentStorageType::PyObject));
        if uses_python_storage {
            SchedulerAccess::Exclusive
        } else {
            SchedulerAccess::Shared
        }
    }

    fn resource_ids(&mut self, world: &mut World, key: &Py<PyType>) -> ResolvedResource {
        let type_bound = key.bind(self.py);
        let resource_type = PyResourceType::try_from((type_bound, self.py)).ok();
        let untyped_state = untyped_state_resource_name(type_bound).is_some();
        let Some(rt) = resource_type else {
            return ResolvedResource::default();
        };
        // Register (not just look up) so access is declared even when the
        // resource is inserted after schedule init; an undeclared access
        // would let conflicting systems race (UB).
        let primary = rt.register_component_id(world, self.py);
        let mut aux_reads = Vec::new();
        // Custom resource params resolve their dynamic ComponentId through the
        // neutral registry at runtime. The value itself is covered by `primary`.
        if matches!(rt, PyResourceType::Custom(_)) {
            aux_reads.push(world.register_component::<ResourceRegistry>());
        }
        if untyped_state {
            aux_reads.push(world.register_component::<PyStateMachineRegistry>());
        }
        ResolvedResource { primary, aux_reads }
    }

    fn asset_ids(&mut self, world: &mut World, key: &AssetTypePtr) -> ResolvedAsset {
        // Register the Assets<T> id up front; a bridge that exists always
        // yields an id, so access is never silently dropped.
        let primary = global_registry::get_asset_bridge_by_py_type(key.0)
            .map(|bridge| bridge.register_resource_id(world));
        ensure_asset_access_registry(world);
        ResolvedAsset {
            primary,
            aux_reads: vec![world.register_component::<AssetAccessRegistry>()],
        }
    }

    fn message_ids(
        &mut self,
        world: &mut World,
        key: &MessageType,
        write: bool,
    ) -> ResolvedMessage {
        let mut resolved = ResolvedMessage::default();
        if let MessageType::Custom(py_type) = key {
            resolved
                .reads
                .push(world.register_component::<PythonMessageStore>());
            resolved
                .reads
                .push(world.register_component::<MessageRegistryCore>());
            resolved
                .reads
                .push(world.register_component::<PythonMessageClassTable>());
            let type_key = MessageTypeKey::new(py_type.bind(self.py).as_type_ptr() as usize);
            if let Some(access_id) =
                world
                    .get_resource::<MessageRegistryCore>()
                    .and_then(|registry| {
                        registry
                            .channel_for_type(type_key)
                            .and_then(|channel| registry.metadata(channel))
                            .map(|metadata| metadata.access_id)
                    })
            {
                if write {
                    resolved.writes.push(access_id);
                } else {
                    resolved.reads.push(access_id);
                }
            }
            return resolved;
        }
        if write {
            // Message buffers are resources; register (not just look up) the
            // id so access is declared even before the first write. Custom
            // messages are the one residual lookup-only case (see
            // MessageType::register_resource_id).
            if let Some(id) = key.register_resource_id(world) {
                resolved.writes.push(id);
            }
        } else {
            // reader_resource_ids returns every resource touched while
            // materializing the message value.
            resolved.reads.extend(key.reader_resource_ids(world));
        }
        resolved
    }

    fn gizmo_config_id(&mut self, world: &mut World) -> ComponentId {
        world.register_component::<GizmoConfigStore>()
    }

    fn infrastructure_reads(&mut self, world: &mut World) -> Vec<ComponentId> {
        vec![
            // run_unsafe's generation guard reads HotReloadGeneration.
            world.register_component::<HotReloadGeneration>(),
            // run_unsafe's profiling epilogue reads Time and SystemProfiler
            // via shared world access every run; declaring both lets the
            // parallel executor account for the reads and never schedule a
            // conflicting writer alongside.
            world.register_component::<Time>(),
            world.register_component::<SystemProfiler>(),
        ]
    }

    fn resource_marker_id(&mut self, world: &mut World) -> Option<ComponentId> {
        Some(world.register_component::<bevy::ecs::resource::IsResource>())
    }
}

fn missing_resource_error(type_bound: &Bound<'_, PyType>, func_name: &str) -> PyErr {
    let type_name = type_bound
        .name()
        .map(|name| name.to_string())
        .unwrap_or_else(|_| "Unknown".to_string());
    let custom = type_bound
        .getattr("__pybevy_resource_decorated__")
        .ok()
        .and_then(|marker| marker.is_truthy().ok())
        .unwrap_or(false);
    PyTypeError::new_err(system_resource_not_found(func_name, type_name, custom))
}

/// Helper to wrap a resource in Res or ResMut based on mutability.
fn wrap_resource_in_res<'py>(
    py: Python<'py>,
    resource: Py<PyAny>,
    mutable: bool,
    args_buffer: &mut SmallVec<[Py<PyAny>; 8]>,
) {
    if mutable {
        // Wrap in ResMut[ResourceType]
        let resource_bound = resource.into_bound(py);
        let resmut_wrapper = Py::new(py, crate::ecs::resource::PyResMut::new(resource_bound))
            .expect("Failed to create PyResMut");
        args_buffer.push(resmut_wrapper.into_any());
    } else {
        // Wrap in Res[ResourceType]
        let resource_bound = resource.into_bound(py);
        let res_wrapper = Py::new(py, crate::ecs::resource::PyRes::new(resource_bound))
            .expect("Failed to create PyRes");
        args_buffer.push(res_wrapper.into_any());
    }
}

/// Build the Python argument list for one run of a non-observer system.
///
/// The shared run scaffold calls this as its `build_args` hook; the
/// adapter-specific argument construction is the counterpart. Fills
/// `args_buffer` in parameter order and returns `Some(err)` on the first
/// parameter that fails to resolve (e.g. a missing resource), leaving the
/// remaining parameters unbuilt.
///
/// # Safety
/// `world` must be this run's valid `UnsafeWorldCell`, and `validity` must
/// stay active for as long as any built argument can read through the cell.
/// Each arm's access is bounded by what `initialize` declared for that
/// parameter; the executor prevents a conflicting system from running
/// concurrently, so the cell's unchecked borrows are unique.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn build_run_args<'w, 'c1, 'c2>(
    py: Python<'_>,
    params: &[SystemParam],
    query_caches: &[CachedQuery],
    view_caches: &[Result<Arc<CachedPyView>, Arc<str>>],
    message_cursor_storage: &[CursorStorage],
    commands_storage: &mut Option<Commands<'c1, 'c2>>,
    args_buffer: &mut SmallVec<[Py<PyAny>; 8]>,
    world: UnsafeWorldCell<'w>,
    validity: &ValidityFlag,
    last_run: Tick,
    this_run: Tick,
    func_name: &str,
    command_error_sink: &CommandErrorSink,
    parity_trace: Option<ParityRunHandle>,
) -> Option<PyErr> {
    let mut message_reader_idx = 0usize;
    let mut query_cache_idx = 0usize;
    let mut view_cache_idx = 0usize;
    for param in params {
        match &param.ty {
            SystemParamType::PipeInput { .. } => {
                // The interpreter adapter prepends the owned value supplied by
                // the previous pipe stage before building ECS parameters.
            }
            SystemParamType::Local(local) => {
                args_buffer.push(local.clone_ref(py));
            }
            SystemParamType::Resource { type_obj, mutable } => {
                // Fetch resource from world using PyResourceType
                let type_bound = type_obj.bind(py);
                if let Some(state_name) = untyped_state_resource_name(type_bound) {
                    // SAFETY: initialize declared a read of PyStateMachineRegistry
                    // for untyped State/NextState parameters.
                    let ambiguous = unsafe { world.get_resource::<PyStateMachineRegistry>() }
                        .is_some_and(|registry| registry.is_ambiguous());
                    if ambiguous {
                        return Some(PyTypeError::new_err(format!(
                            "System `{func_name}`: untyped {state_name} resource is ambiguous; use {state_name}[YourState]",
                        )));
                    }
                }
                let resource_type = match PyResourceType::try_from((type_bound, py)) {
                    Ok(rt) => rt,
                    Err(e) => {
                        return Some(e);
                    }
                };

                // Use appropriate extraction method based on mutability.
                // Both paths go through narrow cell accessors so no `&World`
                // or `&mut World` is ever materialized for a resource read.
                let resource = if *mutable {
                    // SAFETY: `initialize` declared write access to this
                    // resource (AssetServer/Dynamic bridge id, or the
                    // ResourceRegistry read for Custom);
                    // the executor prevents a conflicting system from running
                    // concurrently, so the cell's unchecked borrow is unique.
                    match unsafe { resource_type.get_from_cell_mut(world, py, validity.clone()) } {
                        Ok(r) => r,
                        Err(_e) => {
                            return Some(missing_resource_error(type_bound, func_name));
                        }
                    }
                } else {
                    // SAFETY: `initialize` declared read access to this
                    // resource; the executor prevents a concurrent writer, so
                    // the cell's unchecked read is unique.
                    match unsafe { resource_type.get_from_cell(world, py, validity.clone()) } {
                        Ok(r) => r,
                        Err(_e) => {
                            return Some(missing_resource_error(type_bound, func_name));
                        }
                    }
                };

                wrap_resource_in_res(py, resource, *mutable, args_buffer);
            }
            SystemParamType::Query { .. } => {
                // Static per-parameter state was built once in `initialize`;
                // borrow the cached QueryState by raw pointer (fenced by the
                // ValidityFlag) instead of rebuilding and conjuring `&mut World`.
                let cached = &query_caches[query_cache_idx];
                query_cache_idx += 1;
                if cached.single_entity_enforced {
                    // SAFETY: `world` is this run's UnsafeWorldCell; the declared
                    // access from `initialize` covers this cached state and the
                    // executor prevents conflicting systems from running
                    // concurrently, so the query's access is unique. `cached`
                    // lives in scheduled run state, which outlives the run.
                    let single_query = unsafe {
                        PySingleQuery::new(cached, world, validity.clone(), last_run, this_run)
                    };

                    let obj = Py::new(py, single_query).expect("Failed to create PySingleQuery");
                    args_buffer.push(obj.into_any());
                } else {
                    // SAFETY: as above, unique access to the cached state's
                    // components is guaranteed by the declared-access scheduling.
                    let query_runtime = unsafe {
                        PyQueryIter::new(cached, world, validity.clone(), last_run, this_run)
                    };

                    let obj = Py::new(py, query_runtime).expect("Failed to create PyQueryIter");
                    args_buffer.push(obj.into_any());
                }
            }
            SystemParamType::View { .. } => {
                let cached = match &view_caches[view_cache_idx] {
                    Ok(cached) => Arc::clone(cached),
                    Err(message) => {
                        return Some(PyRuntimeError::new_err(message.to_string()));
                    }
                };
                view_cache_idx += 1;
                // SAFETY: this cache was built in `initialize` from the
                // exact parameter descriptor used for declared scheduler
                // access. The cell, ticks, and validity all belong to this
                // one scaffold run.
                let py_view = match unsafe {
                    PyView::new_cached(cached, last_run, this_run, world, validity.clone())
                } {
                    Ok(view) => view,
                    Err(error) => {
                        return Some(PyRuntimeError::new_err(error.to_string()));
                    }
                };
                let obj = Py::new(py, py_view).expect("Failed to create PyView");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::World => {
                // A World param requests exclusive access: `flags()` marks this
                // system EXCLUSIVE, so the executor runs it with no other
                // system in flight (initialize declares an empty access set,
                // matching Bevy's ExclusiveFunctionSystem).
                // SAFETY: exclusive scheduling guarantees this `&mut World` is
                // the only borrow of the world for the duration of the run.
                let world_mut = unsafe { world.world_mut() };
                let py_world = unsafe { PyWorld::new(world_mut, validity.clone()) };
                let obj = Py::new(py, py_world).expect("Failed to create PyWorld");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::Commands => {
                // Use the pre-created Commands from commands_storage
                let commands = commands_storage
                    .as_mut()
                    .expect("Commands should be pre-created");
                let py_commands = unsafe {
                    PyCommands::new(
                        commands,
                        validity.clone(),
                        command_error_sink.clone(),
                        parity_trace.clone(),
                    )
                };
                let obj = Py::new(py, py_commands).expect("Failed to create PyCommands");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::Gizmos => {
                let Some(config) = unsafe { world.get_resource::<GizmoConfigStore>() }
                    .and_then(|store| store.get_config::<DefaultGizmoConfigGroup>())
                    .map(|(config, _)| config)
                else {
                    return Some(PyRuntimeError::new_err(GIZMOS_PLUGIN_REQUIRED));
                };
                // SAFETY: ParamSpec::Gizmos declared a read of
                // GizmoConfigStore, and the shared run validity expires before
                // deferred world mutation.
                let config = unsafe {
                    FieldStorage::borrowed(
                        config as *const GizmoConfig as *mut GizmoConfig,
                        validity.with_access_mode(AccessMode::Read),
                    )
                };
                let obj = Py::new(py, PyGizmos::new(config, validity.clone()))
                    .expect("Failed to create PyGizmos");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::MessageWriter { message_type } => {
                if let MessageType::Custom(py_type) = message_type {
                    // SAFETY: KeyResolver declared reads for the neutral store,
                    // registry, and class table plus this channel's synthetic write.
                    let resolved =
                        match unsafe { resolve_from_cell(world, py_type.bind(py).as_type_ptr()) } {
                            Ok(resolved) => resolved,
                            Err(error) => return Some(error),
                        };
                    let py_writer = PyMessageWriter::python(
                        message_type.clone(),
                        resolved,
                        validity.clone(),
                        parity_trace.clone(),
                    );
                    let obj = Py::new(py, py_writer).expect("Failed to create PyMessageWriter");
                    args_buffer.push(obj.into_any());
                    continue;
                }
                // Create PyMessageWriter with narrow cell-based world access.
                // SAFETY: `initialize` declares write access for this
                // writer's Messages<T> id; the writer only reaches that buffer.
                let mw = unsafe { MessageWorld::new(world, validity.clone()) };
                let py_writer =
                    PyMessageWriter::native(message_type.clone(), mw, parity_trace.clone());
                let obj = Py::new(py, py_writer).expect("Failed to create PyMessageWriter");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::MessageReader { message_type } => {
                let cursor = message_cursor_storage.get(message_reader_idx).cloned();
                message_reader_idx += 1;
                if let MessageType::Custom(py_type) = message_type {
                    // SAFETY: KeyResolver declared all resources read by resolution.
                    let resolved =
                        match unsafe { resolve_from_cell(world, py_type.bind(py).as_type_ptr()) } {
                            Ok(resolved) => resolved,
                            Err(error) => return Some(error),
                        };
                    let cursor = match python_message_cursor(cursor) {
                        Ok(cursor) => cursor,
                        Err(error) => return Some(error),
                    };
                    let py_reader = PyMessageReader::python(resolved, cursor, validity.clone());
                    let obj = Py::new(py, py_reader).expect("Failed to create PyMessageReader");
                    args_buffer.push(obj.into_any());
                    continue;
                }
                // Create PyMessageReader with narrow cell-based world access.
                // SAFETY: `initialize` declares reads for this reader's
                // resource ids; the reader only reaches those.
                let mw_1 = unsafe { MessageWorld::new(world, validity.clone()) };
                let py_messages = PyMessages {
                    message_type: message_type.clone(),
                    world: mw_1,
                    cursor_storage: cursor,
                };
                let py_reader = PyMessageReader::native(py_messages);
                let obj = Py::new(py, py_reader).expect("Failed to create PyMessageReader");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::MessageMutator { message_type } => {
                let cursor = message_cursor_storage.get(message_reader_idx).cloned();
                message_reader_idx += 1;
                let MessageType::Custom(py_type) = message_type else {
                    return Some(PyTypeError::new_err(
                        "MessageMutator currently supports custom Python messages only",
                    ));
                };
                // SAFETY: the shared access walk declares store/registry/class-table
                // reads and this channel's synthetic write access.
                let resolved =
                    match unsafe { resolve_from_cell(world, py_type.bind(py).as_type_ptr()) } {
                        Ok(resolved) => resolved,
                        Err(error) => return Some(error),
                    };
                let cursor = match python_message_cursor(cursor) {
                    Ok(cursor) => cursor,
                    Err(error) => return Some(error),
                };
                let py_mutator = PyMessageMutator::python(
                    message_type.clone(),
                    resolved,
                    cursor,
                    validity.clone(),
                    parity_trace.clone(),
                );
                let obj = Py::new(py, py_mutator).expect("Failed to create PyMessageMutator");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::On { .. } => {
                // On parameters are only valid in observer contexts.
                // Observer dispatch uses the shared trigger-aware shell.
                unreachable!(
                    "On parameter in non-observer system: observers use the trigger-aware runtime"
                )
            }
            SystemParamType::Assets {
                type_ptr,
                wrapper_class,
                logical_type_id,
                logical_type_name,
                mutable,
            } => {
                let borrow_counter = match unsafe {
                    asset_borrow_counter_from_cell(
                        world,
                        type_ptr.0,
                        validity,
                        format!("system:{func_name}"),
                    )
                } {
                    Ok(counter) => counter,
                    Err(error) => return Some(error),
                };
                // Create PyAssets wrapper with cell-based world access.
                // SAFETY: `world` is this run's UnsafeWorldCell; `initialize`
                // declares this Assets<T> resource's access, which
                // bounds the data PyAssets reaches via the AssetBridge.
                let py_assets = unsafe {
                    PyAssets::new(
                        type_ptr.0,
                        wrapper_class.map(|w| w.0),
                        *logical_type_id,
                        logical_type_name.clone(),
                        world,
                        validity.clone(),
                        *mutable,
                        borrow_counter,
                    )
                };
                let obj = Py::new(py, resource_initializer(py_assets))
                    .expect("Failed to create PyAssets");

                if *mutable {
                    // Wrap in Mut[Assets[T]]
                    let assets_any: Bound<'_, PyAny> = obj.into_bound(py).into_any();
                    let mut_wrapper =
                        Py::new(py, PyMut::new(assets_any)).expect("Failed to create PyMut");
                    args_buffer.push(mut_wrapper.into_any());
                } else {
                    args_buffer.push(obj.into_any());
                }
            }
        }
    }
    None
}

/// Build arguments and invoke one observer callable using the caller's
/// validity window and callback-local command queue.
///
/// # Safety
/// `world` must be held exclusively for the complete call and `validity` must
/// remain active until every argument and transient query cache has dropped.
pub(crate) unsafe fn execute_prepared_observer(
    py: Python,
    callable: &Bound<'_, PyAny>,
    params: &[SystemParam],
    world_cell: UnsafeWorldCell<'_>,
    command_queue: &mut CommandQueue,
    on_param: Py<PyOn>,
    validity: &ValidityFlag,
    command_error_sink: &CommandErrorSink,
    message_cursor_storage: &[CursorStorage],
    parity_trace: Option<ParityRunHandle>,
) -> PyResult<()> {
    // SAFETY: the observer core supplies exclusive access for this callback;
    // every derived wrapper is fenced by `validity` and drops before return.
    let world = unsafe { world_cell.world_mut() };
    ensure_asset_access_registry(world);
    // Transient per-Query cached states for this observer dispatch. Observers have no
    // scheduled cache (they run outside the schedule), but they hold an exclusive
    // `&mut World`, so building a CachedQuery here is sound. Boxed so each state has a
    // stable heap address while a PyQueryIter holds a raw pointer to it. Declared before
    // the validity guard so it drops LAST: the guard invalidates the shared flag before
    // these caches are freed, so any leaked PyQueryIter sees "invalid" before use.
    let mut transient_caches: Vec<Box<CachedQuery>> = Vec::new();

    let needs_commands = params
        .iter()
        .any(|param| matches!(param.ty, SystemParamType::Commands));
    // Keep the Commands façade alive through argument construction and the
    // Python call: PyCommands stores a validity-fenced pointer to this value.
    let mut commands =
        needs_commands.then(|| create_commands_from_queue(command_queue, world_cell));
    // Declared after `commands` and transient caches so Python argument
    // destruction (including reentrant finalizers) happens while every raw
    // pointer target is still alive. The core invalidates immediately after
    // this helper returns.
    let mut args_buffer: Vec<Py<PyAny>> = Vec::with_capacity(params.len());
    let mut message_reader_idx = 0usize;

    // Build the executor's ptr-keyed custom_component_ids cache from the neutral
    // (usize-keyed) registry, recovering each `*const PyTypeObject` from its
    // stored `type_ptr as usize` identity.
    let custom_component_ids = {
        let registry = world.get_resource::<crate::ecs::component_type::ComponentRegistry>();
        if let Some(reg) = registry {
            Arc::new(
                reg.ids_by_type()
                    .iter()
                    .map(|(&type_id, &id)| (type_id as *const PyTypeObject, id))
                    .collect::<HashMap<*const PyTypeObject, ComponentId>>(),
            )
        } else {
            Arc::new(HashMap::new())
        }
    };

    for param in params {
        match &param.ty {
            SystemParamType::PipeInput { .. } => {
                return Err(PyTypeError::new_err(
                    "In[...] is only valid as the first parameter of a downstream pipe stage",
                ));
            }
            SystemParamType::On { .. } => {
                args_buffer.push(on_param.clone_ref(py).into_any());
            }
            SystemParamType::Commands => {
                // SAFETY: the neutral observer core owns this callback-local
                // queue and matching exclusive World cell until all arguments
                // have dropped; it applies the queue only after invalidation.
                let commands = commands
                    .as_mut()
                    .expect("Commands parameter requires a local Commands façade");
                let py_commands = unsafe {
                    PyCommands::new(
                        commands,
                        validity.clone(),
                        command_error_sink.clone(),
                        parity_trace.clone(),
                    )
                };
                let obj = Py::new(py, py_commands).expect("Failed to create PyCommands");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::Gizmos => {
                let Some(config) = world
                    .get_resource::<GizmoConfigStore>()
                    .and_then(|store| store.get_config::<DefaultGizmoConfigGroup>())
                    .map(|(config, _)| config)
                else {
                    return Err(PyRuntimeError::new_err(GIZMOS_PLUGIN_REQUIRED));
                };
                // SAFETY: observer execution owns this world access window,
                // and the shared validity expires before its local command
                // queue is applied.
                let config = unsafe {
                    FieldStorage::borrowed(
                        config as *const GizmoConfig as *mut GizmoConfig,
                        validity.with_access_mode(AccessMode::Read),
                    )
                };
                let obj = Py::new(py, PyGizmos::new(config, validity.clone()))
                    .expect("Failed to create PyGizmos");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::Query { param: query_param } => {
                // Build a transient cached state; the exclusive &mut World makes this
                // sound in the observer context (no parallel systems here).
                transient_caches.push(Box::new(CachedQuery::build(
                    world,
                    query_param.clone(),
                    custom_component_ids.clone(),
                    py,
                )));
                let this_run = world.change_tick();
                let cell = world.as_unsafe_world_cell();
                // The box's heap address is stable across later pushes, so this
                // reference (used only to hand a pointer to the query) stays valid.
                let cached_ref: &CachedQuery = transient_caches.last().unwrap();
                if query_param.single_entity_enforced {
                    // SAFETY: exclusive &mut World during observer dispatch; the cached
                    // state and cell reference this same World. `transient_caches` keeps
                    // the box alive until after the Python call. No prior run for
                    // observers, so last_run = Tick(0).
                    let single_query = unsafe {
                        PySingleQuery::new(
                            cached_ref,
                            cell,
                            validity.clone(),
                            Tick::new(0),
                            this_run,
                        )
                    };
                    let obj = Py::new(py, single_query).expect("Failed to create PySingleQuery");
                    args_buffer.push(obj.into_any());
                } else {
                    // SAFETY: see above.
                    let query_runtime = unsafe {
                        PyQueryIter::new(cached_ref, cell, validity.clone(), Tick::new(0), this_run)
                    };
                    let obj = Py::new(py, query_runtime).expect("Failed to create PyQueryIter");
                    args_buffer.push(obj.into_any());
                }
            }
            SystemParamType::Resource { type_obj, mutable } => {
                let type_bound = type_obj.bind(py);
                if let Some(state_name) = untyped_state_resource_name(type_bound)
                    && world
                        .get_resource::<PyStateMachineRegistry>()
                        .is_some_and(|registry| registry.is_ambiguous())
                {
                    return Err(PyTypeError::new_err(format!(
                        "untyped {state_name} resource is ambiguous; use {state_name}[YourState]",
                    )));
                }
                let resource_type = PyResourceType::try_from((type_bound, py))?;
                let resource = if *mutable {
                    resource_type.get_from_world_mut(world, py, validity.clone())?
                } else {
                    resource_type.get_from_world(world, py, validity.clone())?
                };
                // Wrap in Res/ResMut
                if *mutable {
                    let resource_bound = resource.into_bound(py);
                    let resmut_wrapper =
                        Py::new(py, crate::ecs::resource::PyResMut::new(resource_bound))
                            .expect("Failed to create PyResMut");
                    args_buffer.push(resmut_wrapper.into_any());
                } else {
                    let resource_bound = resource.into_bound(py);
                    let res_wrapper = Py::new(py, crate::ecs::resource::PyRes::new(resource_bound))
                        .expect("Failed to create PyRes");
                    args_buffer.push(res_wrapper.into_any());
                }
            }
            SystemParamType::World => {
                let py_world = unsafe { PyWorld::new(world, validity.clone()) };
                let obj = Py::new(py, py_world).expect("Failed to create PyWorld");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::Assets {
                type_ptr,
                wrapper_class,
                logical_type_id,
                logical_type_name,
                mutable,
            } => {
                let borrow_counter = unsafe {
                    asset_borrow_counter_from_cell(
                        world.as_unsafe_world_cell(),
                        type_ptr.0,
                        validity,
                        "observer",
                    )
                }?;
                // SAFETY: observer dispatch holds an exclusive `&mut World`; the cell
                // derived from it is fenced by `validity`.
                let py_assets = unsafe {
                    PyAssets::new(
                        type_ptr.0,
                        wrapper_class.map(|w| w.0),
                        *logical_type_id,
                        logical_type_name.clone(),
                        world.as_unsafe_world_cell(),
                        validity.clone(),
                        *mutable,
                        borrow_counter,
                    )
                };
                let obj = Py::new(py, resource_initializer(py_assets))
                    .expect("Failed to create PyAssets");
                if *mutable {
                    let assets_any: Bound<'_, PyAny> = obj.into_bound(py).into_any();
                    let mut_wrapper =
                        Py::new(py, PyMut::new(assets_any)).expect("Failed to create PyMut");
                    args_buffer.push(mut_wrapper.into_any());
                } else {
                    args_buffer.push(obj.into_any());
                }
            }
            SystemParamType::Local(local) => {
                args_buffer.push(local.clone_ref(py));
            }
            SystemParamType::View { param } => {
                // Observer dispatch owns an exclusive World, so it may build an
                // ephemeral cache for this invocation without a scheduled
                // scheduled system. The injected View owns the Arc after this arm.
                // SAFETY: exclusive observer World access covers the complete
                // cache/runtime lifetime; no parallel scheduler access is needed.
                let cached =
                    unsafe { CachedPyView::build(world, param, custom_component_ids.as_ref(), py) }
                        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
                let this_run = world.change_tick();
                // SAFETY: the cache and cell name this exclusive live World,
                // while validity remains active through the observer call.
                let py_view = unsafe {
                    PyView::new_cached(
                        cached,
                        Tick::new(0),
                        this_run,
                        world.as_unsafe_world_cell(),
                        validity.clone(),
                    )
                }
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
                let obj = Py::new(py, py_view).expect("Failed to create PyView");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::MessageWriter { message_type } => {
                if let MessageType::Custom(py_type) = message_type {
                    let resolved = resolve_from_world(world, py_type.bind(py).as_type_ptr())?;
                    let py_writer = PyMessageWriter::python(
                        message_type.clone(),
                        resolved,
                        validity.clone(),
                        parity_trace.clone(),
                    );
                    let obj = Py::new(py, py_writer).expect("Failed to create PyMessageWriter");
                    args_buffer.push(obj.into_any());
                    continue;
                }
                // SAFETY: observer dispatch holds an exclusive `&mut World`; the cell
                // derived from it is fenced by `validity`.
                let mw =
                    unsafe { MessageWorld::new(world.as_unsafe_world_cell(), validity.clone()) };
                let py_writer =
                    PyMessageWriter::native(message_type.clone(), mw, parity_trace.clone());
                let obj = Py::new(py, py_writer).expect("Failed to create PyMessageWriter");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::MessageReader { message_type } => {
                let cursor = message_cursor_storage.get(message_reader_idx).cloned();
                message_reader_idx += 1;
                if let MessageType::Custom(py_type) = message_type {
                    let resolved = resolve_from_world(world, py_type.bind(py).as_type_ptr())?;
                    let cursor = python_message_cursor(cursor)?;
                    let py_reader = PyMessageReader::python(resolved, cursor, validity.clone());
                    let obj = Py::new(py, py_reader).expect("Failed to create PyMessageReader");
                    args_buffer.push(obj.into_any());
                    continue;
                }
                // SAFETY: observer dispatch holds an exclusive `&mut World`; the cells
                // derived from it are fenced by `validity`.
                let mw_1 =
                    unsafe { MessageWorld::new(world.as_unsafe_world_cell(), validity.clone()) };
                let py_messages = PyMessages {
                    message_type: message_type.clone(),
                    world: mw_1,
                    cursor_storage: cursor,
                };
                let py_reader = PyMessageReader::native(py_messages);
                let obj = Py::new(py, py_reader).expect("Failed to create PyMessageReader");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::MessageMutator { message_type } => {
                let cursor = message_cursor_storage.get(message_reader_idx).cloned();
                message_reader_idx += 1;
                let MessageType::Custom(py_type) = message_type else {
                    return Err(PyTypeError::new_err(
                        "MessageMutator currently supports custom Python messages only",
                    ));
                };
                let resolved = resolve_from_world(world, py_type.bind(py).as_type_ptr())?;
                let cursor = python_message_cursor(cursor)?;
                let py_mutator = PyMessageMutator::python(
                    message_type.clone(),
                    resolved,
                    cursor,
                    validity.clone(),
                    parity_trace.clone(),
                );
                let obj = Py::new(py, py_mutator).expect("Failed to create PyMessageMutator");
                args_buffer.push(obj.into_any());
            }
        }
    }

    // Call the Python function
    if args_buffer.is_empty() {
        callable.call0()?;
    } else {
        let tuple = PyTuple::new(py, &args_buffer).expect("Failed to create PyTuple");
        callable.call1(tuple)?;
    }

    Ok(())
}
