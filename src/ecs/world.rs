//! # PyWorld: Bevy ECS World Access
//!
//! This module provides Python bindings for Bevy's World, the central ECS container.
//!
//! ## Design Notes
//!
//! ### Why PyWorld Uses Custom Storage Instead of ComponentStorage
//!
//! PyWorld has fundamentally different semantics from components and uses a custom
//! `WorldStorage` enum instead of reusing `ComponentStorage`:
//!
//! 1. **Interior Mutability**: Owned worlds use `Box<UnsafeCell<World>>` since PyWorld
//!    methods take `&self` but need `&mut World` access. Components don't need this
//!    because they're accessed through Query[T] (immutable) or Query[Mut[T]] (mutable).
//!
//! 2. **Conditional Validity**: Owned worlds are never invalidated (they live until GC),
//!    while owned components are invalidated on drop to prevent use-after-free of field
//!    borrows. This semantic difference requires `Option<ValidityFlag>`.
//!
//! 3. **Unique Operations**: PyWorld has spawn/despawn/trigger/resource operations that
//!    don't fit the generic storage abstraction designed for component field access.
//!
//! ### Temporary World Access Helper
//!
//! For temporary World access with automatic validity management, use `PyWorld::with_temporary()`:
//!
//! ```rust,ignore
//! PyWorld::with_temporary(app.world_mut(), py, |py_world| {
//!     py_world.insert_resource(py, resource)?;
//!     Ok(result)
//! })?;
//! ```
//!
//! This eliminates the boilerplate of creating ValidityFlag and ValidityGuard manually.

use std::{
    cell::UnsafeCell,
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{system::System, world::World},
    prelude::*,
};
use pybevy_core::registry::global_registry;
use pybevy_reload::{HotReloadGeneration, SystemStage};
use pyo3::{
    PyTypeInfo,
    exceptions::{PyRuntimeError, PyTypeError},
    prelude::*,
    types::{PyTuple, PyType},
};

use crate::{
    app::PyStage,
    assets::{asset_type::PyAssetTypeParam, assets::PyAssets},
    ecs::{
        PyEntity,
        commands::PyCommands,
        component::PyComponentId,
        component_layout::{ComponentLayoutExt, ComponentStorageType, ComponentStorageTypeExt},
        component_type::{ComponentRegistry, PyComponentType, register_custom_component},
        custom_component::PyCustomComponent,
        dynamic_system::{DynamicSystem, execute_system_func},
        entity_commands::PyEntityCommands,
        helpers::validity_guard::{AccessMode, ValidityFlag, ValidityFlagWithMode, ValidityGuard},
        lazy_wrapper_proxy::PyLazyWrapperProxy,
        observer::{BundleFilter, EventType, PyEvent, PyOn},
        observer_registry::ObserverRegistry,
        resource_type::{
            PyResourceStorage, PyResourceType, ResourceRegistry, register_custom_resource,
        },
        state::{PyOnEnterSchedule, PyOnExitSchedule, PyOnTransitionSchedule},
    },
};

/// Internal storage for World - either owned or borrowed
enum WorldStorage {
    /// An owned World instance (using UnsafeCell for interior mutability)
    Owned(Box<UnsafeCell<World>>),
    /// A borrowed mutable reference to a World (as a raw pointer)
    Borrowed(*mut World),
}

/// Represents exclusive access to the Bevy ECS World within a system.
/// This is passed to Python systems that request World access.
///
/// Note: World access requires an exclusive system, which prevents parallel execution.
#[pyclass(name = "World")]
pub struct PyWorld {
    storage: WorldStorage,
    // Runtime validity check - prevents use after system execution
    // None for owned worlds (created from Python), Some for borrowed (system params)
    validity: Option<ValidityFlag>,
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

impl PyWorld {
    /// Create a new PyWorld wrapper around a mutable World reference.
    ///
    /// # Safety
    /// The world pointer must be valid for the lifetime of this PyWorld instance.
    /// This should only be created within the system's run_unsafe and dropped before returning.
    pub(crate) unsafe fn new(world: &mut World, validity: ValidityFlag) -> Self {
        Self {
            storage: WorldStorage::Borrowed(world as *mut World),
            validity: Some(validity),
        }
    }

