use core::fmt;
use std::{alloc::Layout, any::TypeId, collections::HashMap};

use bevy::ecs::{
    component::{ComponentCloneBehavior, ComponentDescriptor, ComponentId, StorageType},
    ptr::OwningPtr,
    world::World,
};
/// Registry of custom Python components (Bevy resource).
///
/// The concrete type is the interpreter-neutral [`CustomComponentRegistry`],
/// re-exported under the crate-local name used throughout this adapter. It is keyed by
/// `type_ptr as usize`; call `.get(type_ptr as usize)` to look a component up.
pub use pybevy_core::custom_component::CustomComponentRegistry as ComponentRegistry;
use pybevy_core::{
    custom_component::{
        PythonObjectDescriptor, RegisterOutcome, register_custom_component_guarded,
    },
    registry::global_registry,
};
use pyo3::{PyTypeInfo, exceptions::PyTypeError, ffi::PyTypeObject, prelude::*, types::PyType};

use crate::ecs::{component::PyComponent, helpers::type_utils::get_python_type_name};

/// All components use dynamic dispatch via feature crate bridges or custom Python components.
#[derive(Debug, Clone, Copy)]
pub enum PyComponentType {
    /// Dynamically registered component from a feature crate
    /// Stores the Python type pointer for lookup in the global bridge registry
    Dynamic(*const PyTypeObject),
    /// Python-defined custom component (via @component decorator)
    Custom(*const PyTypeObject),
}

impl PartialEq for PyComponentType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PyComponentType::Dynamic(a), PyComponentType::Dynamic(b)) => a == b,
            (PyComponentType::Custom(a), PyComponentType::Custom(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for PyComponentType {}

impl std::hash::Hash for PyComponentType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            PyComponentType::Dynamic(ptr) => ptr.hash(state),
            PyComponentType::Custom(ptr) => ptr.hash(state),
        }
    }
}

impl PyComponentType {
    /// Register this component type with the world and return its ComponentId.
    /// For custom components, looks up the pre-registered ID from the HashMap.
    pub fn register_with_world(
        &self,
        world: &mut World,
        custom_component_ids: &HashMap<*const PyTypeObject, ComponentId>,
    ) -> ComponentId {
        match self {
            PyComponentType::Dynamic(type_ptr) => {
                if let Some(bridge) = global_registry::get_bridge_by_py_type(*type_ptr) {
                    bridge.register(world)
                } else {
                    panic!(
                        "Dynamic component bridge not found for type pointer {:p}",
                        type_ptr
                    )
                }
            }
            PyComponentType::Custom(type_ptr) => *custom_component_ids
                .get(type_ptr)
                .expect("Custom component not registered"),
        }
    }

    /// Register this component type with the world and return its ComponentId.
    /// For custom components, registers them on-demand using register_custom_component.
    /// Used by View API and batch operations where custom components aren't pre-registered.
    pub fn register_simple(&self, world: &mut World) -> ComponentId {
        match self {
            PyComponentType::Dynamic(type_ptr) => {
                if let Some(bridge) = global_registry::get_bridge_by_py_type(*type_ptr) {
                    bridge.register(world)
                } else {
                    panic!(
                        "Dynamic component bridge not found for type pointer {:p}",
                        type_ptr
                    )
                }
            }
            PyComponentType::Custom(type_ptr) => {
                let name = Python::attach(|py| get_python_type_name(py, *type_ptr));
                register_custom_component(world, *type_ptr, name)
            }
        }
    }

    /// Get the Rust TypeId for this component type.
    /// Returns None for custom components (they use Py<PyAny> storage).
    pub fn type_id(&self) -> Option<TypeId> {
        match self {
            PyComponentType::Dynamic(type_ptr) => global_registry::get_bridge_by_py_type(*type_ptr)
                .map(|bridge| bridge.bevy_type_id()),
            PyComponentType::Custom(_) => None,
        }
    }

