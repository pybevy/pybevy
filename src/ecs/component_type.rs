use std::{alloc::Layout, any::TypeId, collections::HashMap, sync::Arc};

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
    public_error::{RESOURCE_COMPONENT_INSERT, RESOURCE_COMPONENT_REMOVE},
    registry::global_registry,
};
use pyo3::{PyTypeInfo, exceptions::PyTypeError, ffi::PyTypeObject, prelude::*, types::PyType};
use smallvec::SmallVec;

use crate::ecs::{
    component::PyComponent,
    component_layout::{
        ComponentLayout, ComponentLayoutExt, ComponentStorageType, ComponentStorageTypeExt,
    },
    helpers::type_utils::get_python_type_name,
    resource::{PyRes, PyResMut, PyResource},
    resource_type::register_custom_resource,
};

/// All components use dynamic dispatch via feature crate bridges or custom Python components.
#[derive(Debug, Clone, Copy)]
pub enum PyComponentType {
    /// Dynamically registered component from a feature crate
    /// Stores the Python type pointer for lookup in the global bridge registry
    Dynamic(*const PyTypeObject),
    /// Native Bevy resource stored on its resource entity.
    Resource(*const PyTypeObject),
    /// Python-defined custom component (via @component decorator)
    Custom(*const PyTypeObject),
}

/// Strong references keeping Python-owned component/resource pointers
/// dereferenceable.
///
/// A param stores its component classes as bare `*const PyTypeObject` and
/// outlives the expression that built it, while nothing else is obliged to keep
/// a `@component` or `@resource` class alive. Callers build params from classes
/// the user just supplied, which is what makes reading the pointer here sound.
///
/// The returned handles must be reported from the owner's `__traverse__`, or
/// the retained class pins its defining module's namespace. See docs/safety.md
/// section 6.
pub(crate) fn retain_custom_classes(
    py: Python<'_>,
    types: impl IntoIterator<Item = PyComponentType>,
) -> SmallVec<[Arc<Py<PyType>>; 4]> {
    let mut retained: SmallVec<[Arc<Py<PyType>>; 4]> = SmallVec::new();
    for component in types {
        // Native wrapper classes are module attributes and outlive any param.
        // A Resource without a native bridge is a Python `@resource` class and
        // has the same lifetime requirements as a custom component.
        let type_ptr = match component {
            PyComponentType::Custom(type_ptr) => type_ptr,
            PyComponentType::Resource(type_ptr)
                if global_registry::get_resource_bridge_by_py_type(type_ptr).is_none() =>
            {
                type_ptr
            }
            PyComponentType::Dynamic(_) | PyComponentType::Resource(_) => continue,
        };
        let object_ptr = type_ptr.cast_mut().cast::<pyo3::ffi::PyObject>();
        if retained.iter().any(|held| held.as_ptr() == object_ptr) {
            continue;
        }
        // SAFETY: the caller supplied this class in the expression being built,
        // so the pointer is live for the duration of this call.
        let object = unsafe { Bound::from_borrowed_ptr(py, object_ptr) };
        if let Ok(class) = object.cast_into::<PyType>() {
            retained.push(Arc::new(class.unbind()));
        }
    }
    retained
}

/// Deep-clone retained class handles so each holder owns independent increfs.
///
/// Sharing the `Arc` instead would let two GC-visible owners report the same
/// incref, and a double visit makes the collector under-count and clear a live
/// class. Every pyclass that both stores retained classes and traverses them
/// must clone this way rather than deriving `Clone`. See docs/safety.md
/// section 6.
pub(crate) fn clone_retained_classes(
    py: Python<'_>,
    retained: &[Arc<Py<PyType>>],
) -> SmallVec<[Arc<Py<PyType>>; 4]> {
    retained
        .iter()
        .map(|class| Arc::new(class.as_ref().clone_ref(py)))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ValidationIdentity {
    Native(TypeId),
    Python(usize),
}

impl PartialEq for PyComponentType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PyComponentType::Dynamic(a), PyComponentType::Dynamic(b)) => a == b,
            (PyComponentType::Resource(a), PyComponentType::Resource(b)) => a == b,
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
            PyComponentType::Resource(ptr) => ptr.hash(state),
            PyComponentType::Custom(ptr) => ptr.hash(state),
        }
    }
}

