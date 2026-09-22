//! Python bindings for Bevy's World, the central ECS container.
//!
//! PyWorld uses a custom `WorldStorage` enum instead of `ComponentStorage`
//! because its `&self` methods need interior mutability over `&mut World`,
//! owned worlds keep their validity flag until teardown, and
//! spawn/despawn/trigger/resource operations don't fit the component storage
//! abstraction. Use `PyWorld::with_temporary()` for temporary access with
//! automatic validity management.

use std::{
    alloc::Layout,
    any::TypeId,
    cell::UnsafeCell,
    collections::{HashMap, HashSet},
    ptr::fn_addr_eq,
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{
        ptr::OwningPtr,
        system::{RunSystemError, System},
        world::World,
    },
    prelude::*,
};
use pybevy_core::{
    AssetAccessRegistry, AssetBorrowCounter, ensure_asset_access_registry,
    ensure_no_live_asset_access, extract_entity_from_any,
    public_error::{
        COMPONENT_BRIDGE_NOT_FOUND, RESOURCE_BRIDGE_NOT_FOUND, RESOURCE_ENTITY_DESPAWN,
        WORLD_BATCH_ARGUMENTS, WORLD_CALLBACK_COMMAND_ERRORS, one_shot_parameter_validation_skip,
        unregistered_message_write,
    },
    registry::global_registry,
    resource_initializer,
};
use pybevy_ecs::shared::{
    parity_trace::ParityRunHandle,
    schedule::{StateScheduleLabel, TransitionScheduleLabel},
    system_runtime::ErrorPolicy,
};
use pybevy_reload::{HotReloadGeneration, SystemStage};
use pyo3::{
    PyTraverseError, PyTypeInfo, PyVisit,
    exceptions::{PyRuntimeError, PyTypeError},
    ffi::PyTypeObject,
    prelude::*,
    types::{PyList, PyTuple, PyType, PyTypeMethods},
};

use crate::{
    app::PyStage,
    assets::{asset_type::PyAssetTypeParam, assets::PyAssets},
    ecs::{
        PyEntity,
        batch_spawn::{SpawnBatchCommand, prepare_iter_batch},
        commands::PyCommands,
        component::{PyComponent, PyComponentId},
        component_layout::{ComponentLayoutExt, ComponentStorageType, ComponentStorageTypeExt},
        component_type::{
            ComponentRegistry, PyComponentType, drop_py_object, register_custom_component,
        },
        custom_batch::PyCustomComponentBatch,
        custom_component::PyCustomComponent,
        deferred_drop,
        dynamic_system::lock_or_recover,
        entity_commands::PyEntityCommands,
        helpers::validity_guard::{AccessMode, ValidityFlag, ValidityFlagWithMode, ValidityGuard},
        lazy_wrapper_proxy::{ProxyKind, PyLazyWrapperProxy},
        message::{PyMessageId, PyMessageWriter},
        messages::{MessageType, MessageWorld, PyMessageType},
        observer::{PyEvent, PyOn},
        observer_registry::ObserverRegistry,
        python_message::{python_message_is_registered, resolve_from_world},
        query::{query_param::PyQueryParam, query_runtime::PyQueryIter},
        resource::{PyRes, PyResMut, hierarchy_contains_resource_entity},
        resource_type::{
            PyResourceType, ResourceRegistry, register_custom_resource,
            reject_unparameterized_button_input,
        },
        state::{
            PyOnEnterSchedule, PyOnExitSchedule, PyOnTransitionSchedule,
            canonicalize_state_schedule_label, canonicalize_transition_schedule_label,
        },
        system_interpreter::new_main_one_shot_system,
        world_commands::{self, WorldCommandState},
        world_gc::WorldGcState,
    },
};

/// Internal storage for World - either owned or borrowed
enum WorldStorage {
    /// An owned World instance (using UnsafeCell for interior mutability)
    Owned(Box<UnsafeCell<World>>),
    /// A borrowed mutable reference to a World (as a raw pointer)
    Borrowed(*mut World),
}

enum EitherStateSchedule {
    State(StateScheduleLabel),
    Transition(TransitionScheduleLabel),
}

/// Represents exclusive access to the Bevy ECS World within a system.
/// This is passed to Python systems that request World access.
///
/// Note: World access requires an exclusive system, which prevents parallel execution.
#[pyclass(name = "World", module = "pybevy.ecs")]
pub struct PyWorld {
    storage: WorldStorage,
    pub(crate) parity_trace: Option<ParityRunHandle>,
    // Runtime validity check - prevents use after the underlying World goes away.
    // For borrowed worlds (system params) this is the system flag, invalidated on
    // system exit. For owned worlds (created from Python) it is a flag that starts
    // valid and is invalidated in Drop, so proxies handed out by get/get_mut (which
    // cache a raw world_ptr, not a Py<PyWorld>) cannot outlive `del world`.
    validity: Option<ValidityFlag>,
    asset_borrow_counters: Arc<Mutex<HashMap<TypeId, AssetBorrowCounter>>>,
    gc_state: Option<WorldGcState>,
    /// Flushes queued Python finalizers after the storage fields drop, so an
    /// owned World's teardown decrefs never leak past destruction.
    _deferred_flush: deferred_drop::MutationFlushGuard,
}

// SAFETY: PyWorld is Send because:
// - The raw pointer is protected by the ValidityFlag (Arc<AtomicBool>)
// - ValidityFlag::check() ensures the pointer is only dereferenced when valid
// - The validity flag is set to false when the system execution completes
// - Owned worlds use UnsafeCell but access is controlled by &mut methods
unsafe impl Send for PyWorld {}

// SAFETY: PyWorld is Sync because:
// - Access to the underlying World is controlled by validity checking
// - The ValidityFlag uses atomic operations for thread-safe access
// - We only allow access when the validity flag is true (during system execution)
unsafe impl Sync for PyWorld {}

impl Drop for PyWorld {
    fn drop(&mut self) {
        if matches!(self.storage, WorldStorage::Owned(_))
            && let Some(state) = &self.gc_state
        {
            state.close();
        }
        // An owned world frees its `World` storage after this body returns
        // (fields drop in declaration order). Invalidate its validity flag
        // first so any proxy/handle that outlived `del world` - it caches a
        // raw `world_ptr`, not a `Py<PyWorld>` - fails its validity check on
        // next access instead of dereferencing freed memory. Only owned worlds
        // own their flag; a borrowed world shares the system flag managed by
        // ValidityGuard (and may be one of several duplicates), so leave those
        // untouched. The trailing `_deferred_flush` field drains the retired
        // Python values after the storage drops; finalizers touching the dying
        // world see the invalidated flag and are rejected, as with an inline
        // drop.
        if let WorldStorage::Owned(_) = self.storage
            && let Some(flag) = &self.validity
        {
            flag.set_invalid();
        }
    }
}

impl PyWorld {
    pub(crate) fn gc_state(&self) -> Option<WorldGcState> {
        self.gc_state.clone()
    }

    pub(crate) fn default_resource_instance(
        py: Python<'_>,
        type_obj: &Bound<'_, PyType>,
    ) -> PyResult<Py<PyAny>> {
        type_obj
            .call0()
            .map(Bound::unbind)
            .map_err(|error| {
                if error.to_string().contains("missing")
                    && error.to_string().contains("required")
                {
                    let type_name = type_obj.name().unwrap_or_else(|_| {
                        pyo3::types::PyString::new(py, "Resource")
                    });
                    PyTypeError::new_err(format!(
                        "Cannot initialize resource `{type_name}` with default values: resource requires constructor arguments. Use `insert_resource()` instead."
                    ))
                } else {
                    error
                }
            })
    }