    /// Create a new PyWorld that owns its World
    pub(crate) fn new_owned(world: World) -> Self {
        Self {
            storage: WorldStorage::Owned(Box::new(UnsafeCell::new(world))),
            validity: None, // Owned worlds don't need validity checking
        }
    }

    /// Check if this World instance is still valid for use
    pub(crate) fn check_valid(&self) -> PyResult<()> {
        if let Some(ref validity) = self.validity {
            Ok(validity.check()?)
        } else {
            Ok(()) // Owned worlds are always valid
        }
    }

    /// Get a clone of the validity flag for sharing with child structures
    /// Returns None for owned worlds (they don't need validity tracking)
    pub(crate) fn validity(&self) -> Option<ValidityFlag> {
        self.validity.clone()
    }

    // validity-checked raw pointer access, see docs/safety.md
    #[allow(clippy::mut_from_ref)]
    pub(crate) fn world_mut(&self) -> PyResult<&mut World> {
        self.check_valid()?;
        Ok(match &self.storage {
            WorldStorage::Owned(boxed) => unsafe { &mut *boxed.get() },
            WorldStorage::Borrowed(ptr) => unsafe { &mut **ptr },
        })
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
        }
    }

    pub(crate) fn world_ptr(&self) -> *mut World {
        match &self.storage {
            WorldStorage::Owned(boxed) => boxed.get(),
            WorldStorage::Borrowed(ptr) => *ptr,
        }
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
    pub(crate) fn with_temporary<F, R>(world: &mut World, _py: Python, f: F) -> PyResult<R>
    where
        F: FnOnce(&PyWorld) -> PyResult<R>,
    {
        let validity = ValidityFlag::new();
        let _guard = ValidityGuard::new(validity.clone());
        let py_world = unsafe { PyWorld::new(world, validity) };
        f(&py_world)
    }

    /// Internal helper to get Assets resource for a specific asset type.
    fn get_assets_resource(
        &self,
        py: Python,
        type_ptr: *const pyo3::ffi::PyTypeObject,
    ) -> PyResult<Py<PyAny>> {
        let world_ptr = self.world_ptr();
        let validity = self.validity.clone().unwrap_or_default();

        // Create PyAssets wrapper for the specified asset type
        // When called from World.resource(), assume mutable access (for backwards compatibility)
        // SAFETY: `world_ptr` is valid while this PyWorld is valid; the derived cell is
        // fenced by the same `validity` flag. PyAssets only reaches the `Assets<T>` resource.
        let cell = unsafe { (*world_ptr).as_unsafe_world_cell() };
        let py_assets = unsafe {
            Py::new(
                py,
                (
                    PyAssets::new(type_ptr, None, cell, validity, true),
                    super::resource::PyResource,
                ),
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
        let world = self.world_mut()?;

        let component_id = {
            // No ComponentRegistry resource => no custom components registered in
            // this world, so the entity cannot have this one. Match Bevy's
            // `World::get`, which returns `None` (not an error) for a missing or
            // unregistered component type.
            let Some(registry) = world.get_resource::<ComponentRegistry>() else {
                return Ok(None);
            };
            match registry.get(type_ptr) {
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
                    )
                };

                let py_obj = Py::new(py, proxy).expect("Failed to create lazy wrapper proxy");
                Ok(Some(py_obj.into_any()))
            }
            ComponentStorageType::PyObject => {
                let world_ptr = self.world_ptr();
                let entity_ref = world.entity(entity_id);
                let untyped_ptr = entity_ref
                    .get_by_id(component_id)
                    .expect("Component existence already verified")
                    .as_ptr();

                let py_obj_ptr = unsafe {
                    let py_any_ref = &*(untyped_ptr as *const Py<PyAny>);
                    py_any_ref.as_ptr()
                };

                let custom_comp = PyCustomComponent::from_borrowed(
                    py_obj_ptr,
                    validity,
                    component_id,
                    entity_id,
                    world_ptr,
                );

                let py_obj = Py::new(py, (custom_comp, crate::ecs::component::PyComponent))
                    .expect("Failed to create PyCustomComponent");
                Ok(Some(py_obj.into_any()))
            }
        }
    }
}

#[pymethods]
impl PyWorld {
    /// Create a new owned World from Python
    #[new]
    pub fn py_new() -> Self {
        Self::new_owned(World::new())
    }

