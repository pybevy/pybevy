use bevy::ecs::{
    entity::Entity, hierarchy::ChildOf, ptr::OwningPtr, system::Commands, world::World,
};
use pybevy_core::registry::global_registry;
use pyo3::{
    exceptions::{PyRuntimeError, PyStopIteration, PyTypeError, PyValueError},
    ffi::PyTypeObject,
    prelude::*,
    types::{PyTuple, PyType},
};

use super::{
    PyEntity,
    component_type::{PyComponentType, register_custom_component},
    entity_commands::PyEntityCommands,
    helpers::{type_utils::get_python_type_name, validity_guard::ValidityFlag},
    resource_type::PyResourceType,
    world::PyWorld,
};
use crate::ecs::{
    batch_spawn::SpawnBatchCommand,
    component_layout::{
        ComponentLayout, ComponentLayoutExt, ComponentStorageType, ComponentStorageTypeExt,
        serialize_to_wrapper,
    },
    component_type::ComponentRegistry,
    component_wrapper::*,
    dynamic_system::execute_system_func,
    observer::{BundleFilter, PyEvent, PyOn},
    observer_registry::ObserverRegistry,
};

/// Wrapper to make PyTypeObject pointer Send-safe by storing it as usize
/// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
#[derive(Copy, Clone)]
struct SendTypePtr(usize);

impl SendTypePtr {
    fn new(ptr: *const PyTypeObject) -> Self {
        SendTypePtr(ptr as usize)
    }

    fn as_ptr(&self) -> *const PyTypeObject {
        self.0 as *const PyTypeObject
    }
}

unsafe impl Send for SendTypePtr {}
unsafe impl Sync for SendTypePtr {}

/// Wrapper around Bevy's Commands system parameter.
/// Commands queue operations to be applied to the World after the system completes.
///
/// Safety: This type is Send because:
/// 1. Raw pointers are just addresses (Send if we guarantee validity)
/// 2. ValidityFlag is Arc<AtomicBool> which is Send + Sync
/// 3. Runtime validity checking prevents use after the system completes
/// 4. The optional PyWorld reference is also Send (Py<T> is Send if T is Send)
#[pyclass(name = "Commands")]
pub struct PyCommands {
    commands_ptr: *mut (),
    is_world: bool, // Flag to indicate if this wraps a World instead of Commands
    // Keep the PyWorld alive if we're wrapping a World
    _world_ref: Option<Py<PyWorld>>,
    // Runtime validity check - prevents use after system execution
    validity: ValidityFlag,
}

// SAFETY: PyCommands is Send because:
// - The raw pointer is protected by the ValidityFlag (Arc<AtomicBool>)
// - ValidityFlag::check() ensures the pointer is only dereferenced when valid
// - The validity flag is set to false when the system execution completes
// - Py<PyWorld> is Send when PyWorld is Send
unsafe impl Send for PyCommands {}

// SAFETY: PyCommands is Sync because:
// - Access to the underlying Commands/World is controlled by validity checking
// - The ValidityFlag uses atomic operations for thread-safe access
// - We only allow access when the validity flag is true (during system execution)
unsafe impl Sync for PyCommands {}

impl PyCommands {
    /// Create a new PyCommands wrapper around a mutable Commands reference.
    ///
    /// # Safety
    /// The commands pointer must be valid for the lifetime of this PyCommands instance.
    /// This should only be created within the system's run_unsafe and dropped before returning.
    pub(crate) unsafe fn new(commands: &mut Commands, validity: ValidityFlag) -> Self {
        Self {
            commands_ptr: commands as *mut Commands as *mut (),
            is_world: false,
            _world_ref: None,
            validity,
        }
    }

    /// Create a PyCommands that wraps a World pointer
    ///
    /// # Safety
    /// The world pointer must be valid for the lifetime of this PyCommands instance.
    pub(crate) unsafe fn from_world(
        world_ptr: *mut World,
        world_ref: Py<PyWorld>,
        validity: ValidityFlag,
    ) -> Self {
        Self {
            commands_ptr: world_ptr as *mut (),
            is_world: true,
            _world_ref: Some(world_ref),
            validity,
        }
    }

    /// Create a temporary PyCommands that wraps a World pointer without owning a PyWorld reference
    /// Used internally when we're already within a PyWorld method
    ///
    /// # Safety
    /// The world pointer must be valid for the lifetime of this PyCommands instance.
    pub(crate) unsafe fn from_world_temporary(
        world_ptr: *mut World,
        validity: ValidityFlag,
    ) -> Self {
        Self {
            commands_ptr: world_ptr as *mut (),
            is_world: true,
            _world_ref: None,
            validity,
        }
    }

