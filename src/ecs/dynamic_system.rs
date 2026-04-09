use std::{
    collections::{HashMap, HashSet},
    env,
    sync::{Arc, Mutex},
    time::Instant,
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
use pybevy_ecs::shared::access_validation::{
    self as shared_validation, ComponentAccess, ParamAccess, QueryFilters,
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
    assets::{asset_server::PyAssetServer, assets::PyAssets},
    ecs::{
        commands::PyCommands,
        component_type::{PyComponentType, register_component_id, register_custom_component},
        filter::QueryFilter,
        helpers::{
            type_utils::get_python_type_name,
            validity_guard::{ValidityFlag, ValidityGuard},
        },
        message::{PyMessageReader, PyMessageWriter},
        messages::PyMessages,
        mutable::PyMut,
        observer::PyOn,
        query::{
            query_param::QueryData, query_runtime::PyQueryIter, single_runtime::PySingleQuery,
        },
        resource::PyResource,
        resource_type::PyResourceType,
        system::{SystemFunction, SystemParamType},
        view::{view::PyView, view_param::ViewParamType},
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
    components_to_write: Vec<ComponentId>,
    components_to_read: Vec<ComponentId>,
    with_filters: Vec<ComponentId>,
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
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
// and DynamicSystem is only used within the context of a running Bevy app with Python active
unsafe impl Send for DynamicSystem {}
unsafe impl Sync for DynamicSystem {}

/// Extract QueryFilters from a PyQueryParam for disjointness checking.
///
/// Includes implicit With for all queried component types, matching Bevy's
/// behavior where `Query<&T>` implies `With<T>` for access checking.
fn query_filters_from_query_param(
    query_param: &crate::ecs::query::query_param::PyQueryParam,
) -> QueryFilters {
    let mut filters = QueryFilters::default();

    for data in &query_param.data {
        if let QueryData::Component { ty, .. } = data {
            filters.with.insert(ty.to_string());
        }
    }

    for filter in &query_param.filters {
        match filter {
            QueryFilter::With(with) => {
                for comp in &with.values {
                    filters.with.insert(comp.to_string());
                }
            }
            QueryFilter::Without(without) => {
                for comp in &without.values {
                    filters.without.insert(comp.to_string());
                }
            }
            QueryFilter::Changed(_)
            | QueryFilter::Added(_)
            | QueryFilter::Has(_)
            | QueryFilter::AnyOf(_) => {}
        }
    }
    filters
}

/// Extract QueryFilters from a PyViewParam for disjointness checking.
///
/// Includes implicit With for all viewed component types, matching Bevy's
/// behavior where accessing a component implies `With<T>`.
fn query_filters_from_view_param(
    view_param: &crate::ecs::view::view_param::PyViewParam,
) -> QueryFilters {
    let mut filters = QueryFilters::default();

    for param_type in &view_param.parameters {
        let ViewParamType::Component { comp_type, .. } = param_type;
        filters.with.insert(comp_type.to_string());
    }

    for comp in &view_param.with_filters {
        filters.with.insert(comp.to_string());
    }
    for comp in &view_param.without_filters {
        filters.without.insert(comp.to_string());
    }
    // Changed/Added filters imply With (component must be present)
    for comp in &view_param.changed_filters {
        filters.with.insert(comp.to_string());
    }
    for comp in &view_param.added_filters {
        filters.with.insert(comp.to_string());
    }
    filters
}

impl DynamicSystem {
    pub(crate) fn new(
        func: Py<PyAny>,
        generation: u32,
        error_state: Arc<Mutex<Vec<PyErr>>>,
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
            components_to_write: Vec::new(),
            components_to_read: Vec::new(),
            with_filters: Vec::new(),
            custom_component_ids: HashMap::new(),
            inner,
            func_name,
            last_run: None,
            needs_commands,
            args_buffer: SmallVec::new(),
            error_state,
            module_name,
            function_name,
            expected_generation: generation,
            stage,
            command_queue: CommandQueue::default(),
            last_error_msg: None,
            last_error_print_time: None,
            suppressed_error_count: 0,
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

    /// Convert system parameters to ParamAccess for validation.
    fn to_param_accesses<K: std::hash::Hash + Eq + Clone>(
        params: &[crate::ecs::system::SystemParam],
        mut get_key: impl FnMut(&PyComponentType) -> K,
    ) -> Vec<ParamAccess<K>> {
        params
            .iter()
            .map(|param| match &param.ty {
                SystemParamType::Query { param: query_param } => {
                    let filters = query_filters_from_query_param(query_param);
                    let accesses = query_param
                        .data
                        .iter()
                        .filter_map(|d| {
                            if let QueryData::Component {
                                ty: comp_type,
                                mutable,
                                ..
                            } = d
                            {
                                Some(ComponentAccess {
                                    key: get_key(comp_type),
                                    name: comp_type.to_string(),
                                    mutable: *mutable,
                                })
                            } else {
                                None
                            }
                        })
                        .collect();
                    ParamAccess::Components { accesses, filters }
                }
                SystemParamType::View { param: view_param } => {
                    let filters = query_filters_from_view_param(view_param);
                    let accesses = view_param
                        .parameters
                        .iter()
                        .map(|vpt| {
                            let ViewParamType::Component { comp_type, mutable } = vpt;
                            ComponentAccess {
                                key: get_key(comp_type),
                                name: comp_type.to_string(),
                                mutable: *mutable,
                            }
                        })
                        .collect();
                    ParamAccess::Components { accesses, filters }
                }
                SystemParamType::Resource { type_obj, mutable } => {
                    let type_ptr = type_obj.as_ptr() as usize;
                    let type_name = format!("Resource@{:x}", type_ptr);
                    ParamAccess::Resource {
                        key: type_ptr,
                        name: type_name,
                        mutable: *mutable,
                    }
                }
                SystemParamType::Assets {
                    type_ptr,
                    wrapper_class,
                    mutable,
                } => {
                    // Use wrapper_class (e.g. GroundMaterial) as conflict key when present,
                    // falling back to type_ptr (e.g. ShaderMaterial). This matches Bevy
                    // semantics where Assets<MaterialA> and Assets<MaterialB> are separate
                    // resources even when both are backed by the same underlying type.
                    let asset_type_name = if let Some(wrapper) = wrapper_class {
                        format!("{:p}", wrapper.0)
                    } else {
                        format!("{:p}", type_ptr.0)
                    };
                    ParamAccess::Assets {
                        key: asset_type_name.clone(),
                        name: asset_type_name,
                        mutable: *mutable,
                    }
                }
                SystemParamType::World => ParamAccess::World,
                _ => ParamAccess::None,
            })
            .collect()
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
        // Defense-in-depth: verify this system's generation matches the current active
        // generation BEFORE doing any work. This prevents zombie system execution if the
        // schedule-level run_if(generation_matches(N)) condition is bypassed for any reason
        // (e.g., schedule rebuild edge cases). Without this guard, each hot reload would
        // accumulate duplicate systems that all execute on the same entities every frame.
        {
            let world_ref = unsafe { world.world() };
            let current_gen = world_ref
                .get_resource::<HotReloadGeneration>()
                .map(|res| res.current)
                .unwrap_or(0);
            if current_gen != self.expected_generation {
                return Ok(());
            }
        }

        // Start timing for profiler (captures entire system execution)
        Python::attach(|py| {
            let start_time = Instant::now();

            // Create validity flag for this system execution
            // This will be shared by all system parameters
            let validity = ValidityFlag::new();

            // Create RAII guard that will automatically invalidate when system completes
            // This happens even if Python code panics
            let _validity_guard = ValidityGuard::new(validity.clone());

            // Reuse the args buffer - clear it and it keeps its capacity
            self.args_buffer.clear();

            // Track any parameter preparation errors
            let mut param_error: Option<PyErr> = None;

            // Create Commands if needed using a local CommandQueue.
            // This avoids needing &mut World (which is unsound in parallel systems).
            // Instead, we use UnsafeWorldCell::entities() and entities_allocator()
            // which are safe for concurrent access (read-only metadata).
            // The queue is appended to self.command_queue after the system runs,
            // then flushed to the world via queue_deferred().
            let mut local_command_queue = if self.needs_commands {
                Some(CommandQueue::default())
            } else {
                None
            };
            let mut commands_storage = if let Some(ref mut queue) = local_command_queue {
                Some(
                    pybevy_ecs::shared::command_queue_helpers::create_commands_from_queue(
                        queue, world,
                    ),
                )
            } else {
                None
            };

            // Get world_mut for other parameters (unsafe: we know Commands won't conflict)
            let world_mut = unsafe { world.world_mut() };

            // Lock the inner state to access system_func and message_cursor_storage
            let mut inner_guard = lock_or_recover(&self.inner);
            if inner_guard.gutted {
                return;
            }
            let Some(system_func) = inner_guard.system_func.as_ref() else {
                eprintln!(
                    "⚠️ System {}.{} has no function — skipping",
                    self.module_name, self.function_name
                );
                return;
            };

            let mut message_reader_idx = 0usize;
            for param in &system_func.params {
                match &param.ty {
                    SystemParamType::Local(local) => {
                        self.args_buffer.push(local.clone_ref(py));
                    }
                    SystemParamType::Resource { type_obj, mutable } => {
                        // Fetch resource from world using PyResourceType
                        let type_bound = type_obj.bind(py);
                        let resource_type = match PyResourceType::try_from((type_bound, py)) {
                            Ok(rt) => rt,
                            Err(e) => {
                                param_error = Some(e);
                                break; // Stop processing parameters
                            }
                        };

                        // Use appropriate extraction method based on mutability
                        let resource = if *mutable {
                            // Mutable access - get borrowed mutable reference
                            let world_mut = unsafe { world.world_mut() };
                            match resource_type.get_from_world_mut(world_mut, py, validity.clone())
                            {
                                Ok(r) => r,
                                Err(_e) => {
                                    let type_name = type_bound
                                        .name()
                                        .map(|n| n.to_string())
                                        .unwrap_or_else(|_| "Unknown".to_string());
                                    let err_msg = format!(
                                        "System `{}`: resource `{}` not found in world",
                                        self.func_name, type_name
                                    );
                                    param_error = Some(PyTypeError::new_err(err_msg));
                                    break; // Stop processing parameters
                                }
                            }
                        } else {
                            // Read-only access - get borrowed const reference
                            let world_ref = unsafe { world.world() };
                            match resource_type.get_from_world(world_ref, py, validity.clone()) {
                                Ok(r) => r,
                                Err(_e) => {
                                    let type_name = type_bound
                                        .name()
                                        .map(|n| n.to_string())
                                        .unwrap_or_else(|_| "Unknown".to_string());
                                    let err_msg = format!(
                                        "System `{}`: resource `{}` not found in world",
                                        self.func_name, type_name
                                    );
                                    param_error = Some(PyTypeError::new_err(err_msg));
                                    break; // Stop processing parameters
                                }
                            }
                        };

                        Self::wrap_resource_in_res(py, resource, *mutable, &mut self.args_buffer);
                    }
                    SystemParamType::Query { param: query_param } => {
                        if query_param.single_entity_enforced {
                            // Use PySingleQuery for Single<T> queries
                            let single_query = unsafe {
                                PySingleQuery::new(
                                    query_param.clone(),
                                    world_mut,
                                    Arc::new(self.custom_component_ids.clone()),
                                    validity.clone(),
                                )
                            };

                            let obj =
                                Py::new(py, single_query).expect("Failed to create PySingleQuery");
                            self.args_buffer.push(obj.into_any());
                        } else {
                            // Use PyQueryIter for normal Query<T> queries
                            let query_runtime = unsafe {
                                PyQueryIter::new(
                                    query_param.clone(),
                                    world_mut,
                                    Arc::new(self.custom_component_ids.clone()),
                                    validity.clone(),
                                )
                            };

                            let obj =
                                Py::new(py, query_runtime).expect("Failed to create PyQueryIter");
                            self.args_buffer.push(obj.into_any());
                        }
                    }
                    SystemParamType::View { param } => {
                        // Extract component types and mutability from PyViewParam
                        let mut component_types = Vec::new();
                        let mut mutable_components = HashSet::new();

                        for view_param_type in &param.parameters {
                            let ViewParamType::Component { comp_type, mutable } = view_param_type;
                            component_types.push(comp_type.clone());
                            if *mutable {
                                mutable_components.insert(comp_type.clone());
                            }
                        }

                        let filter_types = param.with_filters.to_vec();
                        let without_filter_types = param.without_filters.to_vec();
                        let changed_filter_types = param.changed_filters.to_vec();
                        let added_filter_types = param.added_filters.to_vec();
                        let last_run = self.get_last_run();

                        let py_view = unsafe {
                            PyView::new_with_filters(
                                component_types,
                                mutable_components,
                                filter_types,
                                without_filter_types,
                                changed_filter_types,
                                added_filter_types,
                                last_run,
                                world_mut,
                                validity.clone(),
                            )
                        };
                        let obj = Py::new(py, py_view).expect("Failed to create PyView");
                        self.args_buffer.push(obj.into_any());
                    }
                    SystemParamType::World => {
                        // Create PyWorld wrapper for exclusive world access
                        let py_world = unsafe { PyWorld::new(world_mut, validity.clone()) };
                        let obj = Py::new(py, py_world).expect("Failed to create PyWorld");
                        self.args_buffer.push(obj.into_any());
                    }
                    SystemParamType::AssetServer => {
                        // Create PyAssetServer wrapper with world access
                        let py_asset_server =
                            unsafe { PyAssetServer::new(world_mut, validity.clone()) };
                        let obj = Py::new(py, (py_asset_server, PyResource))
                            .expect("Failed to create PyAssetServer");
                        self.args_buffer.push(obj.into_any());
                    }
                    SystemParamType::Commands => {
                        // Use the pre-created Commands from commands_storage
                        let commands = commands_storage
                            .as_mut()
                            .expect("Commands should be pre-created");
                        let py_commands = unsafe { PyCommands::new(commands, validity.clone()) };
                        let obj = Py::new(py, py_commands).expect("Failed to create PyCommands");
                        self.args_buffer.push(obj.into_any());
                    }
                    SystemParamType::MessageWriter { message_type } => {
                        // Create PyMessageWriter wrapper with world access

                        let py_world = unsafe { PyWorld::new(world_mut, validity.clone()) };
                        let py_writer = PyMessageWriter {
                            message_type: message_type.clone(),
                            world: py_world,
                        };
                        let obj = Py::new(py, py_writer).expect("Failed to create PyMessageWriter");
                        self.args_buffer.push(obj.into_any());
                    }
                    SystemParamType::MessageReader { message_type } => {
                        // Create PyMessageReader wrapper with world access and persistent cursor

                        let py_world_1 = unsafe { PyWorld::new(world_mut, validity.clone()) };
                        let py_world_2 = unsafe { PyWorld::new(world_mut, validity.clone()) };
                        let cursor = inner_guard
                            .message_cursor_storage
                            .get(message_reader_idx)
                            .cloned();
                        message_reader_idx += 1;
                        let py_messages = PyMessages {
                            message_type: message_type.clone(),
                            world: py_world_1,
                            cursor_storage: cursor,
                        };
                        let py_reader = PyMessageReader {
                            world: py_world_2,
                            messages: py_messages,
                        };
                        let obj = Py::new(py, py_reader).expect("Failed to create PyMessageReader");
                        self.args_buffer.push(obj.into_any());
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
                        // Create PyAssets wrapper with world access
                        let py_assets = unsafe {
                            PyAssets::new(
                                type_ptr.0,
                                wrapper_class.map(|w| w.0),
                                world_mut,
                                validity.clone(),
                                *mutable,
                            )
                        };
                        let obj = Py::new(py, (py_assets, PyResource))
                            .expect("Failed to create PyAssets");

                        if *mutable {
                            // Wrap in Mut[Assets[T]]
                            let assets_any: Bound<'_, PyAny> = obj.into_bound(py).into_any();
                            let mut_wrapper = Py::new(py, PyMut::new(assets_any))
                                .expect("Failed to create PyMut");
                            self.args_buffer.push(mut_wrapper.into_any());
                        } else {
                            self.args_buffer.push(obj.into_any());
                        }
                    }
                }
            }

            // Check if there was an error during parameter preparation
            if let Some(err) = param_error {
                // Drop the inner guard before acquiring error_state lock
                drop(inner_guard);
                // Store the error in shared state
                let mut error_lock = self.error_state.lock().unwrap();
                error_lock.push(err);
                // Don't call the function if there was an error
            } else {
                // Get current generation and refresh function if needed
                let current_generation = {
                    let world_ref = unsafe { world.world() };
                    if let Some(gen_res) = world_ref.get_resource::<HotReloadGeneration>() {
                        gen_res.current
                    } else {
                        0 // No hot reload, use generation 0
                    }
                };

                // Get the current version of the function (may trigger re-import on generation change)
                // Inlined from get_function() since we already hold the inner lock
                let func = {
                    let get_func_result: PyResult<Py<PyAny>> = (|| {
                        if self.module_name == "__main__" {
                            if let Some(ref func) = inner_guard.cached_func {
                                return Ok(func.clone_ref(py));
                            } else {
                                return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                                    "Function {}.{} not cached",
                                    self.module_name, self.function_name
                                )));
                            }
                        }
                        if inner_guard.cached_generation != current_generation
                            || inner_guard.cached_func.is_none()
                        {
                            let module = py.import(self.module_name.as_str())?.into_any();
                            let func = module.getattr(self.function_name.as_str())?;
                            inner_guard.cached_func = Some(func.unbind());
                            inner_guard.cached_generation = current_generation;
                            if is_verbose() {
                                eprintln!(
                                    "🔄 Hot reload: Refreshed function {}.{} for generation {}",
                                    self.module_name, self.function_name, current_generation
                                );
                            }
                        }
                        Ok(inner_guard.cached_func.as_ref().unwrap().clone_ref(py))
                    })();
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
                            return;
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
                    let tuple =
                        PyTuple::new(py, &self.args_buffer).expect("Failed to create PyTuple");
                    func.bind(py).call1(tuple)
                };

                if let Err(e) = call_result {
                    let error_str = e.to_string();

                    // Extract full Python traceback with file/line info
                    let traceback_str = e.traceback(py).map(|tb| {
                        tb.format()
                            .unwrap_or_else(|_| "(traceback format failed)".into())
                    });

                    // Store error for MCP get_last_error (always)
                    let world_ref = unsafe { world.world_mut() };
                    let current_time = world_ref
                        .get_resource::<Time>()
                        .map(|t| t.elapsed_secs_f64())
                        .unwrap_or(0.0);
                    let mut last_error = world_ref
                        .get_resource_or_insert_with(pybevy_core::LastSystemError::default);
                    last_error.error = Some(error_str.clone());
                    last_error.traceback = traceback_str;
                    last_error.timestamp_secs = current_time;

                    // When hot reload is NOT active, store the error for propagation
                    // to the caller (app.update() / app.run()). This makes assert/raise
                    // in systems actually fail tests and standalone scripts.
                    let hot_reload_active = {
                        let wr = unsafe { world.world() };
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

            // Consume the Commands wrapper before appending the queue
            // (Commands borrows local_command_queue, must release that borrow first)
            let _commands_storage = commands_storage;

            // Append queued commands to the per-system queue for deferred application
            if let Some(mut queue) = local_command_queue {
                self.command_queue.append(&mut queue);
            }

            // Record timing at the end of system execution (captures entire execution block)
            let duration = start_time.elapsed();
            let world_ref = unsafe { world.world() };
            if let Some(profiler) = world_ref.get_resource::<SystemProfiler>() {
                // Get current time from Time resource for startup visibility tracking
                let current_time = world_ref
                    .get_resource::<Time>()
                    .map(|t| t.elapsed_secs_f64())
                    .unwrap_or(0.0);
                profiler.record_timing(&self.func_name, duration, self.stage, current_time);
            }
        });

        // Update last_run tick for change detection (same as FunctionSystem::run_unsafe)
        self.last_run = Some(world.change_tick());

        Ok(())
    }

    fn apply_deferred(&mut self, world: &mut World) {
        self.command_queue.apply(world);
    }

    fn queue_deferred(&mut self, _world: DeferredWorld) {}

    unsafe fn validate_param_unsafe(
        &mut self,
        world: UnsafeWorldCell,
    ) -> Result<(), SystemParamValidationError> {
        let params = {
            let inner = lock_or_recover(&self.inner);
            if inner.gutted {
                return Ok(());
            }
            inner.system_func.as_ref().unwrap().params.clone()
        };
        let accesses = Self::to_param_accesses(&params, |comp_type| {
            self.get_component_id_for_validation(world, comp_type)
        });
        shared_validation::validate_access(&accesses).map_err(|conflict| {
            let error_msg = format!(
                "System '{}' has conflicting component access:\n\
                 - Parameter {} requests {} access to {}\n\
                 - Parameter {} already has {} access to {}\n\
                 Rust's borrowing rules forbid multiple mutable references or \
                 mixing mutable and immutable references to the same data.",
                self.func_name,
                conflict.param_idx,
                if conflict.mutable {
                    "mutable"
                } else {
                    "immutable"
                },
                conflict.comp_name,
                conflict.existing_idx,
                if conflict.existing_mut {
                    "mutable"
                } else {
                    "immutable"
                },
                conflict.existing_name
            );
            SystemParamValidationError::new::<Self>(
                true, // DynamicSystem implements Send + Sync
                error_msg,
                format!("parameter_{}", conflict.param_idx),
            )
        })
    }

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

        // Collect component IDs and filters first to avoid borrow conflicts
        let mut read_ids: Vec<ComponentId> = Vec::new();
        let mut write_ids = Vec::new();
        let mut with_ids = Vec::new();

        // Parse system parameters from the SystemFunction and register component accesses
        for param in &params {
            match &param.ty {
                SystemParamType::Query { param: query_param } => {
                    // For queries, we need to register component accesses
                    for param_type in &query_param.data {
                        if let QueryData::Component {
                            ty: comp_type,
                            mutable,
                            ..
                        } = param_type
                        {
                            let id = match comp_type {
                                PyComponentType::Custom(type_ptr) => {
                                    // Check if we've already registered this custom component
                                    if let Some(&id) = self.custom_component_ids.get(type_ptr) {
                                        id
                                    } else {
                                        // Get the component name from the Python type
                                        let name = Python::attach(|py| {
                                            get_python_type_name(py, *type_ptr)
                                        });

                                        // Register the custom component and store its ID
                                        let id = register_custom_component(world, *type_ptr, name);
                                        self.custom_component_ids.insert(*type_ptr, id);
                                        id
                                    }
                                }
                                // All built-in component types
                                _ => register_component_id(
                                    world,
                                    comp_type,
                                    &self.custom_component_ids,
                                ),
                            };

                            // Register read or write access based on mutability
                            if *mutable {
                                write_ids.push(id);
                            } else {
                                // Read-only access - enables parallel execution of multiple read-only queries
                                read_ids.push(id);
                            }
                        }
                    }

                    // Handle filters
                    for filter in &query_param.filters {
                        let component_types_opt: Option<&[PyComponentType]> = match filter {
                            QueryFilter::With(with) => Some(&with.values),
                            QueryFilter::Without(without) => Some(&without.values),
                            QueryFilter::Changed(changed) => {
                                Some(std::slice::from_ref(&changed.component_type))
                            }
                            QueryFilter::Added(added) => {
                                Some(std::slice::from_ref(&added.component_type))
                            }
                            QueryFilter::Has(has) => {
                                Some(std::slice::from_ref(&has.component_type))
                            }
                            QueryFilter::AnyOf(any_of) => Some(&any_of.values),
                        };

                        if let Some(component_types) = component_types_opt {
                            for comp_type in component_types {
                                let id = match comp_type {
                                    PyComponentType::Custom(type_ptr) => {
                                        // Check if we've already registered this custom component
                                        if let Some(&id) = self.custom_component_ids.get(type_ptr) {
                                            id
                                        } else {
                                            // Get the component name from the Python type
                                            let name = Python::attach(|py| {
                                                let type_obj = unsafe {
                                                    Bound::from_borrowed_ptr(
                                                        py,
                                                        *type_ptr as *mut PyObject,
                                                    )
                                                };
                                                let type_bound = type_obj.cast::<PyType>()?;
                                                Ok::<String, PyErr>(type_bound.name()?.to_string())
                                            })
                                            .unwrap_or_else(|_| "Unknown".to_string());

                                            // Register the custom component and store its ID
                                            let id =
                                                register_custom_component(world, *type_ptr, name);
                                            self.custom_component_ids.insert(*type_ptr, id);
                                            id
                                        }
                                    }
                                    // For built-in components, use the generated register_simple method
                                    _ => comp_type.register_simple(world),
                                };
                                with_ids.push(id);
                            }
                        }
                    }
                }
                SystemParamType::Resource {
                    type_obj: _,
                    mutable: _,
                } => {
                    // Resources will be handled differently - they don't affect filtered access
                }
                SystemParamType::Assets { .. } => {
                    // Assets are resources - they don't affect filtered access
                }
                SystemParamType::View { param } => {
                    // Extract component types and mutability from PyViewParam
                    let mut component_types = Vec::new();
                    let mut mutable_components = HashSet::new();

                    for view_param_type in &param.parameters {
                        let ViewParamType::Component { comp_type, mutable } = view_param_type;
                        component_types.push(comp_type.clone());
                        if *mutable {
                            mutable_components.insert(comp_type.clone());
                        }
                    }

                    // Register components with correct access modes
                    for comp_type in &component_types {
                        let id = match comp_type {
                            PyComponentType::Custom(type_ptr) => {
                                // Check if already registered
                                if let Some(&id) = self.custom_component_ids.get(type_ptr) {
                                    id
                                } else {
                                    // Get the component name from the Python type
                                    let name = Python::attach(|py| {
                                        let type_obj = unsafe {
                                            Bound::from_borrowed_ptr(py, *type_ptr as *mut PyObject)
                                        };
                                        let type_bound = type_obj.cast::<PyType>()?;
                                        Ok::<String, PyErr>(type_bound.name()?.to_string())
                                    })
                                    .unwrap_or_else(|_| "Unknown".to_string());

                                    // Register the custom component
                                    let id = register_custom_component(world, *type_ptr, name);
                                    self.custom_component_ids.insert(*type_ptr, id);
                                    id
                                }
                            }
                            // All built-in components (Transform, PointLight, etc.) use Dynamic dispatch
                            PyComponentType::Dynamic(_) => {
                                register_component_id(world, comp_type, &self.custom_component_ids)
                            }
                        };

                        // Add to appropriate access list based on mutability
                        if mutable_components.contains(comp_type) {
                            write_ids.push(id);
                        } else {
                            read_ids.push(id);
                        }
                    }
                }
                SystemParamType::Local(_) => {
                    // Local state doesn't affect world access
                }
                SystemParamType::AssetServer => {
                    // AssetServer is a resource - doesn't affect filtered access
                }
                SystemParamType::World => {
                    // World access requires exclusive access - no filtering needed
                }
                SystemParamType::Commands => {
                    // Commands don't require specific component access
                }
                SystemParamType::MessageWriter { message_type } => {
                    if let Some(id) = message_type.resource_id(world) {
                        write_ids.push(id);
                    }
                }
                SystemParamType::MessageReader { message_type } => {
                    if let Some(id) = message_type.resource_id(world) {
                        read_ids.push(id);
                    }
                }
                SystemParamType::On { .. } => {
                    // Observers don't require component access registration
                    // They're triggered via World.trigger() and have separate execution paths
                }
            }
        }

        // Apply the collected IDs to self for tracking
        for id in read_ids {
            self.add_read(id);
        }
        for id in write_ids {
            self.add_write(id);
        }
        for id in with_ids {
            self.add_with(id);
        }

        pybevy_ecs::shared::access_sets::build_access_set(
            &self.components_to_read,
            &self.components_to_write,
            &self.with_filters,
        )
    }

    fn check_change_tick(&mut self, _check: CheckChangeTicks) {}

    fn get_last_run(&self) -> Tick {
        self.last_run.unwrap_or(Tick::new(0))
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.last_run = Some(last_run);
    }
}

impl DynamicSystem {
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
        let accesses = Self::to_param_accesses(&params, |comp_type| comp_type.to_string());
        shared_validation::validate_access(&accesses).map_err(|conflict| {
            PyRuntimeError::new_err(format!(
                "System '{}' has conflicting component access:\n\
                 - Parameter {} requests {} access to {}\n\
                 - Parameter {} already has {} access to {}\n\
                 Rust's borrowing rules forbid multiple mutable references or \
                 mixing mutable and immutable references to the same data.",
                self.func_name,
                conflict.param_idx,
                if conflict.mutable {
                    "mutable"
                } else {
                    "immutable"
                },
                conflict.comp_name,
                conflict.existing_idx,
                if conflict.existing_mut {
                    "mutable"
                } else {
                    "immutable"
                },
                conflict.existing_name
            ))
        })
    }

    fn add_read(&mut self, id: ComponentId) {
        if !self.components_to_read.contains(&id) {
            self.components_to_read.push(id);
        }
    }

    fn add_write(&mut self, id: ComponentId) {
        if !self.components_to_write.contains(&id) {
            self.components_to_write.push(id);
        }
    }

    fn add_with(&mut self, id: ComponentId) {
        if !self.with_filters.contains(&id) {
            self.with_filters.push(id);
        }
    }

    /// Get the ComponentId for a given component type during validation.
    /// This uses the already-registered IDs from initialize() or looks up custom components.
    fn get_component_id_for_validation(
        &self,
        world: UnsafeWorldCell,
        comp_type: &PyComponentType,
    ) -> ComponentId {
        // SAFETY: We're in validate_param_unsafe which is called during system initialization
        // The world reference is valid for the duration of this call
        let world_ref = unsafe { world.world_mut() };

        match comp_type {
            PyComponentType::Custom(type_ptr) => {
                // Look up the already-registered custom component ID
                if let Some(&id) = self.custom_component_ids.get(type_ptr) {
                    id
                } else {
                    // This shouldn't happen if initialize() ran first, but handle it gracefully
                    // Register it now (this will return the existing ID if already registered)
                    let name = Python::attach(|py| {
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
/// * `system_func` - The parsed system function with parameters
/// * `world` - Mutable world reference
/// * `on_param` - The On trigger parameter (required for observer dispatch)
pub(crate) fn execute_system_func(
    py: Python,
    system_func: &SystemFunction,
    world: &mut World,
    on_param: Py<PyOn>,
) -> PyResult<()> {
    let validity = ValidityFlag::new();
    let _validity_guard = ValidityGuard::new(validity.clone());

    let mut args_buffer: Vec<Py<PyAny>> = Vec::with_capacity(system_func.params.len());

    // Build custom_component_ids from the world's ComponentRegistry
    let custom_component_ids = {
        let registry = world.get_resource::<crate::ecs::component_type::ComponentRegistry>();
        if let Some(reg) = registry {
            Arc::new(reg.registry.clone())
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
                if query_param.single_entity_enforced {
                    let single_query = unsafe {
                        PySingleQuery::new(
                            query_param.clone(),
                            world,
                            custom_component_ids.clone(),
                            validity.clone(),
                        )
                    };
                    let obj = Py::new(py, single_query).expect("Failed to create PySingleQuery");
                    args_buffer.push(obj.into_any());
                } else {
                    let query_runtime = unsafe {
                        PyQueryIter::new(
                            query_param.clone(),
                            world,
                            custom_component_ids.clone(),
                            validity.clone(),
                        )
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
            SystemParamType::AssetServer => {
                let py_asset_server = unsafe { PyAssetServer::new(world, validity.clone()) };
                let obj = Py::new(py, (py_asset_server, PyResource))
                    .expect("Failed to create PyAssetServer");
                args_buffer.push(obj.into_any());
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
                let py_assets = unsafe {
                    PyAssets::new(
                        type_ptr.0,
                        wrapper_class.map(|w| w.0),
                        world,
                        validity.clone(),
                        *mutable,
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
                let mut component_types = Vec::new();
                let mut mutable_components = HashSet::new();
                for view_param_type in &param.parameters {
                    let ViewParamType::Component { comp_type, mutable } = view_param_type;
                    component_types.push(comp_type.clone());
                    if *mutable {
                        mutable_components.insert(comp_type.clone());
                    }
                }
                let filter_types = param.with_filters.to_vec();
                let without_filter_types = param.without_filters.to_vec();
                let changed_filter_types = param.changed_filters.to_vec();
                let added_filter_types = param.added_filters.to_vec();
                let py_view = unsafe {
                    PyView::new_with_filters(
                        component_types,
                        mutable_components,
                        filter_types,
                        without_filter_types,
                        changed_filter_types,
                        added_filter_types,
                        Tick::new(0), // No prior run for observers
                        world,
                        validity.clone(),
                    )
                };
                let obj = Py::new(py, py_view).expect("Failed to create PyView");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::MessageWriter { message_type } => {
                let py_world = unsafe { PyWorld::new(world, validity.clone()) };
                let py_writer = PyMessageWriter {
                    message_type: message_type.clone(),
                    world: py_world,
                };
                let obj = Py::new(py, py_writer).expect("Failed to create PyMessageWriter");
                args_buffer.push(obj.into_any());
            }
            SystemParamType::MessageReader { message_type } => {
                let py_world_1 = unsafe { PyWorld::new(world, validity.clone()) };
                let py_world_2 = unsafe { PyWorld::new(world, validity.clone()) };
                let py_messages = PyMessages {
                    message_type: message_type.clone(),
                    world: py_world_1,
                    cursor_storage: None,
                };
                let py_reader = PyMessageReader {
                    world: py_world_2,
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