    /// Try to convert from Python type object to PyComponentType.
    /// Returns Custom variant for decorated Python-defined components.
    pub fn try_from_py_type(ty: &Bound<'_, PyType>, py: Python<'_>) -> PyResult<Self> {
        if !ty.is_subclass_of::<PyComponent>()? {
            return Err(PyErr::new::<PyTypeError, _>(format!(
                "Expected a subclass of Component, but got: {:?} which is not a subclass of Component",
                ty
            )));
        }

        if ty.is(PyComponent::type_object(py)) {
            return Err(PyErr::new::<PyTypeError, _>(
                "Cannot use Component base class directly. Use a concrete component type.",
            ));
        }

        // Check dynamic registry for feature crate components
        let type_ptr = ty.as_type_ptr();
        if global_registry::contains_py_type(type_ptr) {
            return Ok(PyComponentType::Dynamic(type_ptr));
        }

        // Check for special Python-only built-in components (DespawnOnExit, DespawnOnEnter)
        if ty.is(crate::ecs::state::PyDespawnOnExit::type_object(py)) {
            return Ok(PyComponentType::Custom(type_ptr));
        }
        if ty.is(crate::ecs::state::PyDespawnOnEnter::type_object(py)) {
            return Ok(PyComponentType::Custom(type_ptr));
        }

        // Not a built-in or dynamic component - check for custom component decorator
        let has_decorator = ty
            .getattr("__pybevy_component_decorated__")
            .ok()
            .and_then(|marker| marker.is_truthy().ok())
            .unwrap_or(false);

        if !has_decorator {
            return Err(PyErr::new::<PyTypeError, _>(format!(
                "Component class '{}' must be decorated with @component decorator",
                ty.name()?
            )));
        }

        Ok(PyComponentType::Custom(type_ptr))
    }

    /// Extract component from entity and convert to Python object.
    /// For dynamic components: Delegates to bridge extract function.
    /// For custom components: Delegates to QueryRuntime helper (handles wrapper/PyObject storage).
    pub fn extract_from_entity<'py>(
        &self,
        entity: &mut pybevy_core::FilteredEntityAccess,
        component_id: bevy::ecs::component::ComponentId,
        validity: crate::ecs::helpers::validity_guard::ValidityFlagWithMode,
        py: pyo3::Python<'py>,
        query_iter: &crate::ecs::query::query_runtime::PyQueryIter,
        param_idx: usize,
    ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
        match self {
            PyComponentType::Dynamic(type_ptr) => {
                // Use pre-cached extraction function pointer indexed by param position
                if let Some(extract_fn) = query_iter.get_extract_fn(param_idx) {
                    extract_fn(entity, component_id, validity, py)
                } else {
                    // Fallback to global registry
                    if let Some(bridge) = global_registry::get_bridge_by_py_type(*type_ptr) {
                        bridge.extract(entity, component_id, validity, py)
                    } else {
                        Err(pyo3::exceptions::PyRuntimeError::new_err(
                            "Dynamic component bridge not found",
                        ))
                    }
                }
            }
            PyComponentType::Custom(ptr) => {
                query_iter.extract_custom_component(*ptr, entity, component_id, param_idx, py)
            }
        }
    }

    /// Insert component into entity via Commands API.
    #[allow(dead_code)] // used by pybevy_control
    pub fn insert_into_commands<'py>(
        &self,
        _commands: &crate::ecs::commands::PyCommands,
        _entity_id: bevy::ecs::entity::Entity,
        _component: &pyo3::Bound<'py, pyo3::PyAny>,
    ) -> pyo3::PyResult<()> {
        match self {
            PyComponentType::Dynamic(_) => Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Dynamic component insert must be handled by caller with World access",
            )),
            PyComponentType::Custom(_) => Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Custom component insert must be handled by caller",
            )),
        }
    }

    /// Remove component from entity via Commands API.
    #[allow(dead_code)] // used by pybevy_control
    pub fn remove_from_commands(
        &self,
        _commands: &crate::ecs::commands::PyCommands,
        _entity_id: bevy::ecs::entity::Entity,
    ) -> pyo3::PyResult<()> {
        match self {
            PyComponentType::Dynamic(_) => Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Dynamic component remove must be handled by caller with World access",
            )),
            PyComponentType::Custom(_) => Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Custom component remove must be handled by caller",
            )),
        }
    }
}