    pub fn spawn_empty(&self, _py: Python<'_>) -> PyResult<PyEntityCommands> {
        self.check_valid()?;
        let world = self.world_mut()?;
        let entity = world.spawn_empty().id();
        Ok(PyEntityCommands::with_world(entity, self))
    }

    #[pyo3(signature = (*components))]
    pub fn spawn(&self, py: Python, components: &Bound<'_, PyTuple>) -> PyResult<PyEntityCommands> {
        self.check_valid()?;

        let world = self.world_mut()?;

        let entity_id = world.spawn_empty().id();

        // Create a temporary PyCommands wrapper around this world to reuse component insertion logic
        let world_ptr = self.world_ptr();
        let validity = self.validity.clone().unwrap_or_default();

        // SAFETY: We're creating a temporary PyCommands that will be used immediately
        // and dropped before returning, so the world pointer remains valid
        let temp_commands = unsafe { PyCommands::from_world_temporary(world_ptr, validity) };

        // Collect component types for lifecycle events
        let mut component_types = Vec::new();
        for component in components.iter() {
            let component_type = component.get_type();
            if let Ok(comp_type) = PyComponentType::try_from((&component_type, py)) {
                component_types.push(comp_type);
            }
        }

        // Insert components using existing helper
        crate::ecs::commands::insert_components_to_entity_helper(
            &temp_commands,
            py,
            entity_id,
            components,
        )?;

        // Trigger Add lifecycle events for added components
        if !component_types.is_empty() {
            Self::trigger_lifecycle_events_for_add(world_ptr, entity_id, &component_types);
        }

        Ok(PyEntityCommands::with_world(entity_id, self))
    }

    /// Despawn an entity
    pub fn despawn(&self, entity: &PyEntity) -> PyResult<()> {
        self.check_valid()?;
        let world_ptr = self.world_ptr();
        let world = self.world_mut()?;

        // Collect component types before despawning
        let component_types = Self::get_entity_data_names(world, entity.0);

        // Clean up any per-entity observers watching this entity
        ObserverRegistry::cleanup_on_entity_despawn(entity.0, world);

        // Despawn the entity
        world.despawn(entity.0);

        // Trigger Despawn lifecycle events
        if !component_types.is_empty() {
            Self::trigger_lifecycle_events_for_despawn(world_ptr, entity.0, &component_types);
        }

        Ok(())
    }

    /// Get resource from the world
    pub fn resource(&self, py: Python, resource: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        self.check_valid()?;

        // Check if this is an Assets[T] parameter
        if resource.get_type().is(PyAssetTypeParam::type_object(py)) {
            let asset_param = resource.extract::<PyAssetTypeParam>()?;
            // Return the Assets resource for this asset type
            return self.get_assets_resource(py, asset_param.type_ptr());
        }

        // Extract the resource type
        let type_obj: Bound<'_, PyType> = resource.extract()?;
        let py_resource_type = PyResourceType::try_from((&type_obj, py))?;

        // Get the world reference
        let world = unsafe { &*self.world_ptr() };

        // Get validity flag (use a new one if this is an owned world)
        let validity = self.validity.clone().unwrap_or_default();

        // Retrieve the resource from the world
        py_resource_type.get_from_world(world, py, validity)
    }

    pub fn insert_resource(&self, py: Python, resource: Bound<'_, PyAny>) -> PyResult<()> {
        self.check_valid()?;
        // Get the resource type from the instance
        let resource_type = resource.get_type();
        let py_resource_type = PyResourceType::try_from((&resource_type, py))?;

        // Convert the bound resource to a Py<PyAny>
        let resource_instance: Py<PyAny> = resource.unbind();

        // Insert the resource into the world
        let world = self.world_mut()?;
        py_resource_type.insert_into_world(world, py, resource_instance)
    }

    pub fn register_resource(&self, py: Python, resource: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        self.check_valid()?;

        // Extract the resource type
        let type_obj: Bound<'_, PyType> = resource.extract()?;
        let py_resource_type = PyResourceType::try_from((&type_obj, py))?;

        let world = self.world_mut()?;

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
                // Get the resource name from the Python type
                let name = type_obj.name()?.to_string();

                // Register the custom resource
                register_custom_resource(world, type_ptr, name)
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
    }