    /// Check if this Commands instance is still valid for use
    fn check_valid(&self) -> PyResult<()> {
        Ok(self.validity.check()?)
    }

    /// Get a clone of the validity flag for sharing with child structures
    pub(crate) fn validity(&self) -> ValidityFlag {
        self.validity.clone()
    }

    // validity-checked raw pointer access, see docs/safety.md
    #[allow(clippy::mut_from_ref)]
    fn commands_mut(&self) -> PyResult<&mut Commands<'_, '_>> {
        self.validity.check()?;
        if self.is_world {
            return Err(PyRuntimeError::new_err(
                "Cannot get Commands from World-backed PyCommands",
            ));
        }
        Ok(unsafe { &mut *(self.commands_ptr as *mut Commands) })
    }

    // validity-checked raw pointer access, see docs/safety.md
    #[allow(clippy::mut_from_ref)]
    fn world_mut(&self) -> PyResult<&mut World> {
        self.validity.check()?;
        if !self.is_world {
            return Err(PyRuntimeError::new_err(
                "Cannot get World from Commands-backed PyCommands",
            ));
        }
        Ok(unsafe { &mut *(self.commands_ptr as *mut World) })
    }

    /// Get world access if this is world-backed, otherwise return None
    // validity-checked raw pointer access, see docs/safety.md
    #[allow(clippy::mut_from_ref)]
    pub(crate) fn try_world_mut(&self) -> PyResult<Option<&mut World>> {
        self.validity.check()?;
        if self.is_world {
            Ok(Some(unsafe { &mut *(self.commands_ptr as *mut World) }))
        } else {
            Ok(None)
        }
    }

    /// Execute an operation immediately on World or queue it as a Command.
    /// For operations that don't need a return value.
    pub(crate) fn execute_or_queue<F>(&self, operation: F) -> PyResult<()>
    where
        F: FnOnce(&mut World) + Send + 'static,
    {
        if self.is_world {
            let world = self.world_mut()?;
            operation(world);
        } else {
            let commands = self.commands_mut()?;
            commands.queue(operation);
        }
        Ok(())
    }

    /// Execute an operation that returns a value.
    /// Requires separate closures for world and commands cases since they may have different logic.
    fn execute_returning<T, FW, FC>(&self, world_op: FW, commands_op: FC) -> PyResult<T>
    where
        FW: FnOnce(&mut World) -> T,
        FC: FnOnce(&mut Commands) -> T,
    {
        if self.is_world {
            Ok(world_op(self.world_mut()?))
        } else {
            Ok(commands_op(self.commands_mut()?))
        }
    }
}