impl fmt::Display for PyComponentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PyComponentType::Dynamic(type_ptr) => {
                if let Some(bridge) = global_registry::get_bridge_by_py_type(*type_ptr) {
                    write!(f, "{}", bridge.name())
                } else {
                    write!(f, "Dynamic({:p})", *type_ptr)
                }
            }
            PyComponentType::Custom(type_ptr) => {
                let name = Python::attach(|py| get_python_type_name(py, *type_ptr));
                write!(f, "{}", name)
            }
        }
    }
}

impl TryFrom<(&Bound<'_, PyType>, Python<'_>)> for PyComponentType {
    type Error = PyErr;

    fn try_from((ty, py): (&Bound<'_, PyType>, Python)) -> Result<Self, Self::Error> {
        PyComponentType::try_from_py_type(ty, py)
    }
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for PyComponentType {}
unsafe impl Sync for PyComponentType {}

/// Drop function for custom Python objects (components and resources).
///
/// This is called by Bevy when a component/resource is removed.
/// It ensures that the Python reference count is properly decremented.
///
/// # Safety
/// The pointer must point to a valid `Py<PyAny>` instance that was stored
/// in the ECS.
pub(crate) unsafe fn drop_py_object(ptr: OwningPtr<'_>) {
    // SAFETY: The pointer is guaranteed to point to a valid Py<PyAny>
    // that was stored in the ECS. drop_as will properly run the
    // Drop implementation for Py<PyAny>, which decrements the Python
    // reference count.
    unsafe {
        ptr.drop_as::<Py<PyAny>>();
    }
}

/// Helper function to create a ComponentDescriptor for custom Python objects.
///
/// This is shared logic used by both custom component and custom resource registration.
/// Custom components/resources are stored as `Py<PyAny>` objects in the ECS.
///
/// # Arguments
/// * `name` - The name for the descriptor (from the Python class)
///
/// # Returns
/// A ComponentDescriptor configured for storing Python objects
pub(crate) fn create_python_object_descriptor(name: String) -> ComponentDescriptor {
    unsafe {
        ComponentDescriptor::new_with_layout(
            name,
            StorageType::Table,
            Layout::new::<Py<PyAny>>(),
            Some(drop_py_object),
            false,
            ComponentCloneBehavior::Default,
            None,
        )
    }
}

/// PyO3 implementation of [`PythonObjectDescriptor`].
///
/// Stores custom Python components/resources as `Py<PyAny>` objects in the ECS,
/// with a drop function that properly decrements Python reference counts.
pub(crate) struct Pyo3ObjectDescriptor;

impl PythonObjectDescriptor for Pyo3ObjectDescriptor {
    fn create(name: String) -> ComponentDescriptor {
        create_python_object_descriptor(name)
    }
}

/// Extract the fully qualified Python name (`module.qualname`) for a type pointer.
///
/// This matches the format used by `pybevy/decorators.py`:
///   `f"{cls.__module__}.{cls.__qualname__}"`
fn get_python_qualified_name(py: Python, type_ptr: *const PyTypeObject) -> Option<String> {
    // SAFETY: registered type pointers live for the interpreter lifetime
    let type_obj =
        unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };
    let cls = type_obj.cast::<pyo3::types::PyType>().ok()?;
    let module = cls.getattr("__module__").ok()?.extract::<String>().ok()?;
    let qualname = cls.getattr("__qualname__").ok()?.extract::<String>().ok()?;
    Some(format!("{}.{}", module, qualname))
}

/// Helper function to register a custom Python component with Bevy's ECS.
///
/// This function determines whether to use wrapper storage (for primitive-only components)
/// or PyAny storage (for components with complex types) and registers the appropriate
/// ComponentDescriptor with the world.
///
/// The component registry is stored as a resource in the World to ensure proper scoping
/// per-app instance. The registry uses PyTypeObject pointers as keys for type identity.
///
/// During hot reload, Python re-executes `@component` classes creating new PyTypeObject
/// pointers. This function detects that case via a name-based lookup and adds the new
/// pointer as an alias for the existing ComponentId, so entities registered with the old
/// pointer remain visible to queries using the new pointer.
///
/// # Arguments
/// * `world` - The Bevy world to register the component in
/// * `type_ptr` - The PyTypeObject pointer identifying the Python component class
/// * `name` - The name of the component (from the Python class) for the descriptor
///
/// # Returns
/// The ComponentId of the registered component
pub(crate) fn register_custom_component(
    world: &mut World,
    type_ptr: *const PyTypeObject,
    name: String,
) -> ComponentId {
    use crate::ecs::component_layout::{ComponentStorageType, ComponentStorageTypeExt};

    // The neutral registry is created by the guarded register below; ensure the
    // MCP-facing metadata resource exists so we can keep it in sync afterwards.
    if !world.contains_resource::<pybevy_core::CustomComponentInfo>() {
        world.insert_resource(pybevy_core::CustomComponentInfo::default());
    }

    // Backend leaves: type identity, qualified name (for hot-reload aliasing),
    // and the storage type recomputed from the *live* class every spawn (so a
    // mutated `__pybevy_storage__` / `__annotations__` is caught by the guard).
    let type_id = type_ptr as usize;
    let (qualified_name, storage_type) = Python::attach(|py| {
        let qname = get_python_qualified_name(py, type_ptr);
        // SAFETY: registered type pointers live for the interpreter lifetime.
        let py_type =
            unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };
        let storage = match py_type.cast::<pyo3::types::PyType>() {
            Ok(cls) => ComponentStorageType::from_python_class(cls)
                .unwrap_or(ComponentStorageType::PyObject),
            Err(_) => ComponentStorageType::PyObject,
        };
        (qname, storage)
    });

