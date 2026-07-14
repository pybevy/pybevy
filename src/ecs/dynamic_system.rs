use std::{
    collections::HashMap,
    env,
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{
        change_detection::{CheckChangeTicks, Tick},
        component::ComponentId,
        query::FilteredAccessSet,
        system::{RunSystemError, System, SystemIn, SystemParamValidationError, SystemStateFlags},
        world::{CommandQueue, DeferredWorld, World, unsafe_world_cell::UnsafeWorldCell},
    },
    prelude::*,
};
use pybevy_core::{AssetBorrowCounter, registry::global_registry};
#[cfg(debug_assertions)]
use pybevy_ecs::shared::access_audit::assert_query_access_declared;
use pybevy_ecs::shared::{
    access_validation::{self as shared_validation},
    command_queue_helpers::create_commands_from_queue,
    param_spec::{
        BackendKeys, ComponentSpec, FilterSpec, KeyResolver, ParamSpec, QuerySpec, ResolvedMessage,
        ResolvedResource, build_declared_access, conflict_error_message, to_param_accesses,
    },
    run_scaffold::{RunScaffoldResult, run_scaffold},
};
use pybevy_reload::{HotReloadGeneration, SystemProfiler, SystemStage};
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError},
    ffi::{PyObject, PyTypeObject},
    prelude::*,
    types::{PyTuple, PyType},
};
use smallvec::SmallVec;

use crate::{
    assets::assets::PyAssets,
    ecs::{
        commands::PyCommands,
        component_type::{PyComponentType, register_component_id, register_custom_component},
        filter::QueryFilter,
        helpers::{
            type_utils::get_python_type_name,
            validity_guard::{ValidityFlag, ValidityGuard},
        },
        message::{PyMessageReader, PyMessageWriter},
        messages::{CursorStorage, MessageRegistry, MessageType, MessageWorld, PyMessages},
        mutable::PyMut,
        observer::PyOn,
        query::{
            query_param::QueryData,
            query_runtime::{CachedQuery, PyQueryIter},
            single_runtime::PySingleQuery,
        },
        resource::PyResource,
        resource_type::{PyResourceStorage, PyResourceType, ResourceRegistry},
        system::{AssetTypePtr, SystemFunction, SystemParam, SystemParamType},
        view::{cached_view::CachedPyView, view::PyView, view_param::ViewParamType},
        world::PyWorld,
    },
};

/// Check if verbose debug output is enabled via environment variable
fn is_verbose() -> bool {
    env::var("PYBEVY_VERBOSE")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// Helper to lock a mutex, recovering from poison if a thread panicked while holding it.
fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| {
        bevy::log::warn!("Recovered from poisoned DynamicSystemInner mutex");
        poisoned.into_inner()
    })
}

/// Handle to a DynamicSystem's Python-holding inner state.
/// Used by DynamicSystemRegistry to release Python references from old-generation systems.
pub(crate) type DynamicSystemHandle = Arc<Mutex<DynamicSystemInner>>;

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

impl DynamicSystemInner {
    /// Release all Python references held by this system.
    /// Called when the generation is no longer needed.
    ///
    /// SAFETY: Caller MUST hold the GIL (via Python::attach) before calling.
    /// This ensures consistent lock ordering: GIL → per-system Mutex, matching
    /// the order used by DynamicSystem::run_unsafe. The previous implementation
    /// acquired the GIL inside gut() (Mutex → GIL), which was an inversion.
    pub(crate) fn gut(&mut self) {
        if self.gutted {
            return;
        }
        self.system_func = None;
        self.cached_func = None;
        self.message_cursor_storage.clear();
        self.gutted = true;
    }
}

/// Clear the system parameter cache.
/// Called when apps are dropped to prevent stale cache entries.
pub fn clear_system_param_cache() {
    SystemFunction::clear_cache();
}