/// Helper function to insert components to an entity
pub(crate) fn insert_components_to_entity_helper(
    commands: &PyCommands,
    py: Python,
    entity_id: Entity,
    components: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    // Collect component types for lifecycle events
    let mut component_types = Vec::new();
    for component in components.iter() {
        let component_type = component.get_type();
        if let Ok(comp_type) = PyComponentType::try_from((&component_type, py)) {
            component_types.push(comp_type);
        }
    }

    if !component_types.is_empty() {
        if commands.is_world {
            // Immediate execution path
            let world_ptr = commands.commands_ptr as *mut World;

            // Check which components already exist (for Discard)
            let existing_components = {
                let world = unsafe { &*(commands.commands_ptr as *const World) };
                component_types
                    .iter()
                    .filter(|comp_type| {
                        crate::ecs::observer::entity_has_component_type(world, entity_id, comp_type)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            };

            // Fire Discard BEFORE the insert (so observers can read old value)
            if !existing_components.is_empty() {
                PyWorld::trigger_lifecycle_events_for_discard(
                    world_ptr,
                    entity_id,
                    &existing_components,
                );
            }

            // Do the insert
            insert_components_to_entity(commands, py, entity_id, components)?;

            // Fire Insert AFTER the insert
            PyWorld::trigger_lifecycle_events_for_insert(world_ptr, entity_id, &component_types);
        } else {
            // Deferred execution path
            // Queue Discard check+trigger BEFORE the inserts (at apply time)
            let component_types_for_discard = component_types.clone();
            commands.execute_or_queue(move |world| {
                let existing: Vec<_> = component_types_for_discard
                    .iter()
                    .filter(|comp_type| {
                        crate::ecs::observer::entity_has_component_type(world, entity_id, comp_type)
                    })
                    .cloned()
                    .collect();

                if !existing.is_empty() {
                    PyWorld::trigger_lifecycle_events_for_discard(
                        world as *mut World,
                        entity_id,
                        &existing,
                    );
                }
            })?;

            // Queue the actual inserts
            insert_components_to_entity(commands, py, entity_id, components)?;

            // Queue Insert trigger AFTER inserts
            let component_types_for_insert = component_types.clone();
            commands.execute_or_queue(move |world| {
                PyWorld::trigger_lifecycle_events_for_insert(
                    world as *mut World,
                    entity_id,
                    &component_types_for_insert,
                );
            })?;
        }
    } else {
        // No component types to track - just insert
        insert_components_to_entity(commands, py, entity_id, components)?;
    }

    Ok(())
}

/// Internal helper function to insert components to an entity
fn insert_components_to_entity(
    commands: &PyCommands,
    py: Python,
    entity_id: Entity,
    components: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    for component in components.iter() {
        // Determine component type
        let component_type = PyComponentType::try_from((&component.get_type(), py))?;

        // Insert the component based on its type
        match component_type {
            // Children, GlobalTransform use dynamic dispatch from pybevy_core - bridges return appropriate errors
            // Gamepad, AudioSink, SpatialAudioSink now handled via bridge (no_insert returns error)
            PyComponentType::Dynamic(type_ptr) => {
                // Dynamic component - use bridge for insertion
                // Get the bridge for this type
                let bridge = global_registry::get_bridge_by_py_type(type_ptr).ok_or_else(|| {
                    PyRuntimeError::new_err("Dynamic component type not registered")
                })?;

                if commands.is_world {
                    // Direct world access - insert immediately via bridge
                    let world = commands.world_mut()?;
                    bridge.insert(world, entity_id, &component)?;
                } else {
                    // Commands - need to queue the operation
                    // Clone data needed for the deferred operation
                    let py_obj = component.clone().unbind();
                    let bridge_name = bridge.name();

                    commands.execute_or_queue(move |world: &mut World| {
                        // Re-acquire GIL and re-bind the component
                        Python::attach(|py| {
                            let component_bound = py_obj.bind(py);
                            let type_obj = component_bound.get_type();
                            let type_ptr = type_obj.as_type_ptr();

                            // Get the bridge again (it's registered globally)
                            if let Some(bridge) = global_registry::get_bridge_by_py_type(type_ptr)
                                && let Err(e) = bridge.insert(world, entity_id, component_bound)
                            {
                                eprintln!(
                                    "Failed to insert dynamic component '{}': {}",
                                    bridge_name, e
                                );
                            }
                        });
                    })?;
                }
            }
            PyComponentType::Custom(raw_type_ptr) => {
                // Custom component insertion - needs different handling for Commands vs World
                let py_obj = component.clone().unbind();

                // Wrap the type pointer to make it Send-safe
                let type_ptr = SendTypePtr::new(raw_type_ptr);

                // Get the component name from the Python type for the descriptor
                // PERF: do this in the PyComponentType::Custom creation to avoid doing it repeatedly
                let name = Python::attach(|py| get_python_type_name(py, type_ptr.as_ptr()));

                if commands.is_world {
                    // Direct world access - get world ref once and use throughout
                    let world = commands.world_mut()?;

                    // Determine storage type and potentially serialize to wrapper
                    let (component_id, wrapper_data) = Python::attach(|py| {
                        // Register the component
                        let component_id =
                            register_custom_component(world, type_ptr.as_ptr(), name.clone());

                        // Determine storage type
                        // SAFETY: registered type pointers live for the interpreter lifetime
                        let py_type = unsafe {
                            pyo3::Bound::from_borrowed_ptr(
                                py,
                                type_ptr.as_ptr() as *mut pyo3::ffi::PyObject,
                            )
                        };

                        let storage_type = if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                            ComponentStorageType::from_python_class(cls)
                                .unwrap_or(ComponentStorageType::PyObject)
                        } else {
                            ComponentStorageType::PyObject
                        };

                        // Serialize if needed
                        let wrapper_data = match storage_type {
                            ComponentStorageType::Wrapper(_) => {
                                // Serialize to bytes
                                if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                                    if let Ok(layout) = ComponentLayout::from_annotations(cls) {
                                        serialize_to_wrapper(&component.clone(), &layout).ok()
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            }
                            ComponentStorageType::PyObject => None,
                        };

                        (component_id, wrapper_data)
                    });

                    // Insert immediately
                    if let Some(bytes) = wrapper_data {
                        // Wrapper storage - insert the appropriate wrapper
                        let wrapper_size = WrapperSize::for_size(bytes.len())
                            .expect("Wrapper size should be valid");

                        macro_rules! insert_wrapper {
                            ($size:expr, $wrapper_type:ty) => {
                                if wrapper_size == $size {
                                    let mut wrapper = <$wrapper_type>::default();
                                    wrapper.data[..bytes.len()].copy_from_slice(&bytes);

                                    OwningPtr::make(wrapper, |ptr| {
                                        let mut entity = world.entity_mut(entity_id);
                                        unsafe {
                                            entity.insert_by_id(component_id, ptr);
                                        }
                                    });
                                }
                            };
                        }

                        insert_wrapper!(WrapperSize::W8, ComponentWrapper8);
                        insert_wrapper!(WrapperSize::W16, ComponentWrapper16);
                        insert_wrapper!(WrapperSize::W32, ComponentWrapper32);
                        insert_wrapper!(WrapperSize::W64, ComponentWrapper64);
                        insert_wrapper!(WrapperSize::W128, ComponentWrapper128);
                        insert_wrapper!(WrapperSize::W256, ComponentWrapper256);
                        insert_wrapper!(WrapperSize::W512, ComponentWrapper512);
                        insert_wrapper!(WrapperSize::W1024, ComponentWrapper1024);
                    } else {
                        // PyObject storage - use existing path
                        OwningPtr::make(py_obj, |ptr| {
                            let mut entity = world.entity_mut(entity_id);
                            unsafe {
                                entity.insert_by_id(component_id, ptr);
                            }
                        });
                    }
                } else {
                    // Determine storage type and serialize for deferred command
                    let wrapper_data = Python::attach(|py| {
                        // SAFETY: registered type pointers live for the interpreter lifetime
                        let py_type = unsafe {
                            pyo3::Bound::from_borrowed_ptr(
                                py,
                                type_ptr.as_ptr() as *mut pyo3::ffi::PyObject,
                            )
                        };

                        if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                            // Check storage type to determine if we need to serialize
                            let _ = ComponentStorageType::from_python_class(cls);
                        }

                        let storage_type = if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                            ComponentStorageType::from_python_class(cls)
                                .unwrap_or(ComponentStorageType::PyObject)
                        } else {
                            ComponentStorageType::PyObject
                        };

                        // Serialize if needed
                        match storage_type {
                            ComponentStorageType::Wrapper(_) => {
                                if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                                    if let Ok(layout) = ComponentLayout::from_annotations(cls) {
                                        serialize_to_wrapper(&component.clone(), &layout).ok()
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            }
                            ComponentStorageType::PyObject => None,
                        }
                    });

                    // Commands - queue the operation for later
                    commands.execute_or_queue(move |world: &mut World| {
                        // Register the component when the command is applied
                        let component_id =
                            register_custom_component(world, type_ptr.as_ptr(), name);

                        if let Some(bytes) = wrapper_data {
                            // Wrapper storage
                            let wrapper_size = WrapperSize::for_size(bytes.len())
                                .expect("Wrapper size should be valid");

                            macro_rules! insert_wrapper {
                                ($size:expr, $wrapper_type:ty) => {
                                    if wrapper_size == $size {
                                        let mut wrapper = <$wrapper_type>::default();
                                        wrapper.data[..bytes.len()].copy_from_slice(&bytes);

                                        OwningPtr::make(wrapper, |ptr| {
                                            let mut entity = world.entity_mut(entity_id);
                                            unsafe {
                                                entity.insert_by_id(component_id, ptr);
                                            }
                                        });
                                    }
                                };
                            }

                            insert_wrapper!(WrapperSize::W8, ComponentWrapper8);
                            insert_wrapper!(WrapperSize::W16, ComponentWrapper16);
                            insert_wrapper!(WrapperSize::W32, ComponentWrapper32);
                            insert_wrapper!(WrapperSize::W64, ComponentWrapper64);
                            insert_wrapper!(WrapperSize::W128, ComponentWrapper128);
                            insert_wrapper!(WrapperSize::W256, ComponentWrapper256);
                            insert_wrapper!(WrapperSize::W512, ComponentWrapper512);
                            insert_wrapper!(WrapperSize::W1024, ComponentWrapper1024);
                        } else {
                            // PyObject storage
                            OwningPtr::make(py_obj, |ptr| {
                                let mut entity = world.entity_mut(entity_id);
                                unsafe {
                                    entity.insert_by_id(component_id, ptr);
                                }
                            });
                        }
                    })?;
                }
            }
        }
    }

    Ok(())
}

/// Helper function to add a child to an entity
pub(crate) fn add_child_helper(
    commands: &PyCommands,
    parent_id: Entity,
    child_id: Entity,
) -> PyResult<()> {
    if commands.is_world {
        commands
            .world_mut()?
            .entity_mut(parent_id)
            .add_child(child_id);
    } else {
        commands
            .commands_mut()?
            .entity(parent_id)
            .add_child(child_id);
    }
    Ok(())
}

/// Helper function to remove children from an entity
pub(crate) fn remove_children_helper(
    commands: &PyCommands,
    parent_id: Entity,
    child_ids: &[Entity],
) -> PyResult<()> {
    if commands.is_world {
        commands
            .world_mut()?
            .entity_mut(parent_id)
            .detach_children(child_ids);
    } else {
        commands
            .commands_mut()?
            .entity(parent_id)
            .detach_children(child_ids);
    }
    Ok(())
}

/// Helper function to clear all children from an entity
pub(crate) fn clear_children_helper(commands: &PyCommands, parent_id: Entity) -> PyResult<()> {
    if commands.is_world {
        commands
            .world_mut()?
            .entity_mut(parent_id)
            .detach_all_children();
    } else {
        commands
            .commands_mut()?
            .entity(parent_id)
            .detach_all_children();
    }
    Ok(())
}

/// Helper function to set the parent of an entity
pub(crate) fn set_parent_helper(
    commands: &PyCommands,
    child_id: Entity,
    parent_id: Entity,
) -> PyResult<()> {
    if commands.is_world {
        commands
            .world_mut()?
            .entity_mut(child_id)
            .insert(ChildOf(parent_id));
    } else {
        commands
            .commands_mut()?
            .entity(child_id)
            .insert(ChildOf(parent_id));
    }
    Ok(())
}

/// Helper function to remove parent relationship from an entity
pub(crate) fn remove_parent_helper(commands: &PyCommands, child_id: Entity) -> PyResult<()> {
    if commands.is_world {
        commands
            .world_mut()?
            .entity_mut(child_id)
            .remove::<ChildOf>();
    } else {
        commands
            .commands_mut()?
            .entity(child_id)
            .remove::<ChildOf>();
    }
    Ok(())
}

/// Helper function to remove components from an entity
pub(crate) fn remove_components_from_entity_helper(
    commands: &PyCommands,
    py: Python,
    entity_id: Entity,
    components: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    // Collect component types for lifecycle events
    let mut component_types = Vec::new();
    for component in components.iter() {
        if let Ok(component_type_obj) = component.cast::<PyType>()
            && let Ok(comp_type) = PyComponentType::try_from((component_type_obj, py))
        {
            component_types.push(comp_type);
        }
    }

    // Remove the components
    remove_components_from_entity(commands, py, entity_id, components)?;

    // Trigger Remove lifecycle events (deferred if using Commands)
    if !component_types.is_empty() {
        if commands.is_world {
            // Immediate execution - trigger now
            let world_ptr = commands.commands_ptr as *mut World;
            PyWorld::trigger_lifecycle_events_for_remove(world_ptr, entity_id, &component_types);
        } else {
            // Deferred execution - queue the trigger
            commands.execute_or_queue(move |world| {
                PyWorld::trigger_lifecycle_events_for_remove(
                    world as *mut World,
                    entity_id,
                    &component_types,
                );
            })?;
        }
    }

    Ok(())
}

/// Internal helper function to remove components from an entity
fn remove_components_from_entity(
    commands: &PyCommands,
    py: Python,
    entity_id: Entity,
    components: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    for component in components.iter() {
        // Component should be a type (class), not an instance
        let component_type_obj = component.cast::<PyType>().map_err(|_| {
            PyTypeError::new_err(
                "remove() expects component types (classes), not instances. Use Foo instead of Foo()",
            )
        })?;

        // Determine component type
        let component_type = PyComponentType::try_from((component_type_obj, py))?;

        // Remove the component based on its type
        match component_type {
            // Children uses dynamic dispatch from pybevy_core
            PyComponentType::Dynamic(type_ptr) => {
                // Dynamic component removal - use bridge registry to get ComponentId
                // Check if this is an auto-managed component that can't be removed
                if let Some(bridge) = global_registry::get_bridge_by_py_type(type_ptr)
                    && bridge.name() == "Children"
                {
                    return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                        "Cannot remove Children component - it is auto-managed by Bevy. Remove ChildOf components instead.",
                    ));
                }

                let type_ptr_copy = SendTypePtr::new(type_ptr);

                if commands.is_world {
                    // Direct world access - get bridge and remove
                    let world = commands.world_mut()?;
                    if let Some(bridge) = global_registry::get_bridge_by_py_type(type_ptr) {
                        let component_id = bridge.register(world);
                        world.entity_mut(entity_id).remove_by_id(component_id);
                    }
                } else {
                    // Commands - queue the operation for later
                    commands.execute_or_queue(move |world: &mut World| {
                        if let Some(bridge) =
                            global_registry::get_bridge_by_py_type(type_ptr_copy.as_ptr())
                        {
                            let component_id = bridge.register(world);
                            world.entity_mut(entity_id).remove_by_id(component_id);
                        }
                    })?;
                }
            }
            PyComponentType::Custom(raw_type_ptr) => {
                // Custom component removal - needs to use remove_by_id
                let type_ptr = SendTypePtr::new(raw_type_ptr);

                if commands.is_world {
                    // Direct world access - look up ComponentId and remove
                    let world = commands.world_mut()?;
                    if let Some(registry) = world.get_resource::<ComponentRegistry>()
                        && let Some(component_id) = registry.get(type_ptr.as_ptr())
                    {
                        world.entity_mut(entity_id).remove_by_id(component_id);
                    }
                    // Silently ignore if component not registered or not present
                } else {
                    // Commands - queue the operation for later
                    commands.execute_or_queue(move |world: &mut World| {
                        if let Some(registry) = world.get_resource::<ComponentRegistry>()
                            && let Some(component_id) = registry.get(type_ptr.as_ptr())
                        {
                            world.entity_mut(entity_id).remove_by_id(component_id);
                        }
                    })?;
                }
            }
        }
    }

    Ok(())
}