    // The shared storage-flip guard + hot-reload aliasing lives in pybevy_core.
    let generation = world
        .get_resource::<pybevy_reload::HotReloadGeneration>()
        .map_or(0, |generation| generation.current);
    let outcome = register_custom_component_guarded::<Pyo3ObjectDescriptor>(
        world,
        type_id,
        &name,
        qualified_name.as_deref(),
        storage_type,
        generation,
    );

    // Mirror the guard's decision into the MCP-facing CustomComponentInfo.
    match outcome {
        RegisterOutcome::Reused(_) => {}
        RegisterOutcome::Aliased(id) => {
            world
                .resource_mut::<pybevy_core::CustomComponentInfo>()
                .update_type_ptr(id, type_ptr);
        }
        RegisterOutcome::Registered { id, evicted } => {
            let mut info = world.resource_mut::<pybevy_core::CustomComponentInfo>();
            if let Some(stale) = evicted {
                info.remove(stale);
            }
            info.insert(
                id,
                pybevy_core::CustomComponentEntry {
                    type_ptr,
                    name,
                    is_pyobject_storage: matches!(storage_type, ComponentStorageType::PyObject),
                },
            );
        }
    }

    outcome.id()
}

/// Helper function to register a component and get its ComponentId.
///
/// This function centralizes the component type to Rust type mapping and registration,
/// eliminating duplication across query_runtime.rs and view.rs.
///
/// # Arguments
/// * `world` - The Bevy world to register the component in
/// * `comp_type` - The PyComponentType to register
/// * `custom_component_ids` - Map of custom component type pointers to their ComponentIds
///
/// # Returns
/// The ComponentId of the registered component
pub fn register_component_id(
    world: &mut World,
    comp_type: &PyComponentType,
    custom_component_ids: &HashMap<*const PyTypeObject, ComponentId>,
) -> ComponentId {
    comp_type.register_with_world(world, custom_component_ids)
}