impl PyComponentType {
    pub(crate) fn display_name(&self, py: Python<'_>) -> String {
        match self {
            Self::Dynamic(type_ptr) => global_registry::get_bridge_by_py_type(*type_ptr)
                .map_or_else(
                    || format!("Dynamic({:p})", *type_ptr),
                    |bridge| bridge.name().to_owned(),
                ),
            Self::Resource(type_ptr) => global_registry::get_resource_bridge_by_py_type(*type_ptr)
                .map_or_else(
                    || format!("Resource({:p})", *type_ptr),
                    |bridge| bridge.name().to_owned(),
                ),
            Self::Custom(type_ptr) => get_python_type_name(py, *type_ptr),
        }
    }

    /// Canonical validation key. Native aliases collapse to their Bevy type;
    /// custom Python components retain their exact class identity.
    pub(crate) fn validation_identity(&self) -> ValidationIdentity {
        match self {
            Self::Dynamic(ptr) => global_registry::get_bridge_by_py_type(*ptr)
                .map_or(ValidationIdentity::Python(*ptr as usize), |bridge| {
                    ValidationIdentity::Native(bridge.bevy_type_id())
                }),
            Self::Resource(ptr) => global_registry::get_resource_bridge_by_py_type(*ptr)
                .map_or(ValidationIdentity::Python(*ptr as usize), |bridge| {
                    ValidationIdentity::Native(bridge.bevy_type_id())
                }),
            Self::Custom(ptr) => ValidationIdentity::Python(*ptr as usize),
        }
    }

    pub fn supports_mutable_access(&self) -> bool {
        match self {
            Self::Resource(ptr) => global_registry::get_resource_bridge_by_py_type(*ptr)
                .is_none_or(|bridge| bridge.is_mutable()),
            Self::Dynamic(_) | Self::Custom(_) => true,
        }
    }