    /// Create a new PyWorld wrapper around a mutable World reference.
    ///
    /// # Safety
    /// The world pointer must be valid for the lifetime of this PyWorld instance.
    /// This should only be created within the system's run_unsafe and dropped before returning.
    pub(crate) unsafe fn new(world: &mut World, validity: ValidityFlag) -> Self {
        ensure_asset_access_registry(world);
        Self {
            gc_state: WorldGcState::for_world(world.id()),
            storage: WorldStorage::Borrowed(world as *mut World),
            parity_trace: None,
            validity: Some(validity),
            asset_borrow_counters: Arc::new(Mutex::new(HashMap::new())),
            _deferred_flush: deferred_drop::MutationFlushGuard,
        }
    }

    /// Create a new PyWorld that owns its World
    pub(crate) fn new_owned(mut world: World) -> Self {
        ensure_asset_access_registry(&mut world);
        world_commands::initialize(&mut world);
        let validity = ValidityFlag::new_owned_world(world.id());
        let gc_state = WorldGcState::new(world.id());
        Self {
            storage: WorldStorage::Owned(Box::new(UnsafeCell::new(world))),
            parity_trace: None,
            // Starts valid (Write mode); Drop invalidates it so any proxy/handle that
            // outlives `del world` errors instead of dereferencing the freed World.
            validity: Some(validity),
            asset_borrow_counters: Arc::new(Mutex::new(HashMap::new())),
            gc_state: Some(gc_state),
            _deferred_flush: deferred_drop::MutationFlushGuard,
        }
    }

    /// Check if this World instance is still valid for use
    pub(crate) fn check_valid(&self) -> PyResult<()> {
        if let Some(ref validity) = self.validity {
            Ok(validity.check()?)
        } else {
            Ok(()) // No flag: treat as valid (both owned and borrowed worlds carry one)
        }
    }

    /// Get a clone of the validity flag for sharing with child structures.
    /// Both owned and borrowed worlds carry a flag now, so this is `Some` in practice.
    pub(crate) fn validity(&self) -> Option<ValidityFlag> {
        self.validity.clone()
    }

    /// Return a validity-checked shared view of the underlying Bevy World.
    pub(crate) fn world_ref(&self) -> PyResult<&World> {
        self.check_valid()?;
        // SAFETY: both storage variants retain a live World for the wrapper's
        // validity window. This method exposes shared access only; callers that
        // can run native code or mutate the World must use the existing mutable
        // access path and its native-asset barrier.
        Ok(unsafe { &*self.world_ptr() })
    }