pub struct DynamicSystem {
    resources_to_read: Vec<ComponentId>,
    resources_to_write: Vec<ComponentId>,
    /// Maps custom component type pointers to their registered ComponentIds
    custom_component_ids: HashMap<*const PyTypeObject, ComponentId>,
    /// Shared inner state holding Python references - can be gutted externally
    inner: DynamicSystemHandle,
    func_name: String,
    last_run: Option<Tick>,
    /// Pre-computed flags for optimization
    needs_commands: bool,
    /// Reusable buffer for system arguments - avoids allocations on every call
    /// Most systems have 1-8 parameters, so SmallVec[8] keeps them on stack
    args_buffer: SmallVec<[Py<PyAny>; 8]>,
    /// Shared error state for collecting system errors (parameter + execution)
    error_state: Arc<Mutex<Vec<PyErr>>>,
    /// Shared slot for the last error's message/traceback. Written on the Python
    /// exception path in place of a structural `LastSystemError` world insert; the
    /// `Last`-schedule drain system moves it into the resource.
    error_buffer: SystemErrorBuffer,
    /// Hot reload support: module and function names for dynamic lookup
    module_name: String,
    function_name: String,
    /// Expected generation this system was created for (for debugging)
    expected_generation: u32,
    /// Stage where this system runs (for profiler)
    stage: SystemStage,
    /// Per-system command queue for thread-safe Commands support.
    /// Each system gets its own queue so parallel systems don't need &mut World.
    /// The queue is flushed to the world via queue_deferred() after each run.
    command_queue: CommandQueue,
    /// Throttle repeated errors: last error message and when it was printed
    last_error_msg: Option<String>,
    last_error_print_time: Option<std::time::Instant>,
    suppressed_error_count: u32,
    /// Parameter-conflict error precomputed in `initialize` (`None` = no conflict)
    precomputed_validation: Option<SystemParamValidationError>,
    /// One cached QueryState per Query parameter, built once in `initialize` and
    /// reused across every run. Stored here (not inside the Arc<Mutex<inner>>) so it
    /// lives as long as the DynamicSystem the schedule owns; `PyQueryIter` borrows
    /// entries via raw pointer fenced by the per-run ValidityFlag. Indexed in the
    /// order Query parameters appear in the system signature. Never touched by
    /// `gut()`, which only releases Python refs held in the inner state.
    query_caches: Vec<CachedQuery>,
    /// One initialization-time View cache per View parameter, in signature
    /// order. Resolution failures are retained and surfaced during argument
    /// preparation without rebuilding or touching the World on the run path.
    view_caches: Vec<Result<Arc<CachedPyView>, Arc<str>>>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
// and DynamicSystem is only used within the context of a running Bevy app with Python active
unsafe impl Send for DynamicSystem {}
unsafe impl Sync for DynamicSystem {}

/// Backend key types for the pyo3 backend (see `pybevy_ecs::shared::param_spec`).
pub(crate) struct MainKeys;

impl BackendKeys for MainKeys {
    type ComponentKey = PyComponentType;
    type ResourceKey = Py<PyType>;
    type AssetKey = AssetTypePtr;
    type MessageKey = MessageType;
}

fn component_spec(
    comp_type: &PyComponentType,
    mutable: bool,
    optional: bool,
) -> ComponentSpec<MainKeys> {
    let name = comp_type.to_string();
    ComponentSpec {
        key: comp_type.clone(),
        label: name.clone(),
        name,
        mutable,
        optional,
    }
}

fn filter_spec(comp_type: &PyComponentType) -> FilterSpec<MainKeys> {
    FilterSpec {
        key: comp_type.clone(),
        label: comp_type.to_string(),
    }
}

/// Lower one parsed parameter into the shared backend-neutral IR.
///
/// Component names and disjointness labels are class-name strings (see
/// `PyComponentType`'s `Display`); resource validation keys are type-object
/// pointers. Both match the pre-IR validation semantics.
pub(crate) fn lower_param_type(ty: &SystemParamType) -> ParamSpec<MainKeys> {
    match ty {
        SystemParamType::Query { param: query_param } => {
            let mut spec = QuerySpec::default();
            for data in &query_param.data {
                if let QueryData::Component {
                    ty: comp_type,
                    mutable,
                    optional,
                } = data
                {
                    spec.components
                        .push(component_spec(comp_type, *mutable, *optional));
                }
            }
            for filter in &query_param.filters {
                match filter {
                    QueryFilter::With(with) => {
                        for comp_type in &with.values {
                            spec.with.push(filter_spec(comp_type));
                        }
                    }
                    QueryFilter::Without(without) => {
                        for comp_type in &without.values {
                            spec.without.push(filter_spec(comp_type));
                        }
                    }
                    QueryFilter::Changed(changed) => {
                        spec.changed.push(filter_spec(&changed.component_type));
                    }
                    QueryFilter::Added(added) => {
                        spec.added.push(filter_spec(&added.component_type));
                    }
                    QueryFilter::Has(has) => {
                        spec.resolve_only.push(has.component_type.clone());
                    }
                    QueryFilter::AnyOf(any_of) => {
                        for comp_type in &any_of.values {
                            spec.resolve_only.push(comp_type.clone());
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
                    .push(component_spec(comp_type, *mutable, false));
            }
            for comp_type in &view_param.with_filters {
                spec.with.push(filter_spec(comp_type));
            }
            for comp_type in &view_param.without_filters {
                spec.without.push(filter_spec(comp_type));
            }
            for comp_type in &view_param.changed_filters {
                spec.changed.push(filter_spec(comp_type));
            }
            for comp_type in &view_param.added_filters {
                spec.added.push(filter_spec(comp_type));
            }
            ParamSpec::View(spec)
        }
        SystemParamType::Resource { type_obj, mutable } => {
            let type_ptr = type_obj.as_ptr() as usize;
            ParamSpec::Res {
                key: Python::attach(|py| type_obj.clone_ref(py)),
                vkey: Some(type_ptr),
                name: format!("Resource@{:x}", type_ptr),
                mutable: *mutable,
            }
        }
        SystemParamType::Assets {
            type_ptr,
            wrapper_class,
            mutable,
        } => {
            // Access is declared under the real asset type: `type_ptr` (e.g.
            // ShaderMaterial) owns the registered AssetBridge, so `asset_id`
            // resolves it to the Assets<T> ComponentId. A `@material` wrapper
            // (e.g. GroundMaterial) has no bridge, so keying access on the
            // wrapper would declare no Assets<T> access at all while the system
            // still reads/writes the underlying collection -- a data race
            // versus native asset systems. The conflict key keeps using
            // wrapper_class (falling back to type_ptr) so distinct materials
            // backed by the same underlying type stay distinct collections.
            let vkey_ptr = wrapper_class.unwrap_or(*type_ptr);
            let name = format!("{:p}", vkey_ptr.0);
            ParamSpec::Assets {
                key: *type_ptr,
                vkey: name.clone(),
                name,
                mutable: *mutable,
            }
        }
        SystemParamType::World => ParamSpec::World,
        SystemParamType::Commands => ParamSpec::Commands,
        SystemParamType::Local(_) => ParamSpec::Local,
        SystemParamType::MessageWriter { message_type } => ParamSpec::MessageWriter {
            key: message_type.clone(),
        },
        SystemParamType::MessageReader { message_type } => ParamSpec::MessageReader {
            key: message_type.clone(),
        },
        SystemParamType::On { .. } => ParamSpec::Observer,
    }
}

/// Lower a whole signature into the shared IR, in signature order.
pub(crate) fn lower_params(params: &[SystemParam]) -> Vec<ParamSpec<MainKeys>> {
    params.iter().map(|p| lower_param_type(&p.ty)).collect()
}

/// Validate a function's parameters for conflicting ECS access: double mutable
/// access, mixing mutable and immutable, or a `World` parameter alongside
/// anything else. Regular systems run this via
/// [`DynamicSystem::validate_parameters`]; observer registration calls it
/// directly, since observers skip the `add_systems` gate.
pub(crate) fn validate_system_params(params: &[SystemParam], func_name: &str) -> PyResult<()> {
    let specs = lower_params(params);
    let accesses = to_param_accesses(&specs, |comp_type| comp_type.to_string());
    shared_validation::validate_access(&accesses)
        .map_err(|conflict| PyRuntimeError::new_err(conflict_error_message(func_name, &conflict)))
}

/// Resolves pyo3 backend keys against the world during the shared access walk.
///
/// Borrows the system's custom-component cache so custom components are
/// registered on first sight and reused by the runtime (`PyQueryIter::new`
/// reads this cache).
struct MainResolver<'a> {
    custom_component_ids: &'a mut HashMap<*const PyTypeObject, ComponentId>,
}

impl KeyResolver<MainKeys> for MainResolver<'_> {
    fn component_id(&mut self, world: &mut World, key: &PyComponentType) -> Option<ComponentId> {
        Some(match key {
            PyComponentType::Custom(type_ptr) => {
                if let Some(&id) = self.custom_component_ids.get(type_ptr) {
                    id
                } else {
                    let name = Python::attach(|py| get_python_type_name(py, *type_ptr));
                    let id = register_custom_component(world, *type_ptr, name);
                    self.custom_component_ids.insert(*type_ptr, id);
                    id
                }
            }
            _ => register_component_id(world, key, self.custom_component_ids),
        })
    }

    fn resource_ids(&mut self, world: &mut World, key: &Py<PyType>) -> ResolvedResource {
        let resource_type = Python::attach(|py| {
            let type_bound = key.bind(py);
            PyResourceType::try_from((type_bound, py)).ok()
        });
        let Some(rt) = resource_type else {
            return ResolvedResource::default();
        };
        // Register (not just look up) so access is declared even when the
        // resource is inserted after schedule init; an undeclared access
        // would let conflicting systems race (UB).
        let primary = rt.register_component_id(world);
        let mut aux_reads = Vec::new();
        // Custom Python resources live inside `PyResourceStorage`, keyed
        // through `ResourceRegistry`; `run_unsafe`'s cell read path touches
        // both (see PyResourceType::get_custom_from_cell), so declare those
        // reads to keep declared >= actual access. Both read and write params
        // only READ these two resources (the write path returns the same
        // stored Python object).
        if matches!(rt, PyResourceType::Custom(_)) {
            aux_reads.push(world.register_component::<ResourceRegistry>());
            aux_reads.push(world.register_component::<PyResourceStorage>());
        }
        ResolvedResource { primary, aux_reads }
    }

    fn asset_id(&mut self, world: &mut World, key: &AssetTypePtr) -> Option<ComponentId> {
        // Register the Assets<T> id up front; a bridge that exists always
        // yields an id, so access is never silently dropped.
        global_registry::get_asset_bridge_by_py_type(key.0)
            .map(|bridge| bridge.register_resource_id(world))
    }

    fn message_ids(
        &mut self,
        world: &mut World,
        key: &MessageType,
        write: bool,
    ) -> ResolvedMessage {
        let mut resolved = ResolvedMessage::default();
        if write {
            // Message buffers are resources; register (not just look up) the
            // id so access is declared even before the first write. Custom
            // messages are the one residual lookup-only case (see
            // MessageType::register_resource_id).
            if let Some(id) = key.register_resource_id(world) {
                resolved.writes.push(id);
            }
        } else {
            // reader_resource_ids returns the primary Messages<T> id plus any
            // auxiliary read (KeyboardInput also reads ButtonInput<KeyCode>).
            resolved.reads.extend(key.reader_resource_ids(world));
        }
        // Custom message paths resolve their message number through the
        // MessageRegistry resource at run time; declare that read so declared
        // access covers the actual access.
        if matches!(key, MessageType::Custom(_)) {
            resolved
                .reads
                .push(world.register_component::<MessageRegistry>());
        }
        resolved
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
}

impl DynamicSystem {
    pub(crate) fn new(
        func: Py<PyAny>,
        generation: u32,
        error_state: Arc<Mutex<Vec<PyErr>>>,
        error_buffer: SystemErrorBuffer,
        stage: SystemStage,
    ) -> PyResult<Self> {
        let (system_func, func_name, module_name, function_name) = Python::attach(|py| {
            let func_bound = func.bind(py);
            let name = func_bound
                .getattr("__name__")
                .ok()
                .and_then(|n| n.extract::<String>().ok())
                .unwrap_or_else(|| "DynamicSystem".to_string());

            // Extract module name for hot reload support
            let mut module = func_bound
                .getattr("__module__")
                .ok()
                .and_then(|m| m.extract::<String>().ok())
                .unwrap_or_else(|| "__main__".to_string());

            // Handle special case: <run_path> is used when running scripts directly
            // We need to use __main__ instead since <run_path> can't be imported
            if module == "<run_path>" {
                module = "__main__".to_string();
            }

            // Debug: Print function ID to track if we're getting new or old functions
            if is_verbose() {
                eprintln!(
                    "🔍 Creating DynamicSystem for {}.{} (id: {:?}, gen: {})",
                    module,
                    name,
                    func_bound.as_ptr(),
                    generation
                );
            }

            let system_func = SystemFunction::new(py, func_bound.clone())?;

            Ok::<_, PyErr>((system_func, name.clone(), module, name))
        })?;

        let needs_commands = system_func
            .params
            .iter()
            .any(|p| matches!(p.ty, SystemParamType::Commands));

        // Allocate one cursor storage slot per MessageReader parameter
        let message_reader_count = system_func
            .params
            .iter()
            .filter(|p| matches!(p.ty, SystemParamType::MessageReader { .. }))
            .count();
        let message_cursor_storage: Vec<_> = (0..message_reader_count)
            .map(|_| Arc::new(Mutex::new(None)))
            .collect();

        let inner = Arc::new(Mutex::new(DynamicSystemInner {
            system_func: Some(system_func),
            cached_func: Some(func),
            cached_generation: generation,
            message_cursor_storage,
            gutted: false,
        }));

        let system = Self {
            resources_to_read: Vec::new(),
            resources_to_write: Vec::new(),
            custom_component_ids: HashMap::new(),
            inner,
            func_name,
            last_run: None,
            needs_commands,
            args_buffer: SmallVec::new(),
            error_state,
            error_buffer,
            module_name,
            function_name,
            expected_generation: generation,
            stage,
            command_queue: CommandQueue::default(),
            last_error_msg: None,
            last_error_print_time: None,
            suppressed_error_count: 0,
            precomputed_validation: None,
            query_caches: Vec::new(),
            view_caches: Vec::new(),
        };

        // Validate parameters immediately to catch conflicts early
        system.validate_parameters()?;

        Ok(system)
    }

    /// Helper to wrap a resource in Res or ResMut based on mutability
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

    /// Get the cached function (for DynamicCondition)
    pub(crate) fn get_cached_function(&self) -> PyResult<Py<PyAny>> {
        let inner = lock_or_recover(&self.inner);
        inner
            .cached_func
            .as_ref()
            .map(|f| Python::attach(|py| f.clone_ref(py)))
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Function not cached"))
    }

    /// Get a handle for external gutting. Call before the system is consumed by Bevy.
    pub(crate) fn handle(&self) -> DynamicSystemHandle {
        Arc::clone(&self.inner)
    }
}

impl DynamicSystem {
    /// Build the Python argument list for one run of a non-observer system.
    ///
    /// The shared run scaffold calls this as its `build_args` hook; the
    /// rustpython backend's `build_system_args` is the counterpart. Fills
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
    unsafe fn build_run_args<'w, 'c1, 'c2>(
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
    ) -> Option<PyErr> {
        let mut message_reader_idx = 0usize;
        let mut query_cache_idx = 0usize;
        let mut view_cache_idx = 0usize;
        for param in params {
            match &param.ty {
                SystemParamType::Local(local) => {
                    args_buffer.push(local.clone_ref(py));
                }
                SystemParamType::Resource { type_obj, mutable } => {
                    // Fetch resource from world using PyResourceType
                    let type_bound = type_obj.bind(py);
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
                        // ResourceRegistry/PyResourceStorage reads for Custom);
                        // the executor prevents a conflicting system from running
                        // concurrently, so the cell's unchecked borrow is unique.
                        match unsafe {
                            resource_type.get_from_cell_mut(world, py, validity.clone())
                        } {
                            Ok(r) => r,
                            Err(_e) => {
                                let type_name = type_bound
                                    .name()
                                    .map(|n| n.to_string())
                                    .unwrap_or_else(|_| "Unknown".to_string());
                                let err_msg = format!(
                                    "System `{}`: resource `{}` not found in world",
                                    func_name, type_name
                                );
                                return Some(PyTypeError::new_err(err_msg));
                            }
                        }
                    } else {
                        // SAFETY: `initialize` declared read access to this
                        // resource; the executor prevents a concurrent writer, so
                        // the cell's unchecked read is unique.
                        match unsafe { resource_type.get_from_cell(world, py, validity.clone()) } {
                            Ok(r) => r,
                            Err(_e) => {
                                let type_name = type_bound
                                    .name()
                                    .map(|n| n.to_string())
                                    .unwrap_or_else(|_| "Unknown".to_string());
                                let err_msg = format!(
                                    "System `{}`: resource `{}` not found in world",
                                    func_name, type_name
                                );
                                return Some(PyTypeError::new_err(err_msg));
                            }
                        }
                    };

                    Self::wrap_resource_in_res(py, resource, *mutable, args_buffer);
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
                        // lives on the DynamicSystem, which outlives the run.
                        let single_query = unsafe {
                            PySingleQuery::new(cached, world, validity.clone(), last_run, this_run)
                        };

                        let obj =
                            Py::new(py, single_query).expect("Failed to create PySingleQuery");
                        args_buffer.push(obj.into_any());
                    } else {
                        // SAFETY: as above — unique access to the cached state's
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
                    let py_commands = unsafe { PyCommands::new(commands, validity.clone()) };
                    let obj = Py::new(py, py_commands).expect("Failed to create PyCommands");
                    args_buffer.push(obj.into_any());
                }
                SystemParamType::MessageWriter { message_type } => {
                    // Create PyMessageWriter with narrow cell-based world access.
                    // SAFETY: `initialize` declares write access for this
                    // writer's Messages<T> id; the writer only reaches that buffer.
                    let mw = unsafe { MessageWorld::new(world, validity.clone()) };
                    let py_writer = PyMessageWriter {
                        message_type: message_type.clone(),
                        world: mw,
                    };
                    let obj = Py::new(py, py_writer).expect("Failed to create PyMessageWriter");
                    args_buffer.push(obj.into_any());
                }
                SystemParamType::MessageReader { message_type } => {
                    // Create PyMessageReader with narrow cell-based world access.
                    // SAFETY: `initialize` declares reads for this reader's
                    // resource ids (Messages<T>, plus ButtonInput<KeyCode> for
                    // KeyboardInput); the reader only reaches those.
                    let mw_1 = unsafe { MessageWorld::new(world, validity.clone()) };
                    let mw_2 = unsafe { MessageWorld::new(world, validity.clone()) };
                    let cursor = message_cursor_storage.get(message_reader_idx).cloned();
                    message_reader_idx += 1;
                    let py_messages = PyMessages {
                        message_type: message_type.clone(),
                        world: mw_1,
                        cursor_storage: cursor,
                    };
                    let py_reader = PyMessageReader {
                        world: mw_2,
                        messages: py_messages,
                    };
                    let obj = Py::new(py, py_reader).expect("Failed to create PyMessageReader");
                    args_buffer.push(obj.into_any());
                }
                SystemParamType::On { .. } => {
                    // On parameters are only valid in observer contexts.
                    // Observer dispatch uses execute_system_func() instead of run_unsafe().
                    unreachable!(
                        "On parameter in non-observer system — observers use execute_system_func()"
                    )
                }
                SystemParamType::Assets {
                    type_ptr,
                    wrapper_class,
                    mutable,
                } => {
                    // Create PyAssets wrapper with cell-based world access.
                    // SAFETY: `world` is this run's UnsafeWorldCell; `initialize`
                    // declares this Assets<T> resource's access, which
                    // bounds the data PyAssets reaches via the AssetBridge.
                    let py_assets = unsafe {
                        PyAssets::new(
                            type_ptr.0,
                            wrapper_class.map(|w| w.0),
                            world,
                            validity.clone(),
                            *mutable,
                            AssetBorrowCounter::default(),
                        )
                    };
                    let obj =
                        Py::new(py, (py_assets, PyResource)).expect("Failed to create PyAssets");

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
}

impl System for DynamicSystem {
    type In = ();
    type Out = ();

    fn name(&self) -> DebugName {
        DebugName::owned(self.func_name.clone())
    }

    fn flags(&self) -> SystemStateFlags {
        let inner = lock_or_recover(&self.inner);
        if inner.gutted {
            return SystemStateFlags::empty();
        }
        let needs_exclusive = inner
            .system_func
            .as_ref()
            .map(|sf| {
                sf.params
                    .iter()
                    .any(|p| matches!(p.ty, SystemParamType::World))
            })
            .unwrap_or(false);

        pybevy_ecs::shared::system_flags::compute_system_flags(needs_exclusive, self.needs_commands)
    }

    unsafe fn run_unsafe(
        &mut self,
        _input: SystemIn<'_, Self>,
        world: UnsafeWorldCell,
    ) -> Result<Self::Out, RunSystemError> {
        self.validate_params()?;

        let last_run = self.get_last_run();
        // The pyo3 backend gates on a concrete generation (0 == ungated); pass it
        // as `Some` with the absent-resource default so the scaffold's guard
        // reproduces the previous `current_gen(default 0) != expected` check.
        let expected_generation = Some(self.expected_generation);
        // SAFETY: read access to HotReloadGeneration is declared in `initialize`.
        let current_generation = Some(
            unsafe { world.get_resource::<HotReloadGeneration>() }
                .map(|res| res.current)
                .unwrap_or(0),
        );

        // The shared scaffold owns the run sequence (tick advance, ValidityFlag +
        // RAII guard, local Commands, body timing, end-of-run tick); this closure
        // is the interpreter body it calls.
        // SAFETY: `world` is this run's valid cell; the ValidityFlag the scaffold
        // creates fences every argument built below for the run's duration.
        let scaffold_result = unsafe {
            run_scaffold(
                world,
                last_run,
                expected_generation,
                current_generation,
                |ctx, validity, queue| {
                    Python::attach(|py| {
                        // Reuse the args buffer - clear it and it keeps its capacity.
                        self.args_buffer.clear();

                        // Build a Bevy `Commands` over the scaffold's local queue for
                        // any `Commands` param. The raw-pointer deref lets the borrow
                        // match `ctx.world`'s lifetime; both are fenced by `validity`.
                        // SAFETY: `queue` is the scaffold's live local queue.
                        let queue_ptr: *mut CommandQueue = queue;
                        let mut commands_storage = if self.needs_commands {
                            Some(create_commands_from_queue(&mut *queue_ptr, ctx.world))
                        } else {
                            None
                        };

                        // No shared `&mut World` is materialized here: each parameter
                        // arm reaches the world through `ctx.world`, so a non-exclusive
                        // system never conjures whole-world access. The one exception is
                        // the World arm, under the EXCLUSIVE-scheduling guarantee.

                        // Lock the inner state for system_func and message_cursor_storage.
                        let mut inner_guard = lock_or_recover(&self.inner);
                        if inner_guard.gutted {
                            return false;
                        }
                        let Some(system_func) = inner_guard.system_func.as_ref() else {
                            eprintln!(
                                "⚠️ System {}.{} has no function — skipping",
                                self.module_name, self.function_name
                            );
                            return false;
                        };

                        // Build the Python argument list via `build_run_args` (the
                        // rustpython backend's `build_system_args` is the counterpart).
                        // SAFETY: `world` is this run's valid cell and `validity` stays
                        // active for the whole attach block.
                        let param_error = {
                            Self::build_run_args(
                                py,
                                &system_func.params,
                                &self.query_caches,
                                &self.view_caches,
                                &inner_guard.message_cursor_storage,
                                &mut commands_storage,
                                &mut self.args_buffer,
                                ctx.world,
                                validity,
                                ctx.ticks.last_run,
                                ctx.ticks.this_run,
                                &self.func_name,
                            )
                        };

                        // Check if there was an error during parameter preparation
                        if let Some(err) = param_error {
                            // Drop the inner guard before acquiring error_state lock
                            drop(inner_guard);
                            // Store the error in shared state
                            let mut error_lock = self.error_state.lock().unwrap();
                            error_lock.push(err);
                            // Don't call the function if there was an error
                        } else {
                            // The generation the scaffold already read for the guard, reused
                            // here to decide whether to re-import the function.
                            let current_generation = ctx.current_generation.unwrap_or(0);

                            // Get the current version of the function (may trigger re-import on generation change)
                            // Inlined from get_function() since we already hold the inner lock
                            let func = {
                                let get_func_result: PyResult<Py<PyAny>> = (|| {
                                    if self.module_name == "__main__" {
                                        if let Some(ref func) = inner_guard.cached_func {
                                            return Ok(func.clone_ref(py));
                                        } else {
                                            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                                                format!(
                                                    "Function {}.{} not cached",
                                                    self.module_name, self.function_name
                                                ),
                                            ));
                                        }
                                    }
                                    if inner_guard.cached_generation != current_generation
                                        || inner_guard.cached_func.is_none()
                                    {
                                        let module =
                                            py.import(self.module_name.as_str())?.into_any();
                                        let func = module.getattr(self.function_name.as_str())?;
                                        inner_guard.cached_func = Some(func.unbind());
                                        inner_guard.cached_generation = current_generation;
                                        if is_verbose() {
                                            eprintln!(
                                                "🔄 Hot reload: Refreshed function {}.{} for generation {}",
                                                self.module_name,
                                                self.function_name,
                                                current_generation
                                            );
                                        }
                                    }
                                    Ok(inner_guard.cached_func.as_ref().unwrap().clone_ref(py))
                                })(
                                );
                                match get_func_result {
                                    Ok(f) => f,
                                    Err(e) => {
                                        if is_verbose() {
                                            eprintln!(
                                                "❌ Error refreshing function {}.{}: {}",
                                                self.module_name, self.function_name, e
                                            );
                                        }
                                        e.print(py);
                                        return false;
                                    }
                                }
                            };

                            // Drop the inner guard before calling the Python function
                            // This ensures gutting can proceed if needed (though unlikely during execution)
                            drop(inner_guard);

                            // Debug: Print which function we're about to call
                            if is_verbose() {
                                eprintln!(
                                    "🎯 Executing {}.{} (id: {:?}, expected_gen: {}, current_gen: {})",
                                    self.module_name,
                                    self.function_name,
                                    func.as_ptr(),
                                    self.expected_generation,
                                    current_generation
                                );
                            }

                            // Call the Python function
                            let call_result = if self.args_buffer.is_empty() {
                                func.bind(py).call0()
                            } else {
                                let tuple = PyTuple::new(py, &self.args_buffer)
                                    .expect("Failed to create PyTuple");
                                func.bind(py).call1(tuple)
                            };

                            if let Err(e) = call_result {
                                let error_str = e.to_string();

                                // Extract full Python traceback with file/line info
                                let traceback_str = e.traceback(py).map(|tb| {
                                    tb.format()
                                        .unwrap_or_else(|_| "(traceback format failed)".into())
                                });

                                // Buffer the error for MCP get_last_error without touching the
                                // world. A `Last`-schedule drain moves this into `LastSystemError`,
                                // keeping the parallel error path free of structural world mutation.
                                {
                                    let mut buffered = lock_or_recover(&self.error_buffer);
                                    *buffered = Some(BufferedSystemError {
                                        error: error_str.clone(),
                                        traceback: traceback_str,
                                    });
                                }

                                // When hot reload is NOT active, store the error for propagation
                                // to the caller (app.update() / app.run()). This makes assert/raise
                                // in systems actually fail tests and standalone scripts.
                                let hot_reload_active = {
                                    let wr = ctx.world.world();
                                    wr.get_resource::<HotReloadGeneration>().is_some()
                                };
                                if !hot_reload_active {
                                    let mut error_lock = self.error_state.lock().unwrap();
                                    error_lock.push(e.clone_ref(py));
                                }

                                // Throttle repeated stderr prints: same error → print once then
                                // suppress for 5 seconds, showing a count summary when resumed
                                let now = std::time::Instant::now();
                                let is_repeat = self.last_error_msg.as_ref() == Some(&error_str);

                                if is_repeat {
                                    let elapsed = self
                                        .last_error_print_time
                                        .map(|t| now.duration_since(t).as_secs_f32())
                                        .unwrap_or(f32::MAX);

                                    if elapsed < 5.0 {
                                        self.suppressed_error_count += 1;
                                    } else {
                                        // Time to print again
                                        if self.suppressed_error_count > 0 {
                                            eprintln!(
                                                "  ... (repeated {} more times)",
                                                self.suppressed_error_count
                                            );
                                        }
                                        e.print(py);
                                        self.last_error_print_time = Some(now);
                                        self.suppressed_error_count = 0;
                                    }
                                } else {
                                    // New/different error — always print
                                    if self.suppressed_error_count > 0 {
                                        eprintln!(
                                            "  ... (previous error repeated {} more times)",
                                            self.suppressed_error_count
                                        );
                                    }
                                    e.print(py);
                                    self.last_error_msg = Some(error_str);
                                    self.last_error_print_time = Some(now);
                                    self.suppressed_error_count = 0;
                                }
                            }
                        }

                        // Systems ignore the boolean; run conditions interpret it.
                        false
                    })
                },
            )
        };

        match scaffold_result {
            RunScaffoldResult::SkippedStaleGeneration => {}
            RunScaffoldResult::Ran {
                duration,
                end_tick,
                mut local_queue,
                ..
            } => {
                // Append queued commands to the per-system queue for deferred application.
                self.command_queue.append(&mut local_queue);

                // Profiler epilogue: record this run's duration if a profiler is present.
                // Shared with the rustpython backend so the profiler + Time read cannot
                // drift. SAFETY: `world` is valid; the SystemProfiler read is declared.
                let world_ref = unsafe { world.world() };
                SystemProfiler::record_run(world_ref, &self.func_name, duration, self.stage);

                // last_run is the scaffold's end_tick: the change tick read AFTER the
                // run's writes, not this_run. pybevy stamps live ticks, so a
                // start-of-run value would make the system re-detect its own writes.
                self.last_run = Some(end_tick);
            }
        }

        Ok(())
    }

    fn apply_deferred(&mut self, world: &mut World) {
        self.command_queue.apply(world);
    }

    fn queue_deferred(&mut self, _world: DeferredWorld) {}

    fn initialize(&mut self, world: &mut World) -> FilteredAccessSet {
        // Clone the params from inner so we can release the lock before mutating self fields.
        // Params contain Arc/Py refs so cloning is cheap (refcount bumps).
        let params = {
            let inner = lock_or_recover(&self.inner);
            if inner.gutted {
                return FilteredAccessSet::default();
            }
            inner.system_func.as_ref().unwrap().params.clone()
        };

        // The shared walk declares one `FilteredAccess` per Query/View
        // parameter plus a single resource-like access, appends the
        // infrastructure reads, and returns the empty set for exclusive
        // systems (see `build_declared_access` for the full rationale).
        // Component ids resolve through `MainResolver`, which registers
        // custom components into the same cache the runtime reads.
        let specs = lower_params(&params);
        let declared = {
            let mut resolver = MainResolver {
                custom_component_ids: &mut self.custom_component_ids,
            };
            build_declared_access(world, &specs, &mut resolver)
        };
        self.resources_to_read = declared.resources_to_read;
        self.resources_to_write = declared.resources_to_write;

        // Build (or rebuild) the per-Query-parameter cached QueryState now that all
        // component ids are resolved. `initialize` legitimately holds `&mut World`, so
        // this heavy work (id registration, QueryState construction, extraction/access
        // arrays) happens once per parameter instead of on every run. The snapshot of
        // custom_component_ids is complete because the loop above already registered
        // every custom component this system's queries reference. Iterating `params`
        // in signature order keeps `query_caches` aligned with `run_unsafe`'s counter.
        let cc_snapshot = Arc::new(self.custom_component_ids.clone());
        let mut query_caches = Vec::new();
        for param in &params {
            if let SystemParamType::Query { param: query_param } = &param.ty {
                query_caches.push(CachedQuery::build(
                    world,
                    query_param.clone(),
                    cc_snapshot.clone(),
                ));
            }
        }
        self.query_caches = query_caches;

        // Resolve the matching neutral View cache once per View parameter.
        // The shared access walk above and this build both consume the same
        // parsed descriptor and custom-component ID map, keeping scheduler
        // declaration and raw runtime metadata in lockstep.
        let mut view_caches = Vec::new();
        for param in &params {
            if let SystemParamType::View { param: view_param } = &param.ty {
                // SAFETY: `view_param` is the exact descriptor lowered into
                // `specs` above. `initialize` holds exclusive World access, and
                // the resulting cache is used only with this system's declared
                // scheduler access.
                let cached =
                    unsafe { CachedPyView::build(world, view_param, &self.custom_component_ids) }
                        .map_err(|error| {
                            Arc::<str>::from(format!(
                                "System `{}` View parameter `{}` could not be initialized: {error}",
                                self.func_name, param.name
                            ))
                        });
                view_caches.push(cached);
            }
        }
        self.view_caches = view_caches;

        // Conflict validation needs `&mut World` for ComponentId lookups, so it
        // runs here; run_unsafe only reads the stored result.
        let accesses = to_param_accesses(&specs, |comp_type| {
            self.get_component_id_for_validation(world, comp_type)
        });
        self.precomputed_validation =
            shared_validation::validate_access(&accesses)
                .err()
                .map(|conflict| {
                    // skipped=false: a conflict is an error and must reach the app's
                    // error handler; skipped errors are dropped silently by executors.
                    SystemParamValidationError::new::<Self>(
                        false,
                        conflict_error_message(&self.func_name, &conflict),
                        format!("parameter_{}", conflict.param_idx),
                    )
                });

        // Debug-only: verify the declared scheduler access covers what each
        // query's QueryState actually touches. `build_declared_access` derives
        // the declaration from the ParamSpec via `QueryParamAccess`; the
        // QueryState is built from the same spec down a separate path, so a
        // divergence here is a real under-declaration that would let the
        // multithreaded executor run a conflicting system in parallel and race.
        // Catch it loudly at initialize instead of silently at runtime.
        #[cfg(debug_assertions)]
        {
            for (i, cached) in self.query_caches.iter().enumerate() {
                assert_query_access_declared(
                    &self.func_name,
                    i,
                    &declared.set,
                    &cached.component_access(),
                );
            }
        }

        declared.set
    }

    fn check_change_tick(&mut self, check: CheckChangeTicks) {
        // Clamp our stored last_run so it never ages past the change-detection
        // window, mirroring FunctionSystem's check_system_change_tick. Bevy's
        // QueryState exposes no tick-check method (it caches archetype/access data,
        // not change ticks), so clamping last_run is the whole job; every cached
        // query reads this last_run at the start of each run.
        if let Some(last_run) = self.last_run.as_mut() {
            last_run.check_tick(check);
        }
    }

    fn get_last_run(&self) -> Tick {
        self.last_run.unwrap_or(Tick::new(0))
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.last_run = Some(last_run);
    }
}

impl DynamicSystem {
    /// Return the parameter-conflict error precomputed in `initialize`, if any.
    fn validate_params(&self) -> Result<(), SystemParamValidationError> {
        match &self.precomputed_validation {
            Some(err) => Err(err.clone()),
            None => Ok(()),
        }
    }

    /// Validate system parameters for conflicting component access.
    /// This should be called immediately after creating a DynamicSystem to catch errors early.
    pub(crate) fn validate_parameters(&self) -> PyResult<()> {
        let params = {
            let inner = lock_or_recover(&self.inner);
            if inner.gutted {
                return Ok(());
            }
            inner.system_func.as_ref().unwrap().params.clone()
        };
        validate_system_params(&params, &self.func_name)
    }

    /// Get the ComponentId for a given component type during validation.
    /// This uses the already-registered IDs from initialize() or looks up custom components.
    fn get_component_id_for_validation(
        &self,
        world_ref: &mut World,
        comp_type: &PyComponentType,
    ) -> ComponentId {
        match comp_type {
            PyComponentType::Custom(type_ptr) => {
                // Look up the already-registered custom component ID
                if let Some(&id) = self.custom_component_ids.get(type_ptr) {
                    id
                } else {
                    // This shouldn't happen if initialize() ran first, but handle it gracefully
                    // Register it now (this will return the existing ID if already registered)
                    let name = Python::attach(|py| {
                        // SAFETY: registered type pointers live for the interpreter lifetime
                        let type_obj =
                            unsafe { Bound::from_borrowed_ptr(py, *type_ptr as *mut PyObject) };
                        let type_bound = type_obj.cast::<PyType>()?;
                        Ok::<String, PyErr>(type_bound.name()?.to_string())
                    })
                    .unwrap_or_else(|_| "Unknown".to_string());

                    register_custom_component(world_ref, *type_ptr, name)
                }
            }
            // For built-in components, use the generated register_simple method
            _ => comp_type.register_simple(world_ref),
        }
    }
}

/// Execute a system function with full parameter injection.
///
/// This is used by the observer dispatch to inject all system parameters
/// (Commands, Query, Res, etc.) alongside the On trigger parameter.
///
/// # Arguments
/// * `py` - Python GIL token
/// Get or create a synthetic ComponentId for an asset type, keyed by Python type pointer.
/// Used to register `Res[Assets[T]]`/`ResMut[Assets[T]]` access in FilteredAccessSet
/// so Bevy's scheduler prevents cross-system data races on the same asset type.
/// * `system_func` - The parsed system function with parameters
/// * `world` - Mutable world reference
/// * `on_param` - The On trigger parameter (required for observer dispatch)
pub(crate) fn execute_system_func(
    py: Python,
    system_func: &SystemFunction,
    world: &mut World,
    on_param: Py<PyOn>,
) -> PyResult<()> {
    // Transient per-Query cached states for this observer dispatch. Observers have no
    // DynamicSystem cache (they run outside the schedule), but they hold an exclusive
    // `&mut World`, so building a CachedQuery here is sound. Boxed so each state has a
    // stable heap address while a PyQueryIter holds a raw pointer to it. Declared before
    // the validity guard so it drops LAST: the guard invalidates the shared flag before
    // these caches are freed, so any leaked PyQueryIter sees "invalid" before use.
    let mut transient_caches: Vec<Box<CachedQuery>> = Vec::new();

    let validity = ValidityFlag::new();
    let _validity_guard = ValidityGuard::new(validity.clone());

    let mut args_buffer: Vec<Py<PyAny>> = Vec::with_capacity(system_func.params.len());

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

    for param in &system_func.params {
        match &param.ty {
            SystemParamType::On { .. } => {
                args_buffer.push(on_param.clone_ref(py).into_any());
            }
            SystemParamType::Commands => {
                let py_commands = unsafe {
                    PyCommands::from_world_temporary(world as *mut World, validity.clone())
                };
                let obj = Py::new(py, py_commands).expect("Failed to create PyCommands");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::Query { param: query_param } => {
                // Build a transient cached state; the exclusive &mut World makes this
                // sound in the observer context (no parallel systems here).
                transient_caches.push(Box::new(CachedQuery::build(
                    world,
                    query_param.clone(),
                    custom_component_ids.clone(),
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
                mutable,
            } => {
                // SAFETY: observer dispatch holds an exclusive `&mut World`; the cell
                // derived from it is fenced by `validity`.
                let py_assets = unsafe {
                    PyAssets::new(
                        type_ptr.0,
                        wrapper_class.map(|w| w.0),
                        world.as_unsafe_world_cell(),
                        validity.clone(),
                        *mutable,
                        AssetBorrowCounter::default(),
                    )
                };
                let obj = Py::new(py, (py_assets, PyResource)).expect("Failed to create PyAssets");
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
                // DynamicSystem. The injected View owns the Arc after this arm.
                // SAFETY: exclusive observer World access covers the complete
                // cache/runtime lifetime; no parallel scheduler access is needed.
                let cached =
                    unsafe { CachedPyView::build(world, param, custom_component_ids.as_ref()) }
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
                // SAFETY: observer dispatch holds an exclusive `&mut World`; the cell
                // derived from it is fenced by `validity`.
                let mw =
                    unsafe { MessageWorld::new(world.as_unsafe_world_cell(), validity.clone()) };
                let py_writer = PyMessageWriter {
                    message_type: message_type.clone(),
                    world: mw,
                };
                let obj = Py::new(py, py_writer).expect("Failed to create PyMessageWriter");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::MessageReader { message_type } => {
                // SAFETY: observer dispatch holds an exclusive `&mut World`; the cells
                // derived from it are fenced by `validity`.
                let mw_1 =
                    unsafe { MessageWorld::new(world.as_unsafe_world_cell(), validity.clone()) };
                let mw_2 =
                    unsafe { MessageWorld::new(world.as_unsafe_world_cell(), validity.clone()) };
                let py_messages = PyMessages {
                    message_type: message_type.clone(),
                    world: mw_1,
                    cursor_storage: None,
                };
                let py_reader = PyMessageReader {
                    world: mw_2,
                    messages: py_messages,
                };
                let obj = Py::new(py, py_reader).expect("Failed to create PyMessageReader");
                args_buffer.push(obj.into_any());
            }
        }
    }

    // Call the Python function
    if args_buffer.is_empty() {
        system_func.func.bind(py).call0()?;
    } else {
        let tuple = PyTuple::new(py, &args_buffer).expect("Failed to create PyTuple");
        system_func.func.bind(py).call1(tuple)?;
    }

    Ok(())
}
