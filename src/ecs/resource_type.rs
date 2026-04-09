use core::fmt;
use std::collections::HashMap;

use bevy::{ecs::component::ComponentId, prelude::*};
use pybevy_core::registry::global_registry;
use pyo3::{PyTypeInfo, exceptions::PyTypeError, ffi::PyTypeObject, prelude::*, types::PyType};

use crate::{
    app::hot_reload::bindings::PyHotReloadControl,
    assets::{asset_server::PyAssetServer, assets::PyAssets},
    ecs::{
        component_type::create_python_object_descriptor,
        helpers::{
            type_utils::get_python_type_name,
            validity_guard::{AccessMode, ValidityFlag},
        },
        messages::PyMessages,
        resource::PyResource,
        state::{PyNextState, PyState},
    },
};

/// Resource registry stored as a Bevy resource
/// Maps Python type pointers to their ComponentIds
#[derive(Default, Resource)]
pub struct ResourceRegistry {
    pub(crate) registry: HashMap<*const PyTypeObject, ComponentId>,
    /// Maps qualified names (`module.qualname`) to ComponentIds for alias-based
    /// lookup during hot reload, matching the pattern in ComponentRegistry.
    pub(crate) by_name: HashMap<String, ComponentId>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for ResourceRegistry {}
unsafe impl Sync for ResourceRegistry {}

/// Re-export from pybevy_core for cross-crate access
pub use pybevy_core::PyResourceStorage;

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

impl PyResourceType {
    /// Get AssetServer resource from world (read-only)
    fn get_asset_server(world: &World, py: Python, validity: ValidityFlag) -> PyResult<Py<PyAny>> {
        let world_ptr = world as *const World as *mut World;
        let py_asset_server = unsafe { PyAssetServer::new(world_ptr, validity) };
        let asset_server_obj = Py::new(py, (py_asset_server, PyResource))?;
        Ok(asset_server_obj.into_any())
    }