    // Raw pointer access behind the validity check, see docs/safety.md.
    // The returned guard's scope is a native mutation window: deferred Python
    // drops from the previous mutation drain before it, and drops caused by
    // this window drain when the borrow ends, before returning to Python.
    pub(crate) fn world_mut(&self) -> PyResult<deferred_drop::WorldMutGuard<'_>> {
        self.check_valid()?;
        let gc = self.gc_state.as_ref().map(WorldGcState::suspend);
        let world = match &self.storage {
            WorldStorage::Owned(boxed) => unsafe { &mut *boxed.get() },
            WorldStorage::Borrowed(ptr) => unsafe { &mut **ptr },
        };
        Ok(deferred_drop::WorldMutGuard::with_gc(world, gc))
    }

    /// Create a duplicate PyWorld that shares the same underlying world pointer
    /// This is used for creating multiple references to the same world (e.g., in iterators)
    pub(crate) fn duplicate(&self) -> Self {
        Self {
            storage: match &self.storage {
                WorldStorage::Owned(_) => {
                    panic!("Cannot duplicate owned world - only borrowed worlds can be duplicated")
                }
                WorldStorage::Borrowed(ptr) => WorldStorage::Borrowed(*ptr),
            },
            validity: self.validity.clone(),
            parity_trace: self.parity_trace.clone(),
            asset_borrow_counters: self.asset_borrow_counters.clone(),
            gc_state: self.gc_state.clone(),
            _deferred_flush: deferred_drop::MutationFlushGuard,
        }
    }

    pub(crate) fn world_ptr(&self) -> *mut World {
        match &self.storage {
            WorldStorage::Owned(boxed) => boxed.get(),
            WorldStorage::Borrowed(ptr) => *ptr,
        }
    }

    fn check_native_asset_access(&self, operation: &str) -> PyResult<()> {
        self.check_valid()?;
        let world = unsafe { &*self.world_ptr() };
        ensure_no_live_asset_access(world, operation)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }

    fn asset_borrow_counter(&self, type_ptr: *const PyTypeObject) -> PyResult<AssetBorrowCounter> {
        let bridge = global_registry::get_asset_bridge_by_py_type(type_ptr)
            .ok_or_else(|| PyTypeError::new_err("Assets[T] requires a registered asset bridge"))?;
        let type_id = bridge.bevy_type_id();
        if let Some(counter) = self
            .asset_borrow_counters
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&type_id)
            .cloned()
        {
            return Ok(counter);
        }

        let validity = self.validity.clone().unwrap_or_default();
        let origin = match &self.storage {
            WorldStorage::Owned(_) => "World()",
            WorldStorage::Borrowed(_) => "World",
        };
        let world = self.world_mut()?;
        let registry = world
            .get_resource::<AssetAccessRegistry>()
            .ok_or_else(|| PyRuntimeError::new_err("AssetAccessRegistry is not initialized"))?;
        let counter = AssetBorrowCounter::from_scope(registry.new_scope(
            type_id,
            bridge.name(),
            validity,
            origin,
        ));
        Ok(self
            .asset_borrow_counters
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry(type_id)
            .or_insert_with(|| counter.clone())
            .clone())
    }

    /// Execute a function with temporary World access, automatically managing validity guards.
    ///
    /// This helper eliminates the boilerplate of creating ValidityFlag and ValidityGuard
    /// for temporary World access. The PyWorld is only valid during the closure execution.
    ///
    /// # Example
    /// ```rust,ignore
    /// PyWorld::with_temporary(app.world_mut(), py, |py_world| {
    ///     py_world.init_resource(py, resource_type)
    /// })?;
    /// ```
    pub(crate) fn with_temporary<F, R>(world: &mut World, py: Python, f: F) -> PyResult<R>
    where
        F: FnOnce(&PyWorld) -> PyResult<R>,
    {
        let _flush = deferred_drop::MutationFlushGuard;
        let validity = ValidityFlag::new();
        let guard = ValidityGuard::for_world(validity.clone(), world.id());
        // SAFETY: the exclusive callback borrow outlives its guarded adapter.
        let py_world = unsafe { PyWorld::new(world, validity) };
        let result = f(&py_world);
        drop(py_world);
        drop(guard);
        world.flush();
        let commands = world_commands::raise_errors(world, py);
        match result {
            Ok(value) => commands.map(|()| value),
            Err(error) => match commands {
                Ok(()) => Err(error),
                Err(command_error) => {
                    let errors = vec![error.into_value(py), command_error.into_value(py)];
                    let group = py
                        .import("builtins")?
                        .getattr("BaseExceptionGroup")?
                        .call1((WORLD_CALLBACK_COMMAND_ERRORS, errors))?;
                    Err(PyErr::from_value(group))
                }
            },
        }
    }

    /// Internal helper to get Assets resource for a specific asset type.
    fn get_assets_resource(
        &self,
        py: Python,
        asset_param: &PyAssetTypeParam,
    ) -> PyResult<Py<PyAny>> {
        self.check_valid()?;
        let type_ptr = asset_param.type_ptr();
        let world_ptr = self.world_ptr();
        let validity = self.validity.clone().unwrap_or_default();

        // Create PyAssets wrapper for the specified asset type
        // When called from World.resource(), assume mutable access (for backwards compatibility)
        let _gc = self.gc_state.as_ref().map(WorldGcState::suspend);
        // SAFETY: `world_ptr` is valid while this PyWorld is valid; the derived cell is
        // fenced by the same `validity` flag. PyAssets only reaches the `Assets<T>` resource.
        let cell = unsafe { (*world_ptr).as_unsafe_world_cell() };
        let borrow_counter = self.asset_borrow_counter(type_ptr)?;
        let py_assets = unsafe {
            Py::new(
                py,
                resource_initializer(PyAssets::new(
                    type_ptr,
                    asset_param.wrapper_class(py),
                    asset_param.logical_type_id(),
                    asset_param.logical_type_name().map(str::to_owned),
                    cell,
                    validity,
                    true,
                    borrow_counter,
                )),
            )?
        };
        Ok(py_assets.into_any())
    }

    /// Extract a custom (Python-defined) component from an entity.
    fn extract_custom_component(
        &self,
        py: Python,
        entity_id: Entity,
        type_ptr: *const pyo3::ffi::PyTypeObject,
        validity: ValidityFlagWithMode,
    ) -> PyResult<Option<Py<PyAny>>> {
        let mut world = self.world_mut()?;

        let component_id = {
            // No ComponentRegistry resource => no custom components registered in
            // this world, so the entity cannot have this one. Match Bevy's
            // `World::get`, which returns `None` (not an error) for a missing or
            // unregistered component type.
            let Some(registry) = world.get_resource::<ComponentRegistry>() else {
                return Ok(None);
            };
            match registry.get(type_ptr as usize) {
                Some(id) => id,
                None => return Ok(None),
            }
        };

        let entity_ref = world.entity(entity_id);
        if entity_ref.get_by_id(component_id).is_err() {
            return Ok(None);
        }

        let storage_type = {
            // SAFETY: registered type pointers live for the interpreter lifetime
            let py_type =
                unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };
            if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                ComponentStorageType::from_python_class(cls)
                    .unwrap_or(ComponentStorageType::PyObject)
            } else {
                ComponentStorageType::PyObject
            }
        };

        match storage_type {
            ComponentStorageType::Wrapper(wrapper_size) => {
                let data_ptr: *mut u8 = {
                    let entity_ref = world.entity(entity_id);
                    let untyped = entity_ref
                        .get_by_id(component_id)
                        .expect("Component existence already verified");
                    unsafe { wrapper_size.get_ref_ptr_as_mut(untyped) }
                };

                // SAFETY: registered type pointers live for the interpreter lifetime
                let py_type = unsafe {
                    pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject)
                };
                let cls = py_type
                    .cast::<pyo3::types::PyType>()
                    .expect("Type pointer should be valid");
                let layout = std::sync::Arc::new(
                    crate::ecs::component_layout::ComponentLayout::from_annotations(cls)
                        .expect("Layout should be computable for wrapper components"),
                );

                let mutable = validity.access_mode() == AccessMode::Write;
                let world_ptr = self.world_ptr();
                let world_cell = world.as_unsafe_world_cell();
                let proxy = unsafe {
                    PyLazyWrapperProxy::new(
                        data_ptr,
                        layout,
                        type_ptr,
                        validity,
                        mutable,
                        component_id,
                        entity_id,
                        world_ptr,
                        world_cell,
                        None,
                        // Long-lived proxy: the caller may hold it across structural
                        // mutations, so re-resolve on every access.
                        ProxyKind::WorldGet,
                    )
                };

                let py_obj = Py::new(py, proxy).expect("Failed to create lazy wrapper proxy");
                Ok(Some(py_obj.into_any()))
            }
            ComponentStorageType::PyObject => {
                let entity_ref = world.entity(entity_id);
                let untyped_ptr = entity_ref
                    .get_by_id(component_id)
                    .expect("Component existence already verified")
                    .as_ptr();

                let py_object = unsafe {
                    let py_any_ref = &*(untyped_ptr as *const Py<PyAny>);
                    py_any_ref.clone_ref(py)
                };
                let world_cell = world.as_unsafe_world_cell();

                let custom_comp = PyCustomComponent::from_object(
                    py_object,
                    validity,
                    component_id,
                    entity_id,
                    world_cell,
                    None,
                );

                let py_obj = Py::new(py, (custom_comp, crate::ecs::component::PyComponent))
                    .expect("Failed to create PyCustomComponent");
                Ok(Some(py_obj.into_any()))
            }
        }
    }

    fn extract_custom_resource_component(
        &self,
        py: Python,
        entity_id: Entity,
        type_ptr: *const pyo3::ffi::PyTypeObject,
        mutable: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let mut world = self.world_mut()?;
        let Some(component_id) = world
            .get_resource::<ResourceRegistry>()
            .and_then(|registry| registry.get(type_ptr as usize))
        else {
            return Ok(None);
        };
        if world.resource_entities().get(component_id) != Some(entity_id) {
            return Ok(None);
        }

        let value = if mutable {
            let Some(mut value) = world.get_resource_mut_by_id(component_id) else {
                return Ok(None);
            };
            // SAFETY: custom resource IDs use Pyo3ResourceObjectDescriptor.
            unsafe { value.as_mut().deref_mut::<Py<PyAny>>().clone_ref(py) }
        } else {
            let Some(value) = world.get_resource_by_id(component_id) else {
                return Ok(None);
            };
            // SAFETY: custom resource IDs use Pyo3ResourceObjectDescriptor.
            unsafe { value.deref::<Py<PyAny>>().clone_ref(py) }
        };
        if mutable {
            Ok(Some(
                Py::new(
                    py,
                    PyResMut::with_validity(
                        value.bind(py).clone(),
                        self.validity.clone().unwrap_or_default(),
                    ),
                )?
                .into_any(),
            ))
        } else {
            Ok(Some(
                Py::new(
                    py,
                    PyRes::with_validity(
                        value.bind(py).clone(),
                        self.validity.clone().unwrap_or_default(),
                    ),
                )?
                .into_any(),
            ))
        }
    }
}

