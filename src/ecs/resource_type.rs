use core::fmt;

use bevy::{
    ecs::{component::ComponentId, world::unsafe_world_cell::UnsafeWorldCell},
    prelude::*,
};
/// Main/PyO3 name for the backend-neutral custom-resource identity registry.
pub use pybevy_core::custom_resource::CustomResourceRegistry as ResourceRegistry;
use pybevy_core::{
    custom_resource::{
        ResourceRegisterOutcome, insert_dynamic_resource_value, register_custom_resource_guarded,
    },
    public_error::{
        ASSET_SERVER_MANUAL_INSERT, ASSET_SERVER_MANUAL_REMOVE, RESOURCE_BRIDGE_NOT_FOUND,
        expected_resource_subclass, resource_decorator_required, resource_not_present,
        resource_type_not_found, state_resource_descriptor_required,
    },
    registry::global_registry,
    resource_initializer,
};
use pyo3::{PyTypeInfo, exceptions::PyTypeError, ffi::PyTypeObject, prelude::*, types::PyType};

use crate::{
    app::hot_reload::bindings::PyHotReloadControl,
    assets::{asset_server::PyAssetServer, assets::PyAssets},
    ecs::{
        component_type::Pyo3ResourceObjectDescriptor,
        helpers::{
            type_utils::get_python_type_name,
            validity_guard::{AccessMode, ValidityFlag},
        },
        messages::PyMessages,
        state::{PyNextState, PyState},
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PyResourceType {
    AssetServer,
    /// Dynamic resources from feature crates (use bridge dispatch)
    Dynamic(*const PyTypeObject),
    /// Custom Python-defined resources
    Custom(*const PyTypeObject),
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for PyResourceType {}
unsafe impl Sync for PyResourceType {}

pub(crate) fn reject_state_type_as_resource(ty: &Bound<'_, PyType>) -> PyResult<()> {
    if ty.hasattr("__pybevy_state__")? {
        return Err(PyTypeError::new_err(state_resource_descriptor_required(
            ty.name()?,
        )));
    }
    Ok(())
}

impl PyResourceType {
    /// Get AssetServer resource from world (read-only)
    fn get_asset_server(world: &World, py: Python, validity: ValidityFlag) -> PyResult<Py<PyAny>> {
        Self::get_asset_server_cell(world.as_unsafe_world_cell_readonly(), py, validity)
    }

    /// Get AssetServer resource from world (mutable)
    fn get_asset_server_mut(
        world: &mut World,
        py: Python,
        validity: ValidityFlag,
    ) -> PyResult<Py<PyAny>> {
        Self::get_asset_server_cell(world.as_unsafe_world_cell(), py, validity)
    }

    /// Wrap the AssetServer resource from a world cell.
    fn get_asset_server_cell(
        cell: UnsafeWorldCell,
        py: Python,
        validity: ValidityFlag,
    ) -> PyResult<Py<PyAny>> {
        // SAFETY: `cell` references the world the AssetServer belongs to and stays
        // valid while `validity` is active; PyAssetServer reads only the declared
        // AssetServer resource through it.
        let py_asset_server = unsafe { PyAssetServer::new(cell, validity) };
        let asset_server_obj = Py::new(py, resource_initializer(py_asset_server))?;
        Ok(asset_server_obj.into_any())
    }

    /// Read a custom Python resource through its dynamic resource component.
    ///
    /// # Safety
    /// The caller must guarantee reads of `ResourceRegistry` and the resolved resource
    /// component are declared, and that the executor prevents a concurrent writer.
    unsafe fn get_custom_from_cell(
        cell: UnsafeWorldCell,
        type_ptr: *const PyTypeObject,
        py: Python,
        mutable: bool,
    ) -> PyResult<Py<PyAny>> {
        // SAFETY: read access to ResourceRegistry is declared for Custom params.
        let registry = unsafe { cell.get_resource::<ResourceRegistry>() }.ok_or_else(|| {
            let type_name = get_python_type_name(py, type_ptr);
            PyTypeError::new_err(resource_type_not_found(type_name))
        })?;

        let component_id = registry.get(type_ptr as usize).ok_or_else(|| {
            let type_name = get_python_type_name(py, type_ptr);
            PyTypeError::new_err(resource_type_not_found(type_name))
        })?;

        let resource = if mutable {
            // SAFETY: the caller declared write access to this dynamic resource ID.
            let mut value = unsafe { cell.get_resource_mut_by_id(component_id) };
            value.as_mut().map(|value| {
                // `as_mut` marks the resource changed before exposing its value.
                // The value escapes as an owned Python object, so there is no
                // DerefMut to hook: `ResMut[T]` marks on materialization, not on
                // first write. Conservative (spurious Changed, never a missed
                // one) and matches what `Query[Mut[T]]` already does.
                let value = value.as_mut();
                // SAFETY: registration uses Pyo3ResourceObjectDescriptor for this ID.
                unsafe { value.deref_mut::<Py<PyAny>>().clone_ref(py) }
            })
        } else {
            // SAFETY: custom Res declares conservative write access to exclude
            // other systems while this opaque Python value is reachable.
            unsafe { cell.get_resource_by_id(component_id) }.map(|value| {
                // SAFETY: registration uses Pyo3ResourceObjectDescriptor for this ID.
                unsafe { value.deref::<Py<PyAny>>().clone_ref(py) }
            })
        };

        resource.ok_or_else(|| {
            let type_name = get_python_type_name(py, type_ptr);
            PyTypeError::new_err(resource_not_present(type_name))
        })
    }

    /// Read a resource through an `UnsafeWorldCell`, touching only this resource's
    /// declared data instead of borrowing the whole world (as `get_from_world` does).
    ///
    /// # Safety
    /// The caller must guarantee `DynamicSystem::initialize` declared read access to
    /// this resource (and, for Custom resources, to `ResourceRegistry`) and that
    /// the executor prevents a concurrent writer, so
    /// the cell's unchecked reads are unique.
    pub unsafe fn get_from_cell(
        &self,
        cell: UnsafeWorldCell,
        py: Python,
        validity: ValidityFlag,
    ) -> PyResult<Py<PyAny>> {
        match self {
            PyResourceType::AssetServer => Self::get_asset_server_cell(cell, py, validity),
            PyResourceType::Dynamic(type_ptr) => {
                let bridge = global_registry::get_resource_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyTypeError::new_err(RESOURCE_BRIDGE_NOT_FOUND))?;
                // SAFETY: read access to this resource is declared; the executor
                // prevents a concurrent writer, so the cell read is unique.
                unsafe {
                    bridge.get_from_cell(cell, validity.with_access_mode(AccessMode::Read), py)
                }
            }
            PyResourceType::Custom(type_ptr) => {
                // SAFETY: forwarded obligation, see get_custom_from_cell.
                unsafe { Self::get_custom_from_cell(cell, *type_ptr, py, false) }
            }
        }
    }

    /// Mutable counterpart of `get_from_cell`. Custom resources return the same
    /// stored Python object (Python objects are inherently mutable), matching
    /// `get_from_world_mut`.
    ///
    /// # Safety
    /// The caller must guarantee `DynamicSystem::initialize` declared write access
    /// to this resource (Custom resources only require the declared storage reads)
    /// and that the executor prevents a concurrent access, so the cell's unchecked
    /// borrow is unique.
    pub unsafe fn get_from_cell_mut(
        &self,
        cell: UnsafeWorldCell,
        py: Python,
        validity: ValidityFlag,
    ) -> PyResult<Py<PyAny>> {
        match self {
            PyResourceType::AssetServer => Self::get_asset_server_cell(cell, py, validity),
            PyResourceType::Dynamic(type_ptr) => {
                let bridge = global_registry::get_resource_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyTypeError::new_err(RESOURCE_BRIDGE_NOT_FOUND))?;
                // SAFETY: write access to this resource is declared; the executor
                // prevents any concurrent access, so the cell borrow is unique.
                unsafe {
                    bridge.get_mut_from_cell(cell, validity.with_access_mode(AccessMode::Write), py)
                }
            }
            PyResourceType::Custom(type_ptr) => {
                // Custom mutable access returns the same Python object as the read
                // path (Python objects are inherently mutable).
                // SAFETY: forwarded obligation, see get_custom_from_cell.
                unsafe { Self::get_custom_from_cell(cell, *type_ptr, py, true) }
            }
        }
    }

    /// Get the resource from the world and convert it to a Python object (read-only access)
    pub fn get_from_world(
        &self,
        world: &World,
        py: Python,
        validity: ValidityFlag,
    ) -> PyResult<Py<PyAny>> {
        match self {
            PyResourceType::AssetServer => Self::get_asset_server(world, py, validity),
            PyResourceType::Dynamic(type_ptr) => {
                let bridge = global_registry::get_resource_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyTypeError::new_err(RESOURCE_BRIDGE_NOT_FOUND))?;
                bridge.get(world, validity.with_access_mode(AccessMode::Read), py)
            }
            PyResourceType::Custom(type_ptr) => {
                // For custom resources, we need to look up the ComponentId from the registry
                let registry = world.get_resource::<ResourceRegistry>().ok_or_else(|| {
                    // Get the type name for a better error message
                    let type_name = get_python_type_name(py, *type_ptr);
                    PyTypeError::new_err(resource_type_not_found(type_name))
                })?;

                let component_id = registry.get(*type_ptr as usize).ok_or_else(|| {
                    // Get the type name for a better error message
                    let type_name = get_python_type_name(py, *type_ptr);
                    PyTypeError::new_err(resource_type_not_found(type_name))
                })?;

                let resource = world.get_resource_by_id(component_id).ok_or_else(|| {
                    // Get the type name for a better error message
                    let type_name = get_python_type_name(py, *type_ptr);
                    PyTypeError::new_err(resource_not_present(type_name))
                })?;

                // SAFETY: registration uses Pyo3ResourceObjectDescriptor for this ID.
                Ok(unsafe { resource.deref::<Py<PyAny>>().clone_ref(py) })
            }
        }
    }

    /// Get the resource from the world and convert it to a Python object (mutable access)
    pub fn get_from_world_mut(
        &self,
        world: &mut World,
        py: Python,
        validity: ValidityFlag,
    ) -> PyResult<Py<PyAny>> {
        match self {
            PyResourceType::AssetServer => Self::get_asset_server_mut(world, py, validity),
            PyResourceType::Dynamic(type_ptr) => {
                let bridge = global_registry::get_resource_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyTypeError::new_err(RESOURCE_BRIDGE_NOT_FOUND))?;
                bridge.get_mut(world, validity.with_access_mode(AccessMode::Write), py)
            }
            PyResourceType::Custom(type_ptr) => {
                let component_id = world
                    .get_resource::<ResourceRegistry>()
                    .and_then(|registry| registry.get(*type_ptr as usize))
                    .ok_or_else(|| {
                        let type_name = get_python_type_name(py, *type_ptr);
                        PyTypeError::new_err(resource_type_not_found(type_name))
                    })?;
                let mut resource = world.get_resource_mut_by_id(component_id).ok_or_else(|| {
                    let type_name = get_python_type_name(py, *type_ptr);
                    PyTypeError::new_err(resource_not_present(type_name))
                })?;
                // SAFETY: registration uses Pyo3ResourceObjectDescriptor for this ID.
                Ok(unsafe { resource.as_mut().deref_mut::<Py<PyAny>>().clone_ref(py) })
            }
        }
    }

    /// Insert a Python resource instance into the world
    pub fn insert_into_world(
        &self,
        world: &mut World,
        py: Python,
        resource_instance: Py<PyAny>,
    ) -> PyResult<()> {
        match self {
            PyResourceType::AssetServer => Err(PyTypeError::new_err(ASSET_SERVER_MANUAL_INSERT)),
            PyResourceType::Dynamic(type_ptr) => {
                let bridge = global_registry::get_resource_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyTypeError::new_err(RESOURCE_BRIDGE_NOT_FOUND))?;
                bridge.insert(world, resource_instance.bind(py))
            }
            PyResourceType::Custom(type_ptr) => {
                // Get the resource name from the Python type. The neutral
                // registration path is idempotent and handles reload aliases.
                // SAFETY: registered type pointers live for the interpreter lifetime.
                let type_obj =
                    unsafe { Bound::from_borrowed_ptr(py, *type_ptr as *mut pyo3::ffi::PyObject) };
                type_obj.cast::<PyType>()?;
                let component_id = register_custom_resource(world, *type_ptr, py);

                // SAFETY: register_custom_resource created this ID with a descriptor for
                // Py<PyAny>, and all ordinary component insertion paths reject resources.
                unsafe {
                    insert_dynamic_resource_value(world, component_id, resource_instance);
                }

                Ok(())
            }
        }
    }

    /// Remove a Python resource from the world
    pub fn remove_from_world(&self, world: &mut World, _py: Python) -> PyResult<()> {
        match self {
            PyResourceType::AssetServer => Err(PyTypeError::new_err(ASSET_SERVER_MANUAL_REMOVE)),
            PyResourceType::Dynamic(type_ptr) => {
                let bridge = global_registry::get_resource_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| PyTypeError::new_err(RESOURCE_BRIDGE_NOT_FOUND))?;
                bridge.remove(world);
                Ok(())
            }
            PyResourceType::Custom(type_ptr) => {
                let Some(registry) = world.get_resource::<ResourceRegistry>() else {
                    return Ok(());
                };
                let Some(component_id) = registry.get(*type_ptr as usize) else {
                    return Ok(());
                };

                world.remove_resource_by_id(component_id);

                Ok(())
            }
        }
    }

    /// Get the ComponentId for this resource type from the world
    /// Returns None if the resource hasn't been registered/inserted yet
    pub fn get_component_id(&self, world: &World) -> Option<ComponentId> {
        match self {
            PyResourceType::AssetServer => world
                .components()
                .component_id::<bevy::asset::AssetServer>(),
            PyResourceType::Dynamic(type_ptr) => {
                global_registry::get_resource_bridge_by_py_type(*type_ptr)
                    .and_then(|bridge| bridge.resource_id(world))
            }
            PyResourceType::Custom(type_ptr) => world
                .get_resource::<ResourceRegistry>()
                .and_then(|registry| registry.get(*type_ptr as usize)),
        }
    }

    /// Register (get-or-create) the ComponentId for this resource type.
    ///
    /// `get_component_id` is lookup-only and returns None until the resource is
    /// inserted; that silently drops access for resources inserted after schedule
    /// init (e.g. via startup Commands), leaving conflicting systems to race.
    /// This variant creates the id up front so `DynamicSystem::initialize` always
    /// declares it. `initialize` holds `&mut World`, so registration is sound, and
    /// the created id is TypeId-keyed to the one a later insertion resolves to.
    ///
    /// Returns None only when a Dynamic resource has no registered bridge, matching
    /// `get_component_id`'s behavior for that unreachable-at-runtime case.
    pub fn register_component_id(&self, world: &mut World, py: Python) -> Option<ComponentId> {
        match self {
            PyResourceType::AssetServer => {
                Some(world.register_component::<bevy::asset::AssetServer>())
            }
            PyResourceType::Dynamic(type_ptr) => {
                global_registry::get_resource_bridge_by_py_type(*type_ptr)
                    .map(|bridge| bridge.register_resource_id(world))
            }
            PyResourceType::Custom(type_ptr) => {
                // The custom-resource ComponentId comes from a descriptor built
                // from the Python type name alone (no value needed), so it can be
                // created here. register_custom_resource is get-or-reuse and stores
                // the id in ResourceRegistry, so a later insert_resource reuses it.
                Some(register_custom_resource(world, *type_ptr, py))
            }
        }
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

/// Helper function to register a custom Python resource with Bevy's ECS.
///
/// This creates a ComponentDescriptor with the layout of a `Py<PyAny>` and registers
/// its identity with the world. Concrete custom resource values use this descriptor
/// as their dynamic component on Bevy's stable resource entity.
///
/// The neutral resource registry is App-local and uses integer type keys, keeping
/// interpreter objects out of shared bookkeeping.
///
/// During hot reload, Python re-executes resource classes creating new PyTypeObject pointers.
/// This function detects that case via a name-based lookup and adds the new pointer as an
/// alias for the existing ComponentId, preserving access to previously inserted resource data.
///
/// # Arguments
/// * `world` - The Bevy world to register the resource in
/// * `type_ptr` - The PyTypeObject pointer identifying the Python resource class
/// * `py` - The active interpreter token used to read class metadata
///
/// # Returns
/// The ComponentId of the registered resource
pub(crate) fn register_custom_resource(
    world: &mut World,
    type_ptr: *const PyTypeObject,
    py: Python,
) -> ComponentId {
    let name = get_python_type_name(py, type_ptr);
    let qualified_name = get_python_qualified_name(py, type_ptr);
    let generation = world
        .get_resource::<pybevy_reload::HotReloadGeneration>()
        .map_or(0, |generation| generation.current);
    let outcome = register_custom_resource_guarded::<Pyo3ResourceObjectDescriptor>(
        world,
        type_ptr as usize,
        &name,
        qualified_name.as_deref(),
        generation,
    );

    // Synchronize the PyO3/MCP class table only after the neutral registry's
    // World-resource borrow has ended.
    match outcome {
        ResourceRegisterOutcome::Reused(_) => {}
        ResourceRegisterOutcome::Aliased(id) => {
            if let Some(mut custom_info) =
                world.get_resource_mut::<pybevy_core::CustomResourceInfo>()
            {
                custom_info.update_type_ptr(id, type_ptr, retained_type_object(py, type_ptr));
            }

            if let Some(qualified_name) = qualified_name {
                bevy::log::debug!(
                    "Hot reload: aliased resource '{}' (new ptr {:p}) to existing ComponentId {:?}",
                    qualified_name,
                    type_ptr,
                    id,
                );
            }
        }
        ResourceRegisterOutcome::Registered(id) => {
            if !world.contains_resource::<pybevy_core::CustomResourceInfo>() {
                world.insert_resource(pybevy_core::CustomResourceInfo::default());
            }
            world
                .resource_mut::<pybevy_core::CustomResourceInfo>()
                .insert(
                    id,
                    pybevy_core::CustomResourceEntry {
                        type_ptr,
                        type_object: retained_type_object(py, type_ptr),
                        name,
                    },
                );
        }
    }

    outcome.id()
}

/// Take an owned reference to the class behind `type_ptr` so control handlers
/// never call through a pointer whose object was freed by a reload.
fn retained_type_object(py: Python, type_ptr: *const PyTypeObject) -> Option<Py<PyAny>> {
    if type_ptr.is_null() {
        return None;
    }
    // SAFETY: callers pass a live class pointer obtained from a bound type
    // object earlier in the same attached scope.
    let bound =
        unsafe { Bound::from_borrowed_ptr_or_opt(py, type_ptr as *mut pyo3::ffi::PyObject) };
    bound.map(Bound::unbind)
}

impl fmt::Display for PyResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PyResourceType::Custom(type_ptr) => write!(f, "Custom({:p})", type_ptr),
            _ => write!(f, "{:?}", self),
        }
    }
}