#[pymethods]
impl PyCommands {
    pub fn spawn_empty(&self, _py: Python<'_>) -> PyResult<PyEntityCommands> {
        self.check_valid()?;

        let entity = self.execute_returning(
            |world| world.spawn_empty().id(),
            |commands| commands.spawn_empty().id(),
        )?;

        Ok(PyEntityCommands::with_commands(entity, self))
    }

    #[pyo3(signature = (*components))]
    pub fn spawn(&self, py: Python, components: &Bound<'_, PyTuple>) -> PyResult<PyEntityCommands> {
        self.check_valid()?;

        let entity_id = self.execute_returning(
            |world| world.spawn_empty().id(),
            |commands| commands.spawn_empty().id(),
        )?;

        // Handle two cases:
        // 1. spawn(CompA(), CompB()) - multiple args passed directly
        // 2. spawn((CompA(), CompB())) - single tuple arg
        let components_to_insert = if components.len() == 1 {
            // Check if the single argument is itself a tuple
            let first_item = components.get_item(0)?;
            if first_item.is_instance_of::<PyTuple>() {
                // Case 2: User passed a tuple, extract it
                first_item.extract::<Bound<'_, PyTuple>>()?
            } else {
                // Case 1: Single component
                components.clone()
            }
        } else {
            // Case 1: Multiple components passed as separate args
            components.clone()
        };

        // Collect component types for lifecycle events
        let mut component_types = Vec::new();
        for component in components_to_insert.iter() {
            let component_type = component.get_type();
            if let Ok(comp_type) = PyComponentType::try_from((&component_type, py)) {
                component_types.push(comp_type);
            }
        }

        // Then insert each component
        insert_components_to_entity(self, py, entity_id, &components_to_insert)?;

        // Queue lifecycle event triggering (deferred execution)
        if !component_types.is_empty() {
            self.execute_or_queue(move |world| {
                PyWorld::trigger_lifecycle_events_for_add(
                    world as *mut World,
                    entity_id,
                    &component_types,
                );
            })?;
        }

        Ok(PyEntityCommands::with_commands(entity_id, self))
    }