    /// Get AssetServer resource from world (mutable)
    fn get_asset_server_mut(
        world: &mut World,
        py: Python,
        validity: ValidityFlag,
    ) -> PyResult<Py<PyAny>> {
        let world_ptr = world as *mut World;
        let py_asset_server = unsafe { PyAssetServer::new(world_ptr, validity) };
        let asset_server_obj = Py::new(py, (py_asset_server, PyResource))?;
        Ok(asset_server_obj.into_any())
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
                    .ok_or_else(|| {
                        PyTypeError::new_err("Resource bridge not found for dynamic type")
                    })?;
                bridge.get(world, validity.with_access_mode(AccessMode::Read), py)
            }
            PyResourceType::Custom(type_ptr) => {
                // For custom resources, we need to look up the ComponentId from the registry
                let registry = world
                    .get_resource::<ResourceRegistry>()
                    .ok_or_else(|| {
                        // Get the type name for a better error message
                        let type_name = get_python_type_name(py, *type_ptr);
                        PyTypeError::new_err(format!(
                            "Resource type `{}` is not found. Did you call `init_resource` or `insert_resource`?",
                            type_name
                        ))
                    })?;

                let component_id = registry
                    .registry
                    .get(type_ptr)
                    .ok_or_else(|| {
                        // Get the type name for a better error message
                        let type_name = get_python_type_name(py, *type_ptr);
                        PyTypeError::new_err(format!(
                            "Resource type `{}` is not found. Did you call `init_resource` or `insert_resource`?",
                            type_name
                        ))
                    })?;

                // Get the resource from the storage
                let storage = world.get_resource::<PyResourceStorage>().ok_or_else(|| {
                    // Get the type name for a better error message
                    let type_name = get_python_type_name(py, *type_ptr);
                    PyTypeError::new_err(format!(
                        "Resource type `{}` not present in the world",
                        type_name
                    ))
                })?;

                let resource = storage.resources.get(component_id).ok_or_else(|| {
                    // Get the type name for a better error message
                    let type_name = get_python_type_name(py, *type_ptr);
                    PyTypeError::new_err(format!(
                        "Resource type `{}` not present in the world",
                        type_name
                    ))
                })?;

                Ok(resource.clone_ref(py))
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
                    .ok_or_else(|| {
                        PyTypeError::new_err("Resource bridge not found for dynamic type")
                    })?;
                bridge.get_mut(world, validity.with_access_mode(AccessMode::Write), py)
            }
            PyResourceType::Custom(_type_ptr) => {
                // For custom resources, mutable access returns the same Python object
                // (Python objects are inherently mutable)
                // SAFETY: We cast to &World temporarily - this is safe because we're not modifying
                // the world structure itself, only getting a reference to the Python object
                let world_ref = world as &World;
                self.get_from_world(world_ref, py, validity)
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
            PyResourceType::AssetServer => Err(PyTypeError::new_err(
                "AssetServer cannot be manually inserted. It is provided by AssetPlugin.",
            )),
            PyResourceType::Dynamic(type_ptr) => {
                let bridge = global_registry::get_resource_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| {
                        PyTypeError::new_err("Resource bridge not found for dynamic type")
                    })?;
                bridge.insert(world, resource_instance.bind(py))
            }
            PyResourceType::Custom(type_ptr) => {
                // Register the custom resource if not already registered
                let component_id = {
                    // Ensure the registry exists and check if already registered
                    if !world.contains_resource::<ResourceRegistry>() {
                        world.insert_resource(ResourceRegistry::default());
                    }

                    let existing_id = {
                        let registry = world.resource::<ResourceRegistry>();
                        registry.registry.get(type_ptr).copied()
                    }; // registry is dropped here

                    if let Some(id) = existing_id {
                        id
                    } else {
                        // Get the resource name from the Python type
                        let name = {
                            let type_obj = unsafe {
                                Bound::from_borrowed_ptr(py, *type_ptr as *mut pyo3::ffi::PyObject)
                            };
                            let type_bound = type_obj.cast::<PyType>()?;
                            type_bound.name()?.to_string()
                        };

                        // Register the custom resource
                        register_custom_resource(world, *type_ptr, name)
                    }
                };

                // Ensure PyResourceStorage exists
                if !world.contains_resource::<PyResourceStorage>() {
                    world.insert_resource(PyResourceStorage::default());
                }

                // Insert the Python object into the storage
                world
                    .resource_mut::<PyResourceStorage>()
                    .resources
                    .insert(component_id, resource_instance);

                Ok(())
            }
        }
    }

    /// Remove a Python resource from the world
    pub fn remove_from_world(&self, world: &mut World, py: Python) -> PyResult<()> {
        match self {
            PyResourceType::AssetServer => Err(PyTypeError::new_err(
                "AssetServer cannot be manually removed. It is managed by AssetPlugin.",
            )),
            PyResourceType::Dynamic(type_ptr) => {
                let bridge = global_registry::get_resource_bridge_by_py_type(*type_ptr)
                    .ok_or_else(|| {
                        PyTypeError::new_err("Resource bridge not found for dynamic type")
                    })?;
                bridge.remove(world);
                Ok(())
            }
            PyResourceType::Custom(type_ptr) => {
                // Look up the ComponentId from the registry
                let component_id = {
                    let registry = world.get_resource::<ResourceRegistry>().ok_or_else(|| {
                        let type_name = unsafe {
                            let type_obj =
                                Bound::from_borrowed_ptr(py, *type_ptr as *mut pyo3::ffi::PyObject);
                            type_obj
                                .cast::<PyType>()
                                .ok()
                                .and_then(|t| t.name().ok())
                                .map(|n| n.to_string())
                                .unwrap_or_else(|| "Unknown".to_string())
                        };
                        PyTypeError::new_err(format!(
                            "Resource type `{}` is not registered",
                            type_name
                        ))
                    })?;

                    registry.registry.get(type_ptr).copied().ok_or_else(|| {
                        let type_name = unsafe {
                            let type_obj =
                                Bound::from_borrowed_ptr(py, *type_ptr as *mut pyo3::ffi::PyObject);
                            type_obj
                                .cast::<PyType>()
                                .ok()
                                .and_then(|t| t.name().ok())
                                .map(|n| n.to_string())
                                .unwrap_or_else(|| "Unknown".to_string())
                        };
                        PyTypeError::new_err(format!(
                            "Resource type `{}` is not registered",
                            type_name
                        ))
                    })?
                };

                // Remove the Python object from storage
                if let Some(mut storage) = world.get_resource_mut::<PyResourceStorage>() {
                    storage.resources.remove(&component_id);
                }

                Ok(())
            }
        }
    }

    /// Get the ComponentId for this resource type from the world
    /// Returns None if the resource hasn't been registered/inserted yet
    pub fn get_component_id(&self, world: &World) -> Option<ComponentId> {
        match self {
            PyResourceType::AssetServer => {
                world.components().resource_id::<bevy::asset::AssetServer>()
            }
            PyResourceType::Dynamic(type_ptr) => {
                global_registry::get_resource_bridge_by_py_type(*type_ptr)
                    .and_then(|bridge| bridge.resource_id(world))
            }
            PyResourceType::Custom(type_ptr) => world
                .get_resource::<ResourceRegistry>()
                .and_then(|registry| registry.registry.get(type_ptr).copied()),
        }
    }
}