impl TryFrom<(&Bound<'_, PyType>, Python<'_>)> for PyResourceType {
    type Error = PyErr;

    fn try_from((ty, py): (&Bound<'_, PyType>, Python)) -> Result<Self, Self::Error> {
        reject_state_type_as_resource(ty)?;

        // Check if type extends Resource by checking the MRO for a class named "Resource"
        // This handles the case where PyResource is registered from different crates
        // (pybevy_core vs main crate) which creates separate Python type objects
        let mro = ty.mro();
        let mut is_resource = false;

        for base in mro.iter() {
            // MRO elements are PyType objects - use cast instead of deprecated downcast
            if let Ok(base_type) = base.cast::<PyType>()
                && let Ok(name) = base_type.name()
                && name == "Resource"
            {
                is_resource = true;
                break;
            }
        }

        if !is_resource {
            let class_name = ty
                .name()
                .map(|n| n.to_string())
                .unwrap_or_else(|_| "Unknown".to_string());
            return Err(PyErr::new::<PyTypeError, _>(expected_resource_subclass(
                class_name,
            )));
        }

        // Check for known resource types
        if ty.is(PyAssetServer::type_object(py)) {
            Ok(PyResourceType::AssetServer)
        } else {
            // Check for dynamically registered resource bridges (from feature crates)
            let type_ptr = ty.as_type_ptr();
            if global_registry::contains_resource_py_type(type_ptr) {
                return Ok(PyResourceType::Dynamic(type_ptr));
            }

            // Check if this is a built-in resource that doesn't need decorator validation
            let is_builtin = ty.is(PyState::type_object(py))
                || ty.is(PyNextState::type_object(py))
                || ty.is(PyHotReloadControl::type_object(py))
                || ty.is(PyMessages::type_object(py))
                || ty.is(PyAssets::type_object(py));

            if !is_builtin {
                // Not a built-in resource - check for custom resource decorator
                let has_decorator = ty
                    .getattr("__pybevy_resource_decorated__")
                    .ok()
                    .and_then(|marker| marker.is_truthy().ok())
                    .unwrap_or(false);

                if !has_decorator {
                    return Err(PyErr::new::<PyTypeError, _>(resource_decorator_required(
                        ty.name()?,
                    )));
                }
            }

            // Any other subclass of PyResource is a custom resource
            Ok(PyResourceType::Custom(ty.as_type_ptr()))
        }
    }
}