    #[pyo3(signature = (*args, count=None))]
    pub fn spawn_batch(
        &self,
        py: Python,
        args: &Bound<'_, PyTuple>,
        count: Option<usize>,
    ) -> PyResult<Py<PyAny>> {
        self.check_valid()?;

        // Detect legacy iterable path: single arg that is a list or has __iter__ but is not a Component
        if args.len() == 1 && count.is_none() {
            let first = args.get_item(0)?;
            if first.is_instance_of::<pyo3::types::PyList>() || first.hasattr("__next__")? {
                self.spawn_batch_iter(py, first)?;
                return Ok(py.None());
            }
        }

        // Batch/uniform path
        let command = SpawnBatchCommand::new(py, args, count)?;

        if self.is_world {
            let entities = command.apply(self.world_mut()?)?;
            let entity_list: Vec<PyEntity> = entities.into_iter().map(PyEntity).collect();
            Ok(entity_list.into_pyobject(py)?.into())
        } else {
            // Deferred path: queue the batch spawn as a command
            // Entity IDs are not available until flush, so return None
            self.commands_mut()?.queue(move |world: &mut World| {
                if let Err(e) = command.apply(world) {
                    eprintln!("spawn_batch error during command flush: {e}");
                }
            });
            Ok(py.None())
        }
    }