    /// Register this component type with the world and return its ComponentId.
    /// For custom components, looks up the pre-registered ID from the HashMap.
    pub fn register_with_world(
        &self,
        world: &mut World,
        custom_component_ids: &HashMap<*const PyTypeObject, ComponentId>,
        py: Python,
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
            PyComponentType::Resource(type_ptr) => {
                if let Some(bridge) = global_registry::get_resource_bridge_by_py_type(*type_ptr) {
                    bridge.register_resource_id(world)
                } else {
                    register_custom_resource(world, *type_ptr, py)
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
    pub fn register_simple(&self, world: &mut World, py: Python) -> ComponentId {
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
            PyComponentType::Resource(type_ptr) => {
                if let Some(bridge) = global_registry::get_resource_bridge_by_py_type(*type_ptr) {
                    bridge.register_resource_id(world)
                } else {
                    register_custom_resource(world, *type_ptr, py)
                }
            }
            PyComponentType::Custom(type_ptr) => register_custom_component(world, *type_ptr, py),
        }
    }

    /// Get the Rust TypeId for this component type.
    /// Returns None for custom components (they use Py<PyAny> storage).
    pub fn type_id(&self) -> Option<TypeId> {
        match self {
            PyComponentType::Dynamic(type_ptr) => global_registry::get_bridge_by_py_type(*type_ptr)
                .map(|bridge| bridge.bevy_type_id()),
            PyComponentType::Resource(type_ptr) => {
                global_registry::get_resource_bridge_by_py_type(*type_ptr)
                    .map(|bridge| bridge.bevy_type_id())
            }
            PyComponentType::Custom(_) => None,
        }
    }

    /// Try to convert from Python type object to PyComponentType.
    /// Returns Custom variant for decorated Python-defined components.
    pub fn try_from_py_type(ty: &Bound<'_, PyType>, py: Python<'_>) -> PyResult<Self> {
        if !ty.is_subclass_of::<PyComponent>()? {
            let name = ty
                .qualname()
                .ok()
                .and_then(|name| name.extract::<String>().ok())
                .or_else(|| {
                    ty.name()
                        .ok()
                        .and_then(|name| name.extract::<String>().ok())
                })
                .unwrap_or_else(|| "<unknown>".to_string());
            return Err(PyErr::new::<PyTypeError, _>(format!(
                "expected a Component subclass, not {name}"
            )));
        }

        if ty.is(PyComponent::type_object(py)) {
            return Err(PyErr::new::<PyTypeError, _>(
                "Cannot use Component base class directly. Use a concrete component type.",
            ));
        }
        if ty.is(PyResource::type_object(py)) {
            return Err(PyTypeError::new_err(
                "Cannot use Resource base class directly. Use a concrete resource type.",
            ));
        }

        // Check dynamic registry for feature crate components
        let type_ptr = ty.as_type_ptr();
        if let Some(bridge) = global_registry::get_bridge_by_py_type(type_ptr) {
            // Native subclasses share the canonical Bevy component identity.
            return Ok(PyComponentType::Dynamic(bridge.py_type_ptr()));
        }

        if let Some(bridge) = global_registry::get_resource_bridge_by_py_type(type_ptr) {
            return Ok(PyComponentType::Resource(bridge.py_type_ptr()));
        }

        if ty.is_subclass_of::<PyResource>()? {
            let decorated = ty
                .getattr("__pybevy_resource_decorated__")
                .ok()
                .and_then(|marker| marker.is_truthy().ok())
                .unwrap_or(false);
            if decorated {
                return Ok(PyComponentType::Resource(type_ptr));
            }
            return Err(PyTypeError::new_err(format!(
                "resource type '{}' has no component-query bridge",
                ty.name()?
            )));
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
    ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
        match self {
            PyComponentType::Dynamic(type_ptr) => {
                if let Some(extract_fn) = query_iter.get_extract_fn(self) {
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
            PyComponentType::Resource(type_ptr) => {
                if let Some(bridge) = global_registry::get_resource_bridge_by_py_type(*type_ptr) {
                    bridge.extract(entity, component_id, validity, py)
                } else {
                    let value = if validity.access_mode() == pybevy_core::AccessMode::Write {
                        let mut value = entity.get_mut_by_id(component_id).ok_or_else(|| {
                            pyo3::exceptions::PyRuntimeError::new_err(
                                "Custom resource not found on matched resource entity",
                            )
                        })?;
                        // SAFETY: custom resource IDs use Pyo3ResourceObjectDescriptor.
                        unsafe { value.as_mut().deref_mut::<Py<PyAny>>().clone_ref(py) }
                    } else {
                        let value = entity.get_by_id(component_id).ok_or_else(|| {
                            pyo3::exceptions::PyRuntimeError::new_err(
                                "Custom resource not found on matched resource entity",
                            )
                        })?;
                        // SAFETY: custom resource IDs use Pyo3ResourceObjectDescriptor.
                        unsafe { value.deref::<Py<PyAny>>().clone_ref(py) }
                    };
                    if validity.access_mode() == pybevy_core::AccessMode::Write {
                        Ok(Py::new(py, PyResMut::new(value.bind(py).clone()))?.into_any())
                    } else {
                        Ok(Py::new(py, PyRes::new(value.bind(py).clone()))?.into_any())
                    }
                }
            }
            PyComponentType::Custom(ptr) => {
                let access_mode = validity.access_mode();
                query_iter.extract_custom_component(*ptr, entity, component_id, access_mode, py)
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
            PyComponentType::Resource(_) => Err(pyo3::exceptions::PyTypeError::new_err(
                RESOURCE_COMPONENT_INSERT,
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
            PyComponentType::Resource(_) => Err(pyo3::exceptions::PyTypeError::new_err(
                RESOURCE_COMPONENT_REMOVE,
            )),
            PyComponentType::Custom(_) => Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Custom component remove must be handled by caller",
            )),
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

/// Create the immutable descriptor used by custom Python components.
///
/// Custom components are stored as `Py<PyAny>` objects and mark changes through
/// their Python proxy hooks rather than Bevy's mutable untyped access.
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

pub(crate) fn create_python_resource_object_descriptor(name: String) -> ComponentDescriptor {
    // SAFETY: the descriptor layout and drop function both describe Py<PyAny>.
    // Custom resources are mutable so Bevy's by-ID resource access can produce
    // MutUntyped and stamp change ticks for ResMut/Query[Mut].
    unsafe {
        ComponentDescriptor::new_with_layout(
            name,
            StorageType::Table,
            Layout::new::<Py<PyAny>>(),
            Some(drop_py_object),
            true,
            ComponentCloneBehavior::Default,
            None,
        )
    }
}

/// PyO3 implementation of [`PythonObjectDescriptor`].
///
/// Stores custom Python components as `Py<PyAny>` objects in the ECS, with a
/// drop function that properly decrements Python reference counts.
pub(crate) struct Pyo3ObjectDescriptor;

impl PythonObjectDescriptor for Pyo3ObjectDescriptor {
    fn create(name: String) -> ComponentDescriptor {
        create_python_object_descriptor(name)
    }
}

/// Mutable `Py<PyAny>` descriptor used by custom Python resources.
pub(crate) struct Pyo3ResourceObjectDescriptor;

impl PythonObjectDescriptor for Pyo3ResourceObjectDescriptor {
    fn create(name: String) -> ComponentDescriptor {
        create_python_resource_object_descriptor(name)
    }
}

/// Extract the fully qualified Python name (`module.qualname`) for a live type pointer.
///
/// This matches the format used by `pybevy/decorators.py`:
///   `f"{cls.__module__}.{cls.__qualname__}"`
fn get_python_qualified_name(py: Python, type_ptr: *const PyTypeObject) -> Option<String> {
    // SAFETY: the caller obtains this pointer from a live Python class and
    // retains that class before returning from registration.
    let type_obj =
        unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };
    let cls = type_obj.cast::<pyo3::types::PyType>().ok()?;
    let module = cls.getattr("__module__").ok()?.extract::<String>().ok()?;
    let qualname = cls.getattr("__qualname__").ok()?.extract::<String>().ok()?;
    Some(format!("{}.{}", module, qualname))
}

/// Custom component metadata resolved before an insertion is queued.
///
/// Everything here is read off the live Python class while the interpreter is
/// attached, so applying the registration later needs no Python access.
#[derive(Debug, Clone)]
pub(crate) struct PreparedCustomComponentRegistration {
    type_id: usize,
    name: String,
    qualified_name: Option<String>,
    storage_type: ComponentStorageType,
    wrapper_layout: Option<Arc<ComponentLayout>>,
    /// Strong reference keeping `type_id`'s class alive; see CustomComponentEntry.
    retained_type: Option<Arc<Py<PyType>>>,
}

impl PreparedCustomComponentRegistration {
    pub(crate) fn from_python_class(cls: &Bound<'_, PyType>) -> PyResult<Self> {
        let type_ptr = cls.as_type_ptr();
        let storage_type = ComponentStorageType::from_python_class(cls)?;
        Ok(Self {
            type_id: type_ptr as usize,
            name: cls.name()?.to_string(),
            qualified_name: get_python_qualified_name(cls.py(), type_ptr),
            storage_type,
            wrapper_layout: wrapper_layout_for(cls, storage_type),
            retained_type: Some(Arc::new(cls.clone().unbind())),
        })
    }

    pub(crate) fn storage_type(&self) -> ComponentStorageType {
        self.storage_type
    }
}

/// Field layout for the MCP-facing metadata; only wrapper storage has one.
fn wrapper_layout_for(
    cls: &Bound<'_, PyType>,
    storage: ComponentStorageType,
) -> Option<Arc<ComponentLayout>> {
    matches!(storage, ComponentStorageType::Wrapper(_))
        .then(|| ComponentLayout::from_annotations(cls).ok())
        .flatten()
        .map(Arc::new)
}

pub(crate) fn register_prepared_custom_component(
    world: &mut World,
    prepared: &PreparedCustomComponentRegistration,
) -> ComponentId {
    let type_ptr = prepared.type_id as *const PyTypeObject;

    if !world.contains_resource::<pybevy_core::CustomComponentInfo>() {
        world.insert_resource(pybevy_core::CustomComponentInfo::default());
    }

    let generation = world
        .get_resource::<pybevy_reload::HotReloadGeneration>()
        .map_or(0, |generation| generation.current);
    let wrapper_schema = prepared
        .wrapper_layout
        .as_deref()
        .map(ComponentLayout::schema);
    let outcome = register_custom_component_guarded::<Pyo3ObjectDescriptor>(
        world,
        prepared.type_id,
        &prepared.name,
        prepared.qualified_name.as_deref(),
        prepared.storage_type,
        wrapper_schema.as_ref(),
        generation,
    );

    // Mirror the current class into the MCP-facing metadata on every outcome.
    // A Full reload clears this metadata so classes removed from the new scene
    // cannot be constructed through control APIs, while the neutral registry
    // may retain their old aliases temporarily for rollback safety. Reused and
    // aliased classes therefore need to repopulate the metadata too.
    let mut retired_entries = Vec::new();
    {
        let mut info = world.resource_mut::<pybevy_core::CustomComponentInfo>();
        if let RegisterOutcome::Registered {
            evicted: Some(stale),
            ..
        } = outcome
            && let Some(retired) = info.remove(stale)
        {
            retired_entries.push(retired);
        }
        if let Some(retired) = info.replace(
            outcome.id(),
            pybevy_core::CustomComponentEntry {
                type_ptr,
                retained_type: prepared.retained_type.clone(),
                name: prepared.name.clone(),
                is_pyobject_storage: matches!(
                    prepared.storage_type,
                    ComponentStorageType::PyObject
                ),
                wrapper_layout: prepared.wrapper_layout.clone(),
            },
        ) {
            retired_entries.push(retired);
        }
    }
    // Dropping a retained class can run Python finalization. Keep that outside
    // the Bevy resource borrow and under an attached interpreter context.
    if !retired_entries.is_empty() {
        Python::attach(|_| drop(retired_entries));
    }

    outcome.id()
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
pub(crate) fn register_custom_component(
    world: &mut World,
    type_ptr: *const PyTypeObject,
    py: Python,
) -> ComponentId {
    // Storage type is recomputed from the *live* class every spawn, so a mutated
    // `__pybevy_storage__` / `__annotations__` is caught by the guard.
    let qualified_name = get_python_qualified_name(py, type_ptr);
    let name = get_python_type_name(py, type_ptr);
    // SAFETY: the registration caller passes a live Python class pointer;
    // this block immediately turns it into an owned handle.
    let py_type =
        unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };
    let (storage_type, wrapper_layout, retained_type) = match py_type.cast::<PyType>() {
        Ok(cls) => {
            let storage = ComponentStorageType::from_python_class(cls)
                .unwrap_or(ComponentStorageType::PyObject);
            (
                storage,
                wrapper_layout_for(cls, storage),
                Some(Arc::new(cls.clone().unbind())),
            )
        }
        Err(_) => (ComponentStorageType::PyObject, None, None),
    };

    let prepared = PreparedCustomComponentRegistration {
        type_id: type_ptr as usize,
        name,
        qualified_name,
        storage_type,
        wrapper_layout,
        retained_type,
    };
    register_prepared_custom_component(world, &prepared)
}

/// Helper function to register a component and get its ComponentId.
///
/// This function centralizes the component type to Rust type mapping and registration,
/// eliminating duplication across query_runtime.rs and view.rs.
pub fn register_component_id(
    world: &mut World,
    comp_type: &PyComponentType,
    custom_component_ids: &HashMap<*const PyTypeObject, ComponentId>,
    py: Python,
) -> ComponentId {
    comp_type.register_with_world(world, custom_component_ids, py)
}