/// Extract the fully qualified Python name (`module.qualname`) for a type pointer.
///
/// This matches the format used by `pybevy/decorators.py`:
///   `f"{cls.__module__}.{cls.__qualname__}"`
fn get_python_qualified_name(py: Python, type_ptr: *const PyTypeObject) -> Option<String> {
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
/// it with the world as a resource. Custom resources are stored as Python objects in the ECS.
///
/// The resource registry is stored as a resource in the World to ensure proper scoping
/// per-app instance. The registry uses PyTypeObject pointers as keys for type identity.
///
/// During hot reload, Python re-executes resource classes creating new PyTypeObject pointers.
/// This function detects that case via a name-based lookup and adds the new pointer as an
/// alias for the existing ComponentId, preserving access to previously inserted resource data.
///
/// # Arguments
/// * `world` - The Bevy world to register the resource in
/// * `type_ptr` - The PyTypeObject pointer identifying the Python resource class
/// * `name` - The name of the resource (from the Python class) for the descriptor
///
/// # Returns
/// The ComponentId of the registered resource
pub(crate) fn register_custom_resource(
    world: &mut World,
    type_ptr: *const PyTypeObject,
    name: String,
) -> ComponentId {
    // Ensure the registry resource exists
    if !world.contains_resource::<ResourceRegistry>() {
        world.insert_resource(ResourceRegistry::default());
    }

    // Fast path: already registered by this exact pointer
    let existing_id = world
        .resource::<ResourceRegistry>()
        .registry
        .get(&type_ptr)
        .copied();

    if let Some(component_id) = existing_id {
        return component_id;
    }

    // Hot-reload path: check if a previous generation registered the same class by name.
    let qualified_name = Python::attach(|py| get_python_qualified_name(py, type_ptr));

    if let Some(ref qname) = qualified_name {
        let existing_id = world
            .resource::<ResourceRegistry>()
            .by_name
            .get(qname)
            .copied();

        if let Some(existing_id) = existing_id {
            // Add pointer alias for the existing ComponentId
            world
                .resource_mut::<ResourceRegistry>()
                .registry
                .insert(type_ptr, existing_id);

            // Update CustomResourceInfo type pointer for MCP access
            if let Some(mut custom_info) =
                world.get_resource_mut::<pybevy_core::CustomResourceInfo>()
            {
                custom_info.update_type_ptr(existing_id, type_ptr);
            }

            bevy::log::debug!(
                "Hot reload: aliased resource '{}' (new ptr {:p}) to existing ComponentId {:?}",
                qname,
                type_ptr,
                existing_id,
            );

            return existing_id;
        }
    }

    // Resource not in registry, register it with Bevy
    let descriptor = create_python_object_descriptor(name);
    let component_id = world.register_component_with_descriptor(descriptor);

    // Store in registry resource (both by pointer and by name)
    {
        let mut registry = world.resource_mut::<ResourceRegistry>();
        registry.registry.insert(type_ptr, component_id);
        if let Some(qname) = qualified_name {
            registry.by_name.insert(qname, component_id);
        }
    }

    // Store in cross-crate custom resource info (readable by MCP)
    let resource_name = Python::attach(|py| {
        let py_type =
            unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };
        py_type
            .cast::<pyo3::types::PyType>()
            .ok()
            .and_then(|cls| cls.name().ok().map(|n| n.to_string()))
            .unwrap_or_else(|| format!("CustomResource_{:?}", component_id))
    });

    if !world.contains_resource::<pybevy_core::CustomResourceInfo>() {
        world.insert_resource(pybevy_core::CustomResourceInfo::default());
    }
    world
        .resource_mut::<pybevy_core::CustomResourceInfo>()
        .insert(
            component_id,
            pybevy_core::CustomResourceEntry {
                type_ptr,
                name: resource_name,
            },
        );

    component_id
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
            return Err(PyErr::new::<PyTypeError, _>(format!(
                "Expected a subclass of `Resource`, but got `{}` which is not a subclass of `Resource`",
                class_name
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
                    return Err(PyErr::new::<PyTypeError, _>(format!(
                        "Resource class '{}' must be decorated with @resource decorator",
                        ty.name()?
                    )));
                }
            }

            // Any other subclass of PyResource is a custom resource
            Ok(PyResourceType::Custom(ty.as_type_ptr()))
        }
    }
}