#[pymethods]
impl PyWorld {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        let WorldStorage::Owned(storage) = &self.storage else {
            return Ok(());
        };
        if self
            .validity
            .as_ref()
            .is_none_or(|flag| matches!(flag.get_mode(), AccessMode::Invalid))
        {
            return Ok(());
        }
        let Some(_gc) = self.gc_state.as_ref().and_then(WorldGcState::try_traverse) else {
            return Ok(());
        };
        // SAFETY: the gate preserves the allocation; this reference only constructs a cell.
        let world = unsafe { (&*storage.get()).as_unsafe_world_cell_readonly() };
        // SAFETY: the mutation gate excludes changes to the pending command owner.
        if let Some(commands) = unsafe { world.get_resource::<WorldCommandState>() } {
            commands.traverse(visit.clone())?;
        }
        // SAFETY: the gate excludes registration writes to this resource.
        if let Some(info) = unsafe { world.get_resource::<pybevy_core::CustomComponentInfo>() } {
            let mut owners = HashSet::new();
            for (_, entry) in info.iter() {
                if let Some(class) = &entry.retained_type
                    && owners.insert(Arc::as_ptr(class))
                {
                    visit.call(class.as_ref())?;
                }
            }
        }
        // SAFETY: the gate excludes registration writes to this resource.
        if let Some(info) = unsafe { world.get_resource::<pybevy_core::CustomResourceInfo>() } {
            for (_, entry) in info.iter() {
                visit.call(&entry.type_object)?;
            }
        }
        for component in world.components().iter_registered() {
            if component.layout() != Layout::new::<Py<PyAny>>()
                || !component.drop().is_some_and(|drop| {
                    fn_addr_eq(drop, drop_py_object as unsafe fn(OwningPtr<'_>))
                })
            {
                continue;
            }
            for archetype in world.archetypes().iter() {
                for entity in archetype.entities() {
                    let Ok(entity) = world.get_entity(entity.id()) else {
                        continue;
                    };
                    // SAFETY: the gate excludes writes to Python object storage slots.
                    if let Some(value) = unsafe { entity.get_by_id(component.id()) } {
                        // SAFETY: layout and drop identify the exact Python storage type.
                        visit.call(unsafe { value.deref::<Py<PyAny>>() })?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Create a new owned World from Python
    #[new]
    pub fn py_new() -> Self {
        Self::new_owned(World::new())
    }

    pub fn spawn_empty(pyself: PyRef<'_, Self>, _py: Python<'_>) -> PyResult<PyEntityCommands> {
        world_commands::ErrorBoundary::new(pyself.world_ref()?).run(|| {
            pyself.check_native_asset_access("world.spawn_empty()")?;
            let mut world = pyself.world_mut()?;
            let entity = world.spawn_empty().id();
            drop(world);
            Ok(PyEntityCommands::with_world(entity, pyself))
        })
    }

    #[pyo3(signature = (*components))]
    pub fn spawn(
        pyself: PyRef<'_, Self>,
        py: Python,
        components: &Bound<'_, PyTuple>,
    ) -> PyResult<PyEntityCommands> {
        world_commands::ErrorBoundary::new(pyself.world_ref()?).run(|| {
            pyself.check_valid()?;
            let components = crate::ecs::commands::normalize_spawn_components(components)?;
            let component_types = crate::ecs::commands::resolve_spawn_bundle(py, &components)?;
            pyself.check_native_asset_access("world.spawn()")?;

            let entity_id = pyself.world_mut()?.spawn_empty().id();

            // Create a temporary PyCommands wrapper around this world to reuse component insertion logic
            let world_ptr = pyself.world_ptr();
            let validity = pyself.validity.clone().unwrap_or_default();

            // SAFETY: We're creating a temporary PyCommands that will be used immediately
            // and dropped before returning, so the world pointer remains valid
            let temp_commands = unsafe { PyCommands::from_world_temporary(world_ptr, validity) };

            // The shared insertion helper owns Discard -> mutation -> Add ->
            // Insert ordering for both spawn and later insert paths.
            crate::ecs::commands::insert_resolved_components_to_entity(
                &temp_commands,
                entity_id,
                &components,
                component_types,
            )?;

            Ok(PyEntityCommands::with_world(entity_id, pyself))
        })
    }

    /// Despawn an entity
    pub fn despawn(&self, entity: &Bound<'_, PyAny>) -> PyResult<bool> {
        world_commands::ErrorBoundary::new(self.world_ref()?).run(|| {
            let entity = &extract_entity_from_any(entity)?;
            self.check_valid()?;
            let mut world = self.world_mut()?;
            if hierarchy_contains_resource_entity(&world, entity.0) {
                return Err(PyTypeError::new_err(RESOURCE_ENTITY_DESPAWN));
            }
            ensure_no_live_asset_access(&world, "world.despawn()")
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            Ok(crate::ecs::lifecycle_mutation::despawn_recursive(
                &mut world, entity.0,
            ))
        })
    }

    /// Get resource from the world
    pub fn resource(&self, py: Python, resource: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        self.check_valid()?;

        // Check if this is an Assets[T] parameter
        if resource.get_type().is(PyAssetTypeParam::type_object(py)) {
            let asset_param = resource.extract::<PyAssetTypeParam>()?;
            // Return the Assets resource for this asset type
            return self.get_assets_resource(py, &asset_param);
        }

        // Extract the resource type
        let type_obj: Bound<'_, PyType> = resource.extract()?;
        reject_unparameterized_button_input(&type_obj)?;
        let py_resource_type = PyResourceType::try_from((&type_obj, py))?;

        // Get the world reference
        let world = unsafe { &*self.world_ptr() };

        // Get validity flag (use a new one if this is an owned world)
        let validity = self.validity.clone().unwrap_or_default();

        // Retrieve the resource from the world
        let value = py_resource_type.get_from_world(world, py, validity.clone())?;
        if matches!(py_resource_type, PyResourceType::Custom(_)) {
            Ok(Py::new(py, PyRes::with_validity(value.into_bound(py), validity))?.into_any())
        } else {
            Ok(value)
        }
    }

    pub fn insert_resource(&self, py: Python, resource: Bound<'_, PyAny>) -> PyResult<()> {
        world_commands::ErrorBoundary::new(self.world_ref()?).run(|| {
            self.check_valid()?;
            // Get the resource type from the instance
            let resource_type = resource.get_type();
            let py_resource_type = PyResourceType::try_from((&resource_type, py))?;

            // Convert the bound resource to a Py<PyAny>
            let resource_instance: Py<PyAny> = resource.unbind();
            self.check_native_asset_access("world.insert_resource()")?;

            // Insert the resource into the world
            let mut world = self.world_mut()?;
            py_resource_type.insert_into_world(&mut world, py, resource_instance)
        })
    }

    pub fn remove_resource(
        &self,
        py: Python,
        resource_type: Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        world_commands::ErrorBoundary::new(self.world_ref()?).run(|| {
            self.check_valid()?;
            let type_obj = resource_type.cast::<PyType>().map_err(|_| {
                PyTypeError::new_err(
                    "remove_resource expects a resource type (class), not an instance",
                )
            })?;
            let resource = PyResourceType::try_from((type_obj, py))?;
            self.check_native_asset_access("world.remove_resource()")?;
            resource.take_from_world(&mut *self.world_mut()?, py)
        })
    }

    pub fn register_resource(&self, py: Python, resource: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        world_commands::ErrorBoundary::new(self.world_ref()?).run(|| {
            self.check_valid()?;

            // Extract the resource type
            let type_obj: Bound<'_, PyType> = resource.extract()?;
            let py_resource_type = PyResourceType::try_from((&type_obj, py))?;

            // Register the resource type and get its ComponentId
            let component_id = match py_resource_type {
                // Built-in Bevy resources are already registered by their respective plugins
                // Calling register_resource on them is not supported - use init_resource or insert_resource instead
                PyResourceType::AssetServer => {
                    return Err(PyRuntimeError::new_err(format!(
                        "Cannot register built-in resource type {}. Built-in resources are automatically registered by Bevy plugins. Use insert_resource() or init_resource() instead.",
                        type_obj.name()?
                    )));
                }
                PyResourceType::Custom(type_ptr) => {
                    self.check_native_asset_access("world.register_resource()")?;
                    // Register the custom resource
                    register_custom_resource(&mut *self.world_mut()?, type_ptr, py)
                }
                PyResourceType::Dynamic(_) => {
                    // Dynamic resources are registered via their bridges
                    return Err(PyRuntimeError::new_err(format!(
                        "Cannot register dynamic resource type {}. Dynamic resources are automatically registered by their bridges. Use insert_resource() instead.",
                        type_obj.name()?
                    )));
                }
            };

            // Return the ComponentId
            let py_component_id = Py::new(py, PyComponentId(component_id))?;
            Ok(py_component_id.into_any())
        })
    }

    pub fn init_resource(&self, py: Python, resource: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        self.check_valid()?;
        // Extract the resource type
        let type_obj: Bound<'_, PyType> = resource.extract()?;
        let py_resource_type = PyResourceType::try_from((&type_obj, py))?;

        // Create a default instance by calling the type with no arguments
        let resource_instance = Self::default_resource_instance(py, &type_obj)?;
        self.check_native_asset_access("world.init_resource()")?;

        // Insert the resource into the world
        self.insert_resource(py, resource_instance.into_bound(py))?;

        // Get the world again to look up ComponentId
        let world = self.world_mut()?;

        // Get the ComponentId for this resource type
        let component_id = py_resource_type.get_component_id(&world).ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "Resource type {} was inserted but ComponentId not found. This is a bug.",
                type_obj
                    .name()
                    .unwrap_or_else(|_| pyo3::types::PyString::new(py, "Resource"))
            ))
        })?;

        // Return the ComponentId
        let py_component_id = Py::new(py, PyComponentId(component_id))?;
        Ok(py_component_id.into_any())
    }

    pub fn contains_resource(&self, py: Python, resource: Bound<'_, PyAny>) -> PyResult<bool> {
        self.check_valid()?;

        if resource.get_type().is(PyAssetTypeParam::type_object(py)) {
            let asset_param = resource.extract::<PyAssetTypeParam>()?;
            // SAFETY: `check_valid` above fences this momentary shared access.
            let world = unsafe { &*self.world_ptr() };
            let Some(bridge) = global_registry::get_asset_bridge_by_py_type(asset_param.type_ptr())
            else {
                return Ok(false);
            };
            return Ok(bridge
                .resource_id(world)
                .is_some_and(|resource_id| world.contains_resource_by_id(resource_id)));
        }

        // Extract the resource type
        let type_obj: Bound<'_, PyType> = resource.extract()?;
        let py_resource_type = PyResourceType::try_from((&type_obj, py))?;

        let world = unsafe { &*self.world_ptr() };

        match py_resource_type {
            PyResourceType::AssetServer => {
                Ok(world.contains_resource::<bevy::asset::AssetServer>())
            }
            PyResourceType::Custom(type_ptr) => Ok(world
                .get_resource::<ResourceRegistry>()
                .and_then(|registry| registry.get(type_ptr as usize))
                .is_some_and(|component_id| world.contains_resource_by_id(component_id))),
            PyResourceType::Dynamic(type_ptr) => {
                // Check via bridge
                if let Some(bridge) =
                    pybevy_core::registry::global_registry::get_resource_bridge_by_py_type(type_ptr)
                {
                    Ok(bridge.contains_in_world(world))
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// Return the stable entity allocated for a resource, if one exists.
    pub fn resource_entity(
        &self,
        py: Python,
        resource: Bound<'_, PyAny>,
    ) -> PyResult<Option<PyEntity>> {
        self.check_valid()?;

        if resource.get_type().is(PyAssetTypeParam::type_object(py)) {
            let asset_param = resource.extract::<PyAssetTypeParam>()?;
            // SAFETY: `check_valid` above fences this momentary shared access.
            let world = unsafe { &*self.world_ptr() };
            let Some(bridge) = global_registry::get_asset_bridge_by_py_type(asset_param.type_ptr())
            else {
                return Ok(None);
            };
            return Ok(bridge
                .resource_id(world)
                .and_then(|component_id| world.resource_entities().get(component_id))
                .map(PyEntity));
        }

        let type_obj: Bound<'_, PyType> = resource.extract()?;
        let resource_type = PyResourceType::try_from((&type_obj, py))?;
        let world = unsafe { &*self.world_ptr() };

        Ok(resource_type
            .get_component_id(world)
            .and_then(|component_id| world.resource_entities().get(component_id))
            .map(PyEntity))
    }

    /// Iterate over every resource component ID with an allocated Bevy entity.
    pub fn resource_entities<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        self.check_valid()?;
        let world = unsafe { &*self.world_ptr() };
        let entries = world
            .resource_entities()
            .iter()
            .map(|(component_id, entity)| (PyComponentId(component_id), PyEntity(entity)))
            .collect::<Vec<_>>();
        let entries = PyList::new(py, entries)?;
        Ok(entries.call_method0("__iter__")?.unbind())
    }

    /// Get the last system error, if any (PyBevy internal API).
    ///
    /// Returns a tuple of (error_message, traceback) or None if no error.
    #[pyo3(name = "_get_last_error")]
    pub fn get_last_error(&self) -> PyResult<Option<(String, Option<String>)>> {
        self.check_valid()?;
        let world = unsafe { &*self.world_ptr() };
        match world.get_resource::<pybevy_core::LastSystemError>() {
            Some(last_error) if last_error.error.is_some() => Ok(Some((
                last_error.error.clone().unwrap(),
                last_error.traceback.clone(),
            ))),
            _ => Ok(None),
        }
    }

    pub fn iter_entities(&self) -> PyResult<Vec<PyEntity>> {
        self.check_valid()?;
        let world = unsafe { &*self.world_ptr() };
        Ok(world
            .iter_entities()
            .map(|entity| PyEntity(entity.id()))
            .collect())
    }

    pub fn entity(
        pyself: PyRef<'_, Self>,
        entity: &Bound<'_, PyAny>,
    ) -> PyResult<PyEntityCommands> {
        let entity = &extract_entity_from_any(entity)?;
        pyself.check_valid()?;
        let world = pyself.world_mut()?;
        world.get_entity(entity.0).map_err(|_| {
            PyRuntimeError::new_err(pybevy_core::public_error::entity_does_not_exist(entity.0))
        })?;
        drop(world);
        Ok(PyEntityCommands::with_world(entity.0, pyself))
    }

    pub fn query(&self, py: Python, param: PyQueryParam) -> PyResult<PyQueryIter> {
        self.check_valid()?;
        let validity = self.validity.clone().unwrap_or_default();
        let mut world = self.world_mut()?; // SAFETY: validity is this World's lifetime fence.
        Ok(unsafe { PyQueryIter::from_world(&mut world, param, validity, py) })
    }

    pub fn commands(pyself: Py<Self>, py: Python) -> PyResult<PyCommands> {
        {
            let borrowed = pyself.borrow(py);
            borrowed.check_valid()?;
        }

        let world_ptr = pyself.borrow(py).world_ptr();
        let validity = pyself.borrow(py).validity.clone().unwrap_or_default();

        // SAFETY: Commands retains this World and its checked exclusive validity.
        let py_commands = unsafe { PyCommands::from_world_queue(world_ptr, pyself, validity) };
        Ok(py_commands)
    }

    pub fn flush(&self, py: Python<'_>) -> PyResult<()> {
        self.check_native_asset_access("world.flush()")?;
        let mut world = self.world_mut()?;
        let _suspension = self
            .validity
            .as_ref()
            .map(ValidityFlag::suspend)
            .transpose()?;
        world.flush();
        world_commands::raise_errors(&world, py)
    }

    #[pyo3(signature = (*components, count=None, batch=None))]
    pub fn spawn_batch(
        &self,
        py: Python,
        components: &Bound<'_, PyTuple>,
        count: Option<usize>,
        batch: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<PyEntity>> {
        world_commands::ErrorBoundary::new(self.world_ref()?).run(|| {
            self.check_valid()?;
            let prepared = if let Some(batch) = batch {
                if !components.is_empty() || count.is_some() {
                    return Err(PyTypeError::new_err(WORLD_BATCH_ARGUMENTS));
                }
                prepare_iter_batch(py, batch)?
            } else if components.len() == 1
                && count.is_none()
                && !components.get_item(0)?.is_instance_of::<PyComponent>()
                && !components
                    .get_item(0)?
                    .is_instance_of::<PyCustomComponentBatch>()
                && global_registry::get_batch_bridge_by_py_type(
                    components.get_item(0)?.get_type().as_type_ptr(),
                )
                .is_none()
                && components.get_item(0)?.hasattr("__iter__")?
            {
                prepare_iter_batch(py, &components.get_item(0)?)?
            } else {
                vec![SpawnBatchCommand::new(py, components, count)?]
            };
            self.check_native_asset_access("world.spawn_batch()")?;
            let mut entities = Vec::new();
            let mut world = self.world_mut()?;
            for command in prepared {
                entities.extend(command.apply(&mut world)?.into_iter().map(PyEntity));
            }
            Ok(entities)
        })
    }

    pub fn register_component(
        &self,
        py: Python,
        component: Bound<'_, PyAny>,
    ) -> PyResult<PyComponentId> {
        world_commands::ErrorBoundary::new(self.world_ref()?).run(|| {
            self.check_valid()?;
            let type_obj: Bound<'_, PyType> = component.extract()?;
            let component_type = PyComponentType::try_from((&type_obj, py))?;
            let mut world = self.world_mut()?;
            let id = match component_type {
                PyComponentType::Custom(type_ptr) => {
                    register_custom_component(&mut world, type_ptr, py)
                }
                PyComponentType::Dynamic(type_ptr) => {
                    global_registry::get_bridge_by_py_type(type_ptr)
                        .ok_or_else(|| PyTypeError::new_err(COMPONENT_BRIDGE_NOT_FOUND))?
                        .register(&mut world)
                }
                PyComponentType::Resource(_) => PyResourceType::try_from((&type_obj, py))?
                    .register_component_id(&mut world, py)
                    .ok_or_else(|| PyTypeError::new_err(RESOURCE_BRIDGE_NOT_FOUND))?,
            };
            Ok(PyComponentId(id))
        })
    }

    pub fn component_id(
        &self,
        py: Python,
        component: Bound<'_, PyAny>,
    ) -> PyResult<Option<PyComponentId>> {
        self.check_valid()?;

        if component.get_type().is(PyAssetTypeParam::type_object(py)) {
            let asset_param = component.extract::<PyAssetTypeParam>()?;
            let Some(bridge) = global_registry::get_asset_bridge_by_py_type(asset_param.type_ptr())
            else {
                return Ok(None);
            };
            let mut world = self.world_mut()?;
            return Ok(Some(PyComponentId(bridge.register_resource_id(&mut world))));
        }

        let type_obj: Bound<'_, PyType> = component.extract()?;

        // Try to convert to PyComponentType
        let py_component_type = match PyComponentType::try_from((&type_obj, py)) {
            Ok(ty) => ty,
            Err(_) => return Ok(None), // Not a registered component type
        };

        // For built-in components, we need to get their ComponentId from the world
        // For custom components, look in the ComponentRegistry
        match py_component_type {
            PyComponentType::Custom(type_ptr) => {
                let world = unsafe { &*self.world_ptr() };
                if let Some(registry) = world.get_resource::<ComponentRegistry>() {
                    // Look up the component ID in the registry
                    if let Some(component_id) = registry.get(type_ptr as usize) {
                        return Ok(Some(PyComponentId(component_id)));
                    }
                }
                Ok(None)
            }
            PyComponentType::Dynamic(type_ptr) => {
                if let Some(bridge) =
                    pybevy_core::registry::global_registry::get_bridge_by_py_type(type_ptr)
                {
                    let mut world = self.world_mut()?;
                    let component_id = bridge.register(&mut world);
                    Ok(Some(PyComponentId(component_id)))
                } else {
                    Ok(None)
                }
            }
            PyComponentType::Resource(_) => {
                let mut world = self.world_mut()?;
                Ok(PyResourceType::try_from((&type_obj, py))?
                    .register_component_id(&mut world, py)
                    .map(PyComponentId))
            }
        }
    }

    pub fn trigger(&self, py: Python, event: Bound<'_, PyAny>) -> PyResult<()> {
        world_commands::ErrorBoundary::new(self.world_ref()?).run(|| {
            self.check_valid()?;

            // Verify the event is a subclass of Event
            let event_type = event.get_type();
            if !event_type.is_subclass_of::<PyEvent>()? {
                return Err(PyRuntimeError::new_err(
                    "trigger() requires an Event subclass instance",
                ));
            }

            // Check if this is an entity-targeted event (has 'entity' field)
            let target_entity = if event.hasattr("entity")? {
                let entity_attr = event.getattr("entity")?;
                Some(entity_attr.extract::<PyEntity>()?.0)
            } else {
                None
            };
            self.check_native_asset_access("world.trigger()")?;

            let mut world = self.world_mut()?;
            let observers = world
                .get_resource::<ObserverRegistry>()
                .map(|registry| registry.snapshot_user_event(&event, target_entity))
                .unwrap_or_default();

            for observer_entry in observers {
                if !ObserverRegistry::matches_user_filter(&observer_entry, &world, target_entity) {
                    continue;
                }

                let on_param = Py::new(
                    py,
                    PyOn {
                        event_data: event.clone().unbind(),
                        entity: target_entity,
                    },
                )?;

                ObserverRegistry::invoke(
                    &observer_entry,
                    &mut world,
                    &on_param,
                    target_entity,
                    ErrorPolicy::PropagateToCaller,
                )?;
            }

            Ok(())
        })
    }

    pub fn write_message(&self, py: Python, message: Py<PyAny>) -> PyResult<Option<PyMessageId>> {
        world_commands::ErrorBoundary::new(self.world_ref()?).run(|| {
            self.check_valid()?;

            let message_type = PyMessageType::from_message_type(&message.bind(py).get_type())?.0;
            let validity = self.validity.clone().unwrap_or_default();
            let mut world = self.world_mut()?;
            if let MessageType::Custom(message_class) = &message_type {
                let type_ptr = message_class.bind(py).as_type_ptr();
                if !python_message_is_registered(&world, type_ptr) {
                    eprintln!(
                        "{}",
                        unregistered_message_write(message_class.bind(py).name()?)
                    );
                    return Ok(None);
                }
                let resolved = resolve_from_world(&world, type_ptr)?;
                return PyMessageWriter::python(message_type, resolved, validity, None)
                    .write(py, message)
                    .map(Some);
            }

            if let MessageType::Dynamic(type_ptr) = &message_type {
                let bridge =
                    global_registry::get_message_bridge_by_py_type(*type_ptr).ok_or_else(|| {
                        PyTypeError::new_err("Message type not registered in global registry")
                    })?;
                if !bridge.is_read_only()
                    && !bridge
                        .resource_id(&world)
                        .is_some_and(|resource_id| world.get_resource_by_id(resource_id).is_some())
                {
                    eprintln!("{}", unregistered_message_write(bridge.name()));
                    return Ok(None);
                }
            }

            let cell = world.as_unsafe_world_cell();
            // SAFETY: World is an exclusive system parameter and the writer is used
            // synchronously before this method returns. The shared validity flag
            // fences the lifetime-erased cell exactly as it does for system writers.
            let message_world = unsafe { MessageWorld::new(cell, validity) };
            PyMessageWriter::native(message_type, message_world, None)
                .write(py, message)
                .map(Some)
        })
    }

    pub fn add_observer(&self, py: Python, observer: Bound<'_, PyAny>) -> PyResult<Py<PyEntity>> {
        world_commands::ErrorBoundary::new(self.world_ref()?).run(|| {
            self.check_valid()?;
            self.check_native_asset_access("world.add_observer()")?;
            let mut world = self.world_mut()?;
            let observer_entity = ObserverRegistry::register_observer(py, &observer, &mut world)?;

            Py::new(py, PyEntity(observer_entity))
        })
    }

    pub fn despawn_observer(&self, observer_entity: &Bound<'_, PyAny>) -> PyResult<()> {
        world_commands::ErrorBoundary::new(self.world_ref()?).run(|| {
            let observer_entity = &extract_entity_from_any(observer_entity)?;
            self.check_valid()?;
            self.check_native_asset_access("world.despawn_observer()")?;
            let mut world = self.world_mut()?;
            ObserverRegistry::despawn_observer(observer_entity.0, &mut world)?;

            Ok(())
        })
    }

    pub fn get(
        &self,
        py: Python,
        entity: &Bound<'_, PyAny>,
        component_type: Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.get_component(py, &extract_entity_from_any(entity)?, component_type)
    }

    pub fn get_mut(
        &self,
        py: Python,
        entity: &Bound<'_, PyAny>,
        component_type: Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.get_component_mut(py, &extract_entity_from_any(entity)?, component_type)
    }

    pub fn run_schedule(&self, py: Python, label: Bound<'_, PyAny>) -> PyResult<()> {
        world_commands::ErrorBoundary::new(self.world_ref()?).run(|| {
            self.check_valid()?;

            // Try PyStage first (SimTick, Update, etc.)
            if let Ok(stage) = label.extract::<PyStage>() {
                self.check_native_asset_access("world.run_schedule()")?;
                // Cast to usize to cross the GIL boundary (raw pointers aren't Ungil).
                // SAFETY: we have exclusive World access (SystemStateFlags::EXCLUSIVE)
                // and the pointer is valid for the system's lifetime (ValidityFlag).
                let mut world = self.world_mut()?;
                let world_addr = &mut *world as *mut World as usize;
                let _suspension = self
                    .validity
                    .as_ref()
                    .map(ValidityFlag::suspend)
                    .transpose()?;

                // Release GIL before running the schedule to avoid deadlock:
                // this exclusive system holds the GIL, but inner Python systems
                // spawned by run_schedule() need to acquire it.
                py.detach(move || {
                    // SAFETY: the retained mutation guard excludes traversal until execution ends.
                    let world = unsafe { &mut *(world_addr as *mut World) };
                    stage.run_on_world(world);
                    Ok::<(), PyErr>(())
                })?;

                return Ok(());
            }

            // State-based schedule labels (OnEnter, OnExit, OnTransition)
            let mut world = self.world_mut()?;

            let _suspension = self
                .validity
                .as_ref()
                .map(ValidityFlag::suspend)
                .transpose()?;

            let schedule = if let Ok(on_enter) = label.cast::<PyOnEnterSchedule>() {
                EitherStateSchedule::State(canonicalize_state_schedule_label(
                    &world,
                    on_enter.borrow().to_bevy_label(py)?,
                ))
            } else if let Ok(on_exit) = label.cast::<PyOnExitSchedule>() {
                EitherStateSchedule::State(canonicalize_state_schedule_label(
                    &world,
                    on_exit.borrow().to_bevy_label(py)?,
                ))
            } else if let Ok(on_transition) = label.cast::<PyOnTransitionSchedule>() {
                EitherStateSchedule::Transition(canonicalize_transition_schedule_label(
                    &world,
                    on_transition.borrow().to_bevy_label(py)?,
                ))
            } else {
                return Err(PyTypeError::new_err(
                    "run_schedule() requires a Stage, OnEnter, OnExit, or OnTransition schedule label",
                ));
            };

            ensure_no_live_asset_access(&world, "world.run_schedule()")
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            let world_addr = &mut *world as *mut World as usize;
            py.detach(move || {
                let world = unsafe { &mut *(world_addr as *mut World) };
                let result = match schedule {
                    EitherStateSchedule::State(label) => world.try_run_schedule(label),
                    EitherStateSchedule::Transition(label) => world.try_run_schedule(label),
                };
                result.map_err(|error| {
                    PyRuntimeError::new_err(format!("Failed to run state schedule: {error}"))
                })
            })?;

            Ok(())
        })
    }

    /// Run a system function once immediately on this world.
    ///
    /// This is useful for testing and debugging, allowing you to run a system
    /// without adding it to a schedule. The system is created, run once, and
    /// then discarded.
    ///
    /// Note: Unlike scheduled systems, `run_system_once` does not preserve
    /// local state between calls - each call creates a fresh system instance.
    /// Change detection may not work as expected.
    ///
    /// # Example
    /// ```python
    /// def my_system(query: Query[Transform]) -> None:
    ///     for transform in query:
    ///         print(transform.translation)
    ///
    /// world.run_system_once(my_system)
    /// ```
    pub fn run_system_once(&self, func: Bound<'_, PyAny>) -> PyResult<()> {
        self.check_valid()?;
        let py = func.py();

        // Create shared error state for the system
        let error_state: Arc<Mutex<Vec<PyErr>>> = Arc::new(Mutex::new(Vec::new()));

        // Get mutable access to the world
        let mut world = self.world_mut()?;

        let _suspension = self
            .validity
            .as_ref()
            .map(ValidityFlag::suspend)
            .transpose()?;

        // Read current hot-reload generation so the system's expected_generation
        // matches and run_unsafe doesn't silently skip execution.
        let generation = world
            .get_resource::<HotReloadGeneration>()
            .map(|res| res.current)
            .unwrap_or(0);

        // Create a DynamicSystem from the Python function
        // Use SystemStage::UpdateOrLast as a default since this is a one-shot execution
        // One-shot exclusive execution; errors return directly to the caller, so a
        // throwaway error buffer (no LastSystemError drain) is sufficient.
        let mut system = new_main_one_shot_system(
            func.unbind(),
            generation,
            error_state.clone(),
            Arc::new(Mutex::new(None)),
            SystemStage::UpdateOrLast,
        )?;

        ensure_no_live_asset_access(&world, "world.run_system_once()")
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;

        // Flush any deferred commands from prior operations (e.g., entities
        // spawned via MCP mutations) so queries inside the system see them.
        world.flush();

        // Initialize the system (registers components, etc.)
        world_commands::raise_errors(&world, py)?;
        let _ = system.initialize(&mut world);

        // Create an UnsafeWorldCell for run_unsafe
        // SAFETY: We have exclusive access to the world through world_mut()
        let world_cell = world.as_unsafe_world_cell();

        // Run the system
        // SAFETY: We have exclusive world access and the system was just initialized
        let result = match unsafe { system.run_unsafe((), world_cell) } {
            Err(RunSystemError::Skipped(error)) => {
                return Err(PyRuntimeError::new_err(one_shot_parameter_validation_skip(
                    error.message,
                )));
            }
            result => result,
        };

        // Apply any deferred commands from the system
        system.apply_deferred(&mut world);

        // Flush any commands that were queued through Commands parameter
        world.flush();

        // Check for any errors that occurred during system execution
        lock_or_recover(&error_state).extend(world_commands::take_errors(&world, py));
        let errors = std::mem::take(&mut *lock_or_recover(&error_state));
        world_commands::raise_collected(py, errors, WORLD_CALLBACK_COMMAND_ERRORS)?;

        // Check for system execution errors
        if let Err(e) = result {
            return Err(PyRuntimeError::new_err(format!(
                "System execution failed: {:?}",
                e
            )));
        }

        Ok(())
    }
}

/// Create a Python World wrapper from a raw world pointer and validity flag.
/// Used by MCP execute_python to inject `world` into the Python execution context.
pub fn create_world_wrapper(
    world_ptr: *mut World,
    validity: ValidityFlag,
    py: Python,
) -> PyResult<Py<PyAny>> {
    let py_world = unsafe { PyWorld::new(&mut *world_ptr, validity) };
    let obj = Py::new(py, py_world)?;
    Ok(obj.into_any())
}

// Internal helper methods for PyWorld
impl PyWorld {
    /// Get all known component types on an entity.
    /// This checks for all built-in component types that the entity has.
    pub(crate) fn get_entity_data_names(
        world: &World,
        entity: bevy::ecs::entity::Entity,
    ) -> Vec<PyComponentType> {
        let mut component_types: Vec<(bevy::ecs::component::ComponentId, PyComponentType)> =
            Vec::new();

        // Get entity reference
        let Ok(entity_ref) = world.get_entity(entity) else {
            return Vec::new(); // Entity doesn't exist
        };

        // Check all registered component bridges
        for bridge in pybevy_core::registry::global_registry::all_component_bridges() {
            if bridge.entity_contains(&entity_ref)
                && let Some(component_id) = world.components().get_id(bridge.bevy_type_id())
            {
                component_types
                    .push((component_id, PyComponentType::Dynamic(bridge.py_type_ptr())));
            }
        }

        for bridge in pybevy_core::registry::global_registry::all_resource_bridges() {
            if bridge.entity_contains(&entity_ref)
                && let Some(component_id) = bridge.resource_id(world)
            {
                component_types.push((
                    component_id,
                    PyComponentType::Resource(bridge.py_type_ptr()),
                ));
            }
        }

        // Check all registered custom Python components.
        // Without this, `On[Despawn, MyCustomComponent]` observers would never
        // fire because the dispatcher iterates this list to build event keys.
        if let Some(custom_info) = world.get_resource::<pybevy_core::CustomComponentInfo>() {
            for (component_id, entry) in custom_info.iter() {
                if entity_ref.contains_id(component_id) {
                    component_types.push((component_id, PyComponentType::Custom(entry.type_ptr)));
                }
            }
        }

        if let Some(custom_info) = world.get_resource::<pybevy_core::CustomResourceInfo>() {
            for (component_id, entry) in custom_info.iter() {
                if entity_ref.contains_id(component_id) {
                    component_types.push((component_id, PyComponentType::Resource(entry.type_ptr)));
                }
            }
        }

        // Inventory and hash-backed registry iteration are not ordering
        // contracts. ComponentId is World-local and deterministic for the
        // registration sequence, so it gives recursive/event-major snapshots
        // one stable order across repeated processes.
        component_types.sort_by_key(|(component_id, _)| component_id.index());
        component_types.dedup_by_key(|(component_id, _)| *component_id);
        component_types
            .into_iter()
            .map(|(_, component_type)| component_type)
            .collect()
    }
}

impl PyWorld {
    pub(crate) fn get_component(
        &self,
        py: Python,
        entity: &PyEntity,
        component_type: Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.check_valid()?;
        let world = self.world_mut()?;

        if !world.entities().contains(entity.0) {
            return Ok(None);
        }

        // Get component type
        let comp_type =
            PyComponentType::try_from((component_type.cast::<pyo3::types::PyType>()?, py))?;

        // Create validity flag for the borrowed component
        let validity = self
            .validity
            .clone()
            .unwrap_or_else(ValidityFlag::new_read)
            .with_access_mode(AccessMode::Read);

        match comp_type {
            PyComponentType::Dynamic(type_ptr) => {
                // Unregistered dynamic component type => the entity can't have it.
                // Match Bevy's `World::get`, which returns `None` for a missing or
                // unregistered component type.
                let Some(bridge) = global_registry::get_bridge_by_py_type(type_ptr) else {
                    return Ok(None);
                };

                // SAFETY: world_ptr comes from this live PyWorld and stays valid while
                // the returned handle's validity flag is active.
                unsafe { bridge.extract_from_entity_ref(entity.0, self.world_ptr(), validity, py) }
            }
            PyComponentType::Resource(type_ptr) => {
                if let Some(bridge) = global_registry::get_resource_bridge_by_py_type(type_ptr) {
                    // SAFETY: world_ptr comes from this live PyWorld, and the returned
                    // handle is fenced by the validity mode supplied above.
                    unsafe {
                        bridge.extract_from_entity_ref(entity.0, self.world_ptr(), validity, py)
                    }
                } else {
                    self.extract_custom_resource_component(py, entity.0, type_ptr, false)
                }
            }
            PyComponentType::Custom(type_ptr) => {
                self.extract_custom_component(py, entity.0, type_ptr, validity)
            }
        }
    }

    pub(crate) fn get_component_mut(
        &self,
        py: Python,
        entity: &PyEntity,
        component_type: Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.check_valid()?;
        let world = self.world_mut()?;

        if !world.entities().contains(entity.0) {
            return Ok(None);
        }

        let comp_type =
            PyComponentType::try_from((component_type.cast::<pyo3::types::PyType>()?, py))?;

        let validity = self
            .validity
            .clone()
            .unwrap_or_else(ValidityFlag::new_write)
            .with_access_mode(AccessMode::Write);

        match comp_type {
            PyComponentType::Dynamic(type_ptr) => {
                // Unregistered dynamic component type => the entity can't have it.
                // Match Bevy's `World::get`, which returns `None` for a missing or
                // unregistered component type.
                let Some(bridge) = global_registry::get_bridge_by_py_type(type_ptr) else {
                    return Ok(None);
                };

                // SAFETY: world_ptr comes from this live PyWorld and stays valid while
                // the returned handle's validity flag is active.
                unsafe { bridge.extract_from_entity_mut(entity.0, self.world_ptr(), validity, py) }
            }
            PyComponentType::Resource(type_ptr) => {
                if let Some(bridge) = global_registry::get_resource_bridge_by_py_type(type_ptr) {
                    // SAFETY: world_ptr comes from this live PyWorld, and the returned
                    // mutable handle is fenced by the write validity mode supplied above.
                    unsafe {
                        bridge.extract_from_entity_mut(entity.0, self.world_ptr(), validity, py)
                    }
                } else {
                    self.extract_custom_resource_component(py, entity.0, type_ptr, true)
                }
            }
            PyComponentType::Custom(type_ptr) => {
                self.extract_custom_component(py, entity.0, type_ptr, validity)
            }
        }
    }
}