    // FIXME: is this needed anymore?
    fn spawn_batch_iter(&self, py: Python, batch: Bound<'_, PyAny>) -> PyResult<()> {
        let iter = batch.call_method0("__iter__")?;
        loop {
            match iter.call_method0("__next__") {
                Ok(bundle) => {
                    let entity_id = self.execute_returning(
                        |world| world.spawn_empty().id(),
                        |commands| commands.spawn_empty().id(),
                    )?;

                    // Extract components from the bundle tuple
                    let components_tuple = bundle.extract::<Bound<'_, PyTuple>>()?;

                    // Insert each component
                    insert_components_to_entity(self, py, entity_id, &components_tuple)?;
                }
                Err(e) => {
                    if e.is_instance_of::<PyStopIteration>(py) {
                        break;
                    }
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    pub fn entity(&self, entity: &PyEntity) -> PyResult<PyEntityCommands> {
        self.check_valid()?;
        if self.is_world {
            let world = self.world_mut()?;
            if world.get_entity(entity.0).is_err() {
                return Err(PyValueError::new_err("Entity does not exist in the world"));
            }
        }
        // Note: For Commands backend, we can't check existence (deferred operations)
        Ok(PyEntityCommands::with_commands(entity.0, self))
    }

    pub fn get_entity(&self, entity: &PyEntity) -> PyResult<Option<PyEntityCommands>> {
        self.check_valid()?;
        if self.is_world {
            let world = self.world_mut()?;
            if world.get_entity(entity.0).is_err() {
                return Ok(None);
            }
        }
        Ok(Some(PyEntityCommands::with_commands(entity.0, self)))
    }

    pub fn despawn(&self, entity: &PyEntity) -> PyResult<()> {
        self.check_valid()?;
        let entity_id = entity.0;

        if self.is_world {
            // Direct world access
            let world_ptr = self.commands_ptr as *mut World;
            let world = self.world_mut()?;

            // Collect component types before despawning
            let component_types =
                crate::ecs::world::PyWorld::get_entity_data_names(world, entity_id);

            // Clean up any per-entity observers watching this entity
            ObserverRegistry::cleanup_on_entity_despawn(entity_id, world);

            // Despawn the entity
            world.despawn(entity_id);

            // Trigger Despawn lifecycle events
            if !component_types.is_empty() {
                PyWorld::trigger_lifecycle_events_for_despawn(
                    world_ptr,
                    entity_id,
                    &component_types,
                );
            }
        } else {
            // Deferred commands
            // We need to collect component types before queuing the despawn
            // This is tricky because we can't access the world yet
            // For now, we'll collect component types in the deferred command
            self.execute_or_queue(move |world| {
                // Collect component types before despawning
                let component_types = PyWorld::get_entity_data_names(world, entity_id);

                // Clean up any per-entity observers watching this entity
                ObserverRegistry::cleanup_on_entity_despawn(entity_id, world);

                // Despawn the entity
                world.despawn(entity_id);

                // Trigger Despawn lifecycle events
                if !component_types.is_empty() {
                    PyWorld::trigger_lifecycle_events_for_despawn(
                        world as *mut World,
                        entity_id,
                        &component_types,
                    );
                }
            })?;
        }

        Ok(())
    }

    pub fn insert_resource(&self, py: Python, resource: Bound<'_, PyAny>) -> PyResult<()> {
        self.check_valid()?;

        // Get the resource type from the instance
        let resource_type = resource.get_type();
        let py_resource_type = PyResourceType::try_from((&resource_type, py))?;

        // Convert the bound resource to a Py<PyAny>
        let resource_instance: Py<PyAny> = resource.unbind();

        if self.is_world {
            // Direct insertion into world
            py_resource_type.insert_into_world(self.world_mut()?, py, resource_instance)?;
        } else {
            // Queue a command to insert the resource later
            // Clone resource_instance for the command closure
            let resource_clone = resource_instance.clone_ref(py);

            self.execute_or_queue(move |world: &mut World| {
                Python::attach(|py| {
                    if let Err(e) = py_resource_type.insert_into_world(world, py, resource_clone) {
                        eprintln!("Error: Failed to insert resource via Commands: {:?}", e);
                    }
                });
            })?;
        }

        Ok(())
    }

    pub fn remove_resource(&self, py: Python, resource_type: Bound<'_, PyAny>) -> PyResult<()> {
        self.check_valid()?;

        // Get the resource type - it should be a type object
        let type_obj = resource_type.cast::<PyType>().map_err(|_| {
            PyTypeError::new_err("remove_resource expects a resource type (class), not an instance")
        })?;

        let py_resource_type = PyResourceType::try_from((type_obj, py))?;

        if self.is_world {
            // Direct removal from world
            py_resource_type.remove_from_world(self.world_mut()?, py)?;
        } else {
            // Queue a command to remove the resource later
            self.execute_or_queue(move |world: &mut World| {
                Python::attach(|py| {
                    if let Err(e) = py_resource_type.remove_from_world(world, py) {
                        eprintln!("Error: Failed to remove resource via Commands: {:?}", e);
                    }
                });
            })?;
        }

        Ok(())
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

        // Clone the event for the deferred command
        let event_clone = event.clone().unbind();

        if self.is_world {
            // Direct World access - trigger immediately
            let world = self.world_mut()?;

            // This is essentially the same logic as World.trigger()
            // Check if this is an entity-targeted event
            let target_entity = if event.hasattr("entity")? {
                let entity_attr = event.getattr("entity")?;
                Some(entity_attr.extract::<PyEntity>()?.0)
            } else {
                None
            };

            let registry = world.get_resource::<ObserverRegistry>();
            if let Some(registry) = registry
                && let Some(observers) = registry.get_observers_for_event(py, &event)?
            {
                let observers = observers.clone();
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
                        if let Some(entity) = target_entity {
                            let filter = BundleFilter {
                                components: bundle_filter.clone(),
                            };
                            if !filter.matches(world, entity) {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    }

                    let on_param = Py::new(
                        py,
                        PyOn {
                            event_data: event_clone.clone_ref(py),
                            entity: target_entity,
                        },
                    )?;
                    execute_system_func(py, &observer_entry.system_func, world, on_param)
                        .inspect_err(|e| {
                            e.print(py);
                        })?;
                }
            }
        } else {
            // Commands - queue the trigger for later
            self.execute_or_queue(move |world: &mut World| {
                Python::attach(|py| {
                    let event_bound = event_clone.bind(py);

                    // Check if this is an entity-targeted event
                    let target_entity = if event_bound.hasattr("entity").unwrap_or(false) {
                        event_bound
                            .getattr("entity")
                            .ok()
                            .and_then(|attr| attr.extract::<PyEntity>().ok())
                            .map(|e| e.0)
                    } else {
                        None
                    };

                    let registry = world.get_resource::<ObserverRegistry>();
                    if let Some(registry) = registry
                        && let Ok(Some(observers)) =
                            registry.get_observers_for_event(py, event_bound)
                    {
                        let observers = observers.clone();
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
                                if let Some(entity) = target_entity {
                                    let filter = BundleFilter {
                                        components: bundle_filter.clone(),
                                    };
                                    if !filter.matches(world, entity) {
                                        continue;
                                    }
                                } else {
                                    continue;
                                }
                            }

                            if let Ok(on_param) = Py::new(
                                py,
                                PyOn {
                                    event_data: event_clone.clone_ref(py),
                                    entity: target_entity,
                                },
                            ) && let Err(e) = execute_system_func(
                                py,
                                &observer_entry.system_func,
                                world,
                                on_param,
                            ) {
                                e.print(py);
                            }
                        }
                    }
                });
            })?;
        }

        Ok(())
    }
}