    pub fn init_resource(&self, py: Python, resource: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        self.check_valid()?;
        // Extract the resource type
        let type_obj: Bound<'_, PyType> = resource.extract()?;
        let py_resource_type = PyResourceType::try_from((&type_obj, py))?;

        // Create a default instance by calling the type with no arguments
        let resource_instance = type_obj.call0().map_err(|e| {
            // Provide a better error message if instantiation fails
            if e.to_string().contains("missing") && e.to_string().contains("required") {
                let type_name = type_obj.name().unwrap_or_else(|_| pyo3::types::PyString::new(py, "Resource"));
                pyo3::exceptions::PyTypeError::new_err(format!(
                    "Cannot initialize resource `{}` with default values: resource requires constructor arguments. Use `insert_resource()` instead.",
                    type_name
                ))
            } else {
                e
            }
        })?;

        // Insert the resource into the world
        self.insert_resource(py, resource_instance)?;

        // Get the world again to look up ComponentId
        let world = self.world_mut()?;

        // Get the ComponentId for this resource type
        let component_id = py_resource_type.get_component_id(world).ok_or_else(|| {
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

        // Extract the resource type
        let type_obj: Bound<'_, PyType> = resource.extract()?;
        let py_resource_type = PyResourceType::try_from((&type_obj, py))?;

        let world = unsafe { &*self.world_ptr() };

        match py_resource_type {
            PyResourceType::AssetServer => {
                Ok(world.contains_resource::<bevy::asset::AssetServer>())
            }
            PyResourceType::Custom(type_ptr) => {
                // Check if the registry exists and contains this type
                if let Some(registry) = world.get_resource::<ResourceRegistry>()
                    && let Some(&component_id) = registry.registry.get(&type_ptr)
                {
                    // Check if the storage exists and contains this resource
                    if let Some(storage) = world.get_resource::<PyResourceStorage>() {
                        return Ok(storage.resources.contains_key(&component_id));
                    }
                }
                Ok(false)
            }
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

    pub fn entity(&self, entity: &PyEntity) -> PyResult<PyEntityCommands> {
        self.check_valid()?;
        let world = self.world_mut()?;
        // Verify entity exists
        world
            .get_entity(entity.0)
            .map_err(|_| PyRuntimeError::new_err("Entity does not exist"))?;
        Ok(PyEntityCommands::with_world(entity.0, self))
    }

    pub fn commands(pyself: Py<Self>, py: Python) -> PyResult<PyCommands> {
        {
            let borrowed = pyself.borrow(py);
            borrowed.check_valid()?;
        }

        let world_ptr = pyself.borrow(py).world_ptr();
        let validity = pyself.borrow(py).validity.clone().unwrap_or_default();

        let py_commands = unsafe { PyCommands::from_world(world_ptr, pyself, validity) };
        Ok(py_commands)
    }

    pub fn spawn_batch(&self, py: Python, batch: Bound<'_, PyAny>) -> PyResult<()> {
        self.check_valid()?;

        let world_ptr = self.world_ptr();
        let validity = self.validity.clone().unwrap_or_default();
        let temp_commands = unsafe { PyCommands::from_world_temporary(world_ptr, validity) };

        let iter = batch.call_method0("__iter__")?;
        loop {
            match iter.call_method0("__next__") {
                Ok(bundle) => {
                    let world = self.world_mut()?;
                    let entity_id = world.spawn_empty().id();

                    // Convert bundle to a tuple of components
                    let components = if bundle.is_instance_of::<pyo3::types::PyTuple>() {
                        bundle.cast::<pyo3::types::PyTuple>()?.clone()
                    } else {
                        pyo3::types::PyTuple::new(py, [&bundle])?
                    };

                    // Collect component types for lifecycle events
                    let mut component_types = Vec::new();
                    for component in components.iter() {
                        let component_type = component.get_type();
                        if let Ok(comp_type) = PyComponentType::try_from((&component_type, py)) {
                            component_types.push(comp_type);
                        }
                    }

                    crate::ecs::commands::insert_components_to_entity_helper(
                        &temp_commands,
                        py,
                        entity_id,
                        &components,
                    )?;

                    if !component_types.is_empty() {
                        Self::trigger_lifecycle_events_for_add(
                            world_ptr,
                            entity_id,
                            &component_types,
                        );
                    }
                }
                Err(e) => {
                    if e.is_instance_of::<pyo3::exceptions::PyStopIteration>(py) {
                        break;
                    }
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    pub fn register_component(&self, _py: Python, component: Bound<'_, PyAny>) -> PyResult<()> {
        self.check_valid()?;

        let type_obj: Bound<'_, PyType> = component.extract()?;
        let type_ptr = type_obj.as_type_ptr();
        let name = type_obj.name()?.to_string();

        let world = self.world_mut()?;
        register_custom_component(world, type_ptr, name);

        Ok(())
    }

    pub fn component_id(
        &self,
        py: Python,
        component: Bound<'_, PyAny>,
    ) -> PyResult<Option<PyComponentId>> {
        self.check_valid()?;

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
                    if let Some(component_id) = registry.get(type_ptr) {
                        return Ok(Some(PyComponentId(component_id)));
                    }
                }
                Ok(None)
            }
            PyComponentType::Dynamic(type_ptr) => {
                if let Some(bridge) =
                    pybevy_core::registry::global_registry::get_bridge_by_py_type(type_ptr)
                {
                    let world = unsafe { &mut *self.world_ptr() };
                    let component_id = bridge.register(world);
                    Ok(Some(PyComponentId(component_id)))
                } else {
                    Ok(None)
                }
            }
        }
    }

    pub fn trigger(&self, py: Python, event: Bound<'_, PyAny>) -> PyResult<()> {
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

        // Get the observer registry
        let world = self.world_mut()?;
        let registry = world.get_resource::<ObserverRegistry>();

        if let Some(registry) = registry {
            // Look up observers for this event type
            if let Some(observers) = registry.get_observers_for_event(py, &event)? {
                // Clone the observers list to avoid borrow conflicts
                let observers = observers.clone();

                // Get world reference for bundle filter checking
                let world_ref = unsafe { &*self.world_ptr() };

                // Execute each observer
                for observer_entry in observers {
                    // Check entity filter if present (per-entity observers)
                    if let Some(filter_entity) = observer_entry.entity_filter {
                        // This observer only triggers for a specific entity
                        if let Some(entity) = target_entity {
                            if entity != filter_entity {
                                // Event targets different entity, skip this observer
                                continue;
                            }
                        } else {
                            // Entity-specific observer on global event - skip
                            continue;
                        }
                    }

                    // Check bundle filter if present
                    if let Some(ref bundle_filter) = observer_entry.bundle_filter {
                        // Bundle filter requires an entity-targeted event
                        if let Some(entity) = target_entity {
                            // Create bundle filter and check if entity matches
                            let filter = BundleFilter {
                                components: bundle_filter.clone(),
                            };

                            if !filter.matches(world_ref, entity) {
                                // Entity doesn't have required components, skip this observer
                                continue;
                            }
                        } else {
                            // Bundle filter on global event - skip this observer
                            continue;
                        }
                    }

                    // Create the On parameter
                    let on_param = Py::new(
                        py,
                        PyOn {
                            event_data: event.clone().unbind(),
                            entity: target_entity,
                        },
                    )?;

                    // Execute the observer with full parameter injection
                    let world = self.world_mut()?;
                    execute_system_func(py, &observer_entry.system_func, world, on_param)
                        .inspect_err(|e| {
                            e.print(py);
                        })?;
                }
            }
        }

        Ok(())
    }

    pub fn add_observer(&self, py: Python, observer: Bound<'_, PyAny>) -> PyResult<Py<PyEntity>> {
        self.check_valid()?;
        let world = self.world_mut()?;

        let observer_entity = ObserverRegistry::register_observer(py, &observer, world)?;

        Py::new(py, PyEntity(observer_entity))
    }

    pub fn despawn_observer(&self, observer_entity: &PyEntity) -> PyResult<()> {
        self.check_valid()?;
        let world = self.world_mut()?;

        ObserverRegistry::despawn_observer(observer_entity.0, world)?;

        Ok(())
    }

    pub fn get(
        &self,
        py: Python,
        entity: &PyEntity,
        component_type: Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.check_valid()?;
        let world = self.world_mut()?;

        // Check if entity exists
        if !world.entities().contains(entity.0) {
            return Err(PyRuntimeError::new_err(format!(
                "Entity {:?} does not exist",
                entity.0
            )));
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

                let entity_ref = world.entity(entity.0);
                bridge.extract_from_entity_ref(&entity_ref, validity, py)
            }
            PyComponentType::Custom(type_ptr) => {
                self.extract_custom_component(py, entity.0, type_ptr, validity)
            }
        }
    }

    pub fn get_mut(
        &self,
        py: Python,
        entity: &PyEntity,
        component_type: Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.check_valid()?;
        let world = self.world_mut()?;

        if !world.entities().contains(entity.0) {
            return Err(PyRuntimeError::new_err(format!(
                "Entity {:?} does not exist",
                entity.0
            )));
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

                let mut entity_mut = world.entity_mut(entity.0);
                bridge.extract_from_entity_mut(&mut entity_mut, validity, py)
            }
            PyComponentType::Custom(type_ptr) => {
                self.extract_custom_component(py, entity.0, type_ptr, validity)
            }
        }
    }

    pub fn run_schedule(&self, py: Python, label: Bound<'_, PyAny>) -> PyResult<()> {
        self.check_valid()?;

        // Try PyStage first (SimTick, Update, etc.)
        if let Ok(stage) = label.extract::<PyStage>() {
            // Cast to usize to cross the GIL boundary (raw pointers aren't Ungil).
            // SAFETY: we have exclusive World access (SystemStateFlags::EXCLUSIVE)
            // and the pointer is valid for the system's lifetime (ValidityFlag).
            let world_addr = self.world_mut()? as *mut World as usize;

            // Release GIL before running the schedule to avoid deadlock:
            // this exclusive system holds the GIL, but inner Python systems
            // spawned by run_schedule() need to acquire it.
            py.detach(move || {
                let world = unsafe { &mut *(world_addr as *mut World) };
                stage.run_on_world(world);
                Ok::<(), PyErr>(())
            })?;

            return Ok(());
        }

        // State-based schedule labels (OnEnter, OnExit, OnTransition)
        let world = self.world_mut()?;

        if let Ok(on_enter) = label.cast::<PyOnEnterSchedule>() {
            let bevy_label = on_enter.borrow().to_bevy_label(py)?;
            world.run_schedule(bevy_label);
        } else if let Ok(on_exit) = label.cast::<PyOnExitSchedule>() {
            let bevy_label = on_exit.borrow().to_bevy_label(py)?;
            world.run_schedule(bevy_label);
        } else if let Ok(on_transition) = label.cast::<PyOnTransitionSchedule>() {
            let bevy_label = on_transition.borrow().to_bevy_label(py)?;
            world.run_schedule(bevy_label);
        } else {
            return Err(PyTypeError::new_err(
                "run_schedule() requires a Stage, OnEnter, OnExit, or OnTransition schedule label",
            ));
        }

        Ok(())
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

        // Create shared error state for the system
        let error_state: Arc<Mutex<Vec<PyErr>>> = Arc::new(Mutex::new(Vec::new()));

        // Get mutable access to the world
        let world = self.world_mut()?;

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
        let mut system = DynamicSystem::new(
            func.unbind(),
            generation,
            error_state.clone(),
            Arc::new(Mutex::new(None)),
            SystemStage::UpdateOrLast,
        )?;

        // Flush any deferred commands from prior operations (e.g., entities
        // spawned via MCP mutations) so queries inside the system see them.
        world.flush();

        // Initialize the system (registers components, etc.)
        let _ = system.initialize(world);

        // Create an UnsafeWorldCell for run_unsafe
        // SAFETY: We have exclusive access to the world through world_mut()
        let world_cell = world.as_unsafe_world_cell();

        // Run the system
        // SAFETY: We have exclusive world access and the system was just initialized
        let result = unsafe { system.run_unsafe((), world_cell) };

        // Apply any deferred commands from the system
        system.apply_deferred(world);

        // Flush any commands that were queued through Commands parameter
        world.flush();

        // Check for any errors that occurred during system execution
        let mut errors = error_state.lock().unwrap();
        if let Some(err) = errors.pop() {
            return Err(err);
        }

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
        let mut component_types = Vec::new();

        // Get entity reference
        let Ok(entity_ref) = world.get_entity(entity) else {
            return component_types; // Entity doesn't exist
        };

        // Check all registered component bridges
        for bridge in pybevy_core::registry::global_registry::all_component_bridges() {
            if bridge.entity_contains(&entity_ref) {
                component_types.push(PyComponentType::Dynamic(bridge.py_type_ptr()));
            }
        }

        // Check all registered custom Python components.
        // Without this, `On[Despawn, MyCustomComponent]` observers would never
        // fire because the dispatcher iterates this list to build event keys.
        if let Some(custom_info) = world.get_resource::<pybevy_core::CustomComponentInfo>() {
            for (component_id, entry) in custom_info.iter() {
                if entity_ref.contains_id(component_id) {
                    component_types.push(PyComponentType::Custom(entry.type_ptr));
                }
            }
        }

        component_types
    }

    /// Generic helper to trigger lifecycle events for a list of components
    fn trigger_lifecycle_events(
        world_ptr: *mut World,
        entity: bevy::ecs::entity::Entity,
        component_types: &[PyComponentType],
        event_type_fn: fn(PyComponentType) -> EventType,
    ) {
        Python::attach(|py| {
            let world_ref = unsafe { &*world_ptr };

            // Get observer registry
            let registry = match world_ref.get_resource::<ObserverRegistry>() {
                Some(r) => r,
                None => return, // No observers registered
            };

            // For each component, trigger the lifecycle event
            for comp_type in component_types {
                let event_type = event_type_fn(comp_type.clone());

                // Get observers for this event type
                if let Some(observers) = registry.get_observers(&event_type) {
                    let observers = observers.clone();

                    for observer_entry in observers {
                        // Check entity filter if present (per-entity observers)
                        if let Some(filter_entity) = observer_entry.entity_filter
                            && entity != filter_entity
                        {
                            continue; // Event targets different entity
                        }

                        // Check bundle filter if present
                        if let Some(ref bundle_filter) = observer_entry.bundle_filter {
                            let filter = BundleFilter {
                                components: bundle_filter.clone(),
                            };
                            if !filter.matches(world_ref, entity) {
                                continue; // Entity doesn't have required components
                            }
                        }

                        // Create the On parameter
                        // For lifecycle events, there's no event data - just the entity
                        if let Ok(on_param) = Py::new(
                            py,
                            PyOn {
                                event_data: py.None(),
                                entity: Some(entity),
                            },
                        ) {
                            let world = unsafe { &mut *world_ptr };
                            if let Err(e) = execute_system_func(
                                py,
                                &observer_entry.system_func,
                                world,
                                on_param,
                            ) {
                                e.print(py);
                            }
                        }
                    }
                }
            }
        });
    }

    /// Trigger lifecycle events for components added to an entity.
    /// This is called internally after components are added.
    pub(crate) fn trigger_lifecycle_events_for_add(
        world_ptr: *mut World,
        entity: bevy::ecs::entity::Entity,
        component_types: &[PyComponentType],
    ) {
        Self::trigger_lifecycle_events(world_ptr, entity, component_types, EventType::Add);
    }

    /// Trigger lifecycle events for components inserted to an entity.
    /// Insert triggers on both initial add and replacement.
    pub(crate) fn trigger_lifecycle_events_for_insert(
        world_ptr: *mut World,
        entity: bevy::ecs::entity::Entity,
        component_types: &[PyComponentType],
    ) {
        Self::trigger_lifecycle_events(world_ptr, entity, component_types, EventType::Insert);
    }

    /// Trigger lifecycle events for components removed from an entity.
    pub(crate) fn trigger_lifecycle_events_for_remove(
        world_ptr: *mut World,
        entity: bevy::ecs::entity::Entity,
        component_types: &[PyComponentType],
    ) {
        Self::trigger_lifecycle_events(world_ptr, entity, component_types, EventType::Remove);
    }

    /// Trigger lifecycle events for entity despawn.
    /// This triggers Despawn for each component type on the entity.
    pub(crate) fn trigger_lifecycle_events_for_despawn(
        world_ptr: *mut World,
        entity: bevy::ecs::entity::Entity,
        component_types: &[PyComponentType],
    ) {
        Self::trigger_lifecycle_events(world_ptr, entity, component_types, EventType::Despawn);
    }

    /// Trigger lifecycle events for component values discarded on an entity.
    /// This triggers when a component is inserted onto an entity that already has it,
    /// before the old value is replaced.
    pub(crate) fn trigger_lifecycle_events_for_discard(
        world_ptr: *mut World,
        entity: bevy::ecs::entity::Entity,
        component_types: &[PyComponentType],
    ) {
        Self::trigger_lifecycle_events(world_ptr, entity, component_types, EventType::Discard);
    }
}
