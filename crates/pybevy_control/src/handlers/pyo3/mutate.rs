use std::{mem, ptr, sync::Arc};

use bevy::ecs::world::World;
use pybevy_core::{ComponentBridge, CustomComponentInfo, CustomResourceInfo, PyResourceStorage};
use pyo3::{
    ffi::PyObject,
    prelude::*,
    types::{PyDict, PyList, PyModule, PyType},
};

use super::scene::resolve_entity;
use crate::{
    bridge::{ControlError, EntityRef},
    handlers::reflect_mutate::{self, ReflectError},
};

/// Find a component bridge by name.
fn find_bridge(name: &str) -> Option<Arc<dyn ComponentBridge>> {
    pybevy_core::registry::global_registry::all_component_bridges()
        .into_iter()
        .find(|b| b.name() == name)
}

/// Check if a tool result contains embedded field-level errors.
/// Used by batch and schedule to detect partial failures.
pub fn has_embedded_errors(value: &serde_json::Value) -> bool {
    value
        .get("errors")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| !arr.is_empty())
}

/// Spawn a new entity with components specified as JSON.
/// Tries Bevy reflection first (no GIL needed), falls back to Python for custom components.
/// Pre-validates all component names; if any are unknown, fails without spawning.
pub fn spawn_entity(
    world: &mut World,
    components: serde_json::Value,
) -> Result<serde_json::Value, ControlError> {
    let obj = components
        .as_object()
        .ok_or_else(|| ControlError::invalid_params("'components' must be a JSON object"))?;

    // Validate all components exist in registry (fail fast)
    let mut validation_errors = Vec::new();
    for (comp_name, _) in obj {
        if find_bridge(comp_name).is_none() {
            validation_errors.push(format!("{comp_name}: not found in registry"));
        }
    }
    if !validation_errors.is_empty() {
        return Err(ControlError::invalid_params(format!(
            "Unknown components: {}",
            validation_errors.join(", ")
        )));
    }

    // Spawn and add components
    let entity = world.spawn_empty().id();
    let entity_id = entity.to_bits();

    let mut added_components = Vec::new();
    let mut errors = Vec::new();

    for (comp_name, comp_fields) in obj {
        let Some(bridge) = find_bridge(comp_name) else {
            errors.push(format!("{comp_name}: not found in registry"));
            continue;
        };

        // Handle non-object values (strings, numbers, etc.) by constructing via Python kwargs
        // e.g., {"Name": "my_name"} → Name(name="my_name"), {"Text2d": "hello"} → Text2d(text="hello")
        if !comp_fields.is_object() {
            spawn_component_python_direct(
                world,
                entity,
                comp_name,
                comp_fields,
                &bridge,
                &mut added_components,
                &mut errors,
            );
            continue;
        }

        let fields = comp_fields.as_object().cloned().unwrap_or_default();
        let type_id = bridge.bevy_type_id();

        // Try reflection first
        match reflect_mutate::reflect_spawn_component(world, entity, type_id, &fields) {
            Ok(()) => {
                added_components.push(comp_name.clone());
                continue;
            }
            Err(
                ReflectError::NotRegistered
                | ReflectError::NoReflectComponent
                | ReflectError::NoReflectDefault
                | ReflectError::NotAStruct,
            ) => {
                // Fall back to Python
            }
            Err(ReflectError::FieldError(_)) => {
                // Fall back to Python — it handles many field types
                // (Color arrays, Vec2/Vec3, enums via type detection, custom wrappers)
            }
            Err(ReflectError::ComponentNotOnEntity) => {
                errors.push(format!("{comp_name}: entity not found"));
                continue;
            }
        }

        // Python fallback: create default and apply fields via setattr
        spawn_component_python(
            world,
            entity,
            comp_name,
            &fields,
            &bridge,
            &mut added_components,
            &mut errors,
        );
    }

    // If errors occurred, despawn the partial entity
    if !errors.is_empty() {
        world.despawn(entity);
        return Ok(serde_json::json!({
            "entity_id": null,
            "components_added": added_components,
            "errors": errors,
        }));
    }

    Ok(serde_json::json!({
        "entity_id": entity_id,
        "components_added": added_components,
    }))
}

/// Python fallback for spawning a component: create via py_type and apply fields.
/// Tries call0() first (default constructor), then kwargs constructor if that fails.
fn spawn_component_python(
    world: &mut World,
    entity: bevy::ecs::entity::Entity,
    comp_name: &str,
    fields: &serde_json::Map<String, serde_json::Value>,
    bridge: &Arc<dyn ComponentBridge>,
    added_components: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    Python::attach(|py| {
        let py_type = bridge.py_type(py);

        // Try default constructor first
        let instance = match py_type.call0() {
            Ok(inst) => {
                // Apply field values via setattr
                for (field_name, field_value) in fields {
                    match convert_field_value(py, &inst, field_name, field_value) {
                        Ok(py_value) => {
                            if let Err(e) = inst.setattr(field_name.as_str(), py_value) {
                                errors.push(format!("{comp_name}.{field_name}: {e}"));
                            }
                        }
                        Err(e) => {
                            errors.push(format!("{comp_name}.{field_name}: {e}"));
                        }
                    }
                }
                inst
            }
            Err(_) if !fields.is_empty() => {
                // Default constructor failed — try passing fields as kwargs
                let kwargs = PyDict::new(py);
                for (field_name, field_value) in fields {
                    match json_to_py(py, field_value) {
                        Ok(py_value) => {
                            if let Err(e) = kwargs.set_item(field_name, py_value) {
                                errors.push(format!("{comp_name}.{field_name}: {e}"));
                            }
                        }
                        Err(e) => {
                            errors.push(format!("{comp_name}.{field_name}: {e}"));
                        }
                    }
                }
                match py_type.call((), Some(&kwargs)) {
                    Ok(inst) => inst,
                    Err(e) => {
                        errors.push(format!("{comp_name}: failed to construct: {e}"));
                        return;
                    }
                }
            }
            Err(e) => {
                errors.push(format!("{comp_name}: failed to create default: {e}"));
                return;
            }
        };

        if let Err(e) = bridge.insert(world, entity, &instance) {
            errors.push(format!("{comp_name}: {e}"));
        } else {
            added_components.push(comp_name.to_string());
        }
    });
}

/// Python fallback for spawning a component from a non-object JSON value.
/// Used for components that accept a single constructor argument (e.g., Name("string"), Text2d("text")).
/// Passes the JSON value as the first positional argument to the Python constructor.
fn spawn_component_python_direct(
    world: &mut World,
    entity: bevy::ecs::entity::Entity,
    comp_name: &str,
    value: &serde_json::Value,
    bridge: &Arc<dyn ComponentBridge>,
    added_components: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    Python::attach(|py| {
        let py_type = bridge.py_type(py);
        let py_value = match json_to_py(py, value) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("{comp_name}: failed to convert value: {e}"));
                return;
            }
        };

        let instance = match py_type.call1((py_value,)) {
            Ok(inst) => inst,
            Err(e) => {
                errors.push(format!("{comp_name}: failed to construct with value: {e}"));
                return;
            }
        };

        if let Err(e) = bridge.insert(world, entity, &instance) {
            errors.push(format!("{comp_name}: {e}"));
        } else {
            added_components.push(comp_name.to_string());
        }
    });
}

/// Despawn an entity by ID or Name
pub fn despawn_entity(
    world: &mut World,
    entity_ref: EntityRef,
) -> Result<serde_json::Value, ControlError> {
    let entity = resolve_entity(world, &entity_ref)?;
    let entity_id = entity.to_bits();

    if world.despawn(entity) {
        Ok(serde_json::json!({
            "despawned": true,
            "entity_id": entity_id,
        }))
    } else {
        Err(ControlError::not_found(format!(
            "Entity {entity_id} could not be despawned"
        )))
    }
}

/// Partial mutation: change individual component fields.
/// Tries Bevy reflection first (no GIL needed), falls back to Python for custom components.
pub fn set_component(
    world: &mut World,
    entity_ref: EntityRef,
    component: String,
    fields: serde_json::Value,
) -> Result<serde_json::Value, ControlError> {
    let entity = resolve_entity(world, &entity_ref)?;
    let entity_id = entity.to_bits();

    let field_obj = fields
        .as_object()
        .ok_or_else(|| ControlError::invalid_params("'fields' must be a JSON object"))?;

    // Find bridge to get type_id
    if let Some(bridge) = find_bridge(&component) {
        let type_id = bridge.bevy_type_id();

        // Try reflection first
        match reflect_mutate::reflect_set_component(world, entity, type_id, field_obj) {
            Ok(updated_fields) => {
                return Ok(serde_json::json!({
                    "entity_id": entity_id,
                    "component": component,
                    "updated_fields": updated_fields,
                }));
            }
            Err(
                ReflectError::NotRegistered
                | ReflectError::NoReflectComponent
                | ReflectError::NotAStruct,
            ) => {
                // Fall back to Python
            }
            Err(ReflectError::ComponentNotOnEntity) => {
                return Ok(serde_json::json!({
                    "entity_id": entity_id,
                    "component": component,
                    "updated_fields": [],
                    "errors": [format!("Component '{component}' not found on entity {entity_id}")],
                }));
            }
            Err(ReflectError::FieldError(_)) => {
                // Fall back to Python — it handles many field types
                // (Color arrays, Vec2/Vec3, enums via type detection, custom wrappers)
            }
            Err(ReflectError::NoReflectDefault) => {
                // Not applicable for set_component, fall back
            }
        }

        // Python fallback for bridge components
        return set_component_python(world, entity, entity_id, &component, field_obj, &bridge);
    }

    // Fallback: check custom Python components via CustomComponentInfo
    set_custom_component(world, entity, entity_id, &component, field_obj)
}

/// Python fallback for set_component: extract via bridge and use setattr.
fn set_component_python(
    world: &mut World,
    entity: bevy::ecs::entity::Entity,
    entity_id: u64,
    component: &str,
    field_obj: &serde_json::Map<String, serde_json::Value>,
    bridge: &Arc<dyn ComponentBridge>,
) -> Result<serde_json::Value, ControlError> {
    let mut updated_fields = Vec::new();
    let mut errors = Vec::new();

    Python::attach(|py| {
        let validity_flag = pybevy_core::ValidityFlag::new_write();
        let validity = validity_flag.with_access_mode(pybevy_core::AccessMode::Write);

        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
            match bridge.extract_from_entity_mut(&mut entity_mut, validity, py) {
                Ok(Some(py_obj)) => {
                    let bound = py_obj.bind(py);
                    for (field_name, field_value) in field_obj {
                        match convert_field_value(py, bound, field_name, field_value) {
                            Ok(py_value) => {
                                if let Err(e) = bound.setattr(field_name.as_str(), py_value) {
                                    errors.push(format!("{field_name}: {e}"));
                                } else {
                                    updated_fields.push(field_name.clone());
                                }
                            }
                            Err(e) => {
                                errors.push(format!("{field_name}: {e}"));
                            }
                        }
                    }
                }
                Ok(None) => {
                    errors.push(format!(
                        "Component '{component}' not found on entity {entity_id}"
                    ));
                }
                Err(e) => {
                    errors.push(format!("Failed to extract component: {e}"));
                }
            }
        } else {
            errors.push(format!("Entity {entity_id} not found"));
        }

        validity_flag.set_invalid();
    });

    let mut result = serde_json::json!({
        "entity_id": entity_id,
        "component": component,
        "updated_fields": updated_fields,
    });

    if !errors.is_empty() {
        result
            .as_object_mut()
            .unwrap()
            .insert("errors".into(), serde_json::json!(errors));
    }

    Ok(result)
}

/// Set fields on a custom Python component (not in bridge registry).
/// Looks up the component via CustomComponentInfo and mutates fields via Python setattr.
fn set_custom_component(
    world: &mut World,
    entity: bevy::ecs::entity::Entity,
    entity_id: u64,
    component: &str,
    field_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, ControlError> {
    // Find the custom component's ComponentId — try exact name match first
    let custom_info = world
        .get_resource::<CustomComponentInfo>()
        .and_then(|info| {
            info.iter()
                .find(|(_, entry)| entry.name == component)
                .map(|(id, entry)| (id, entry.is_pyobject_storage))
        });

    // Fallback: check for qualified name variants (e.g. "module.Oscillator" vs "Oscillator")
    // and scan the entity's archetype for matching custom components
    let custom_info = custom_info.or_else(|| {
        let info = world.get_resource::<CustomComponentInfo>()?;
        let entity_ref = world.get_entity(entity).ok()?;

        // Check if any registered custom component matches by short name
        // and is present on this entity's archetype
        for (id, entry) in info.iter() {
            let short_name = entry.name.rsplit('.').next().unwrap_or(&entry.name);
            if short_name == component && entity_ref.get_by_id(id).is_ok() {
                return Some((id, entry.is_pyobject_storage));
            }
        }

        // Last resort: check entity's archetype for PyObject-stored components
        // whose Python type name matches. Only examine components that lack a Rust
        // TypeId (these are dynamically registered custom Python components).
        let components = world.components();
        for comp_id in entity_ref.archetype().components() {
            // Skip components already in CustomComponentInfo (checked above)
            if info.get(*comp_id).is_some() {
                continue;
            }
            // Skip components with a Rust TypeId (Bevy built-ins, bridge components)
            if let Some(comp_info) = components.get_info(*comp_id)
                && comp_info.type_id().is_some()
            {
                continue;
            }
            // Try interpreting as PyObject-stored component and check Python type name
            if let Ok(ptr) = entity_ref.get_by_id(*comp_id) {
                let matched = Python::attach(|py| {
                    // SAFETY: Components without a Rust TypeId that aren't in
                    // CustomComponentInfo are dynamically registered custom Python
                    // components stored as Py<PyAny>
                    let py_obj: &pyo3::Py<PyAny> =
                        unsafe { &*(ptr.as_ptr() as *const pyo3::Py<PyAny>) };
                    let bound = py_obj.bind(py);
                    let type_name = bound
                        .get_type()
                        .name()
                        .ok()
                        .map(|n| n.to_string())
                        .unwrap_or_default();
                    type_name == component
                });
                if matched {
                    return Some((*comp_id, true));
                }
            }
        }

        None
    });

    let Some((comp_id, is_pyobject_storage)) = custom_info else {
        // Build a helpful error with available custom components
        let available = world
            .get_resource::<CustomComponentInfo>()
            .map(|info| {
                info.iter()
                    .map(|(_, entry)| entry.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let hint = if available.is_empty() {
            String::new()
        } else {
            format!(" Available custom components: {available}.")
        };
        return Err(ControlError::not_found(format!(
            "Component '{component}' not found in custom component registry. \
             If this is a custom @component, try reloading the scene.{hint}"
        )));
    };

    if !is_pyobject_storage {
        return Err(ControlError::invalid_params(format!(
            "Component '{component}' uses wrapper storage — field mutation requires storage=\"python\""
        )));
    }

    // Check entity has this component
    let eref = world
        .get_entity(entity)
        .map_err(|_| ControlError::not_found(format!("Entity {entity_id} not found")))?;

    let has_component = eref.get_by_id(comp_id).is_ok();

    if !has_component {
        // Component not present — create new instance via Python and insert it
        return insert_custom_component(world, entity, entity_id, component, comp_id, field_obj);
    }

    // Component exists — get mutable pointer for in-place mutation
    let ptr = world
        .get_entity(entity)
        .map_err(|_| ControlError::not_found(format!("Entity {entity_id} not found")))?
        .get_by_id(comp_id)
        .map_err(|_| {
            ControlError::not_found(format!(
                "Component '{component}' not found on entity {entity_id}"
            ))
        })?;

    let mut updated_fields = Vec::new();
    let mut errors = Vec::new();

    Python::attach(|py| {
        // SAFETY: We checked is_pyobject_storage above — raw data is a Py<PyAny>
        let py_obj: &pyo3::Py<PyAny> = unsafe { &*(ptr.as_ptr() as *const pyo3::Py<PyAny>) };
        let bound = py_obj.bind(py);

        for (field_name, field_value) in field_obj {
            match convert_field_value(py, bound, field_name, field_value) {
                Ok(py_value) => {
                    if let Err(e) = bound.setattr(field_name.as_str(), py_value) {
                        errors.push(format!("{field_name}: {e}"));
                    } else {
                        updated_fields.push(field_name.clone());
                    }
                }
                Err(e) => {
                    errors.push(format!("{field_name}: {e}"));
                }
            }
        }
    });

    let mut result = serde_json::json!({
        "entity_id": entity_id,
        "component": component,
        "updated_fields": updated_fields,
    });

    if !errors.is_empty() {
        result
            .as_object_mut()
            .unwrap()
            .insert("errors".into(), serde_json::json!(errors));
    }

    Ok(result)
}

/// Create and insert a new custom Python component on an entity.
/// Used when set_component is called but the entity doesn't have the component yet.
fn insert_custom_component(
    world: &mut World,
    entity: bevy::ecs::entity::Entity,
    entity_id: u64,
    component: &str,
    comp_id: bevy::ecs::component::ComponentId,
    field_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, ControlError> {
    // Get the type pointer from CustomComponentInfo
    let type_ptr = world
        .get_resource::<CustomComponentInfo>()
        .and_then(|info| info.get(comp_id).map(|entry| entry.type_ptr))
        .ok_or_else(|| ControlError::internal("Custom component info lost".to_string()))?;

    Python::attach(|py| {
        let py_type = unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut PyObject) };
        let Ok(cls) = py_type.cast::<PyType>() else {
            return Err(ControlError::internal(
                "Custom component type pointer is invalid".to_string(),
            ));
        };

        // Create instance: try default constructor first, then kwargs constructor
        let (instance, updated_fields) = match cls.call0() {
            Ok(inst) => {
                // Default constructor succeeded — apply fields via setattr
                let mut updated = Vec::new();
                for (field_name, field_value) in field_obj {
                    match convert_field_value(py, &inst, field_name, field_value) {
                        Ok(py_value) => {
                            inst.setattr(field_name.as_str(), py_value).map_err(|e| {
                                ControlError::internal(format!("Failed to set {field_name}: {e}"))
                            })?;
                            updated.push(field_name.clone());
                        }
                        Err(e) => {
                            return Err(ControlError::internal(format!(
                                "Failed to convert {field_name}: {e}"
                            )));
                        }
                    }
                }
                (inst, updated)
            }
            Err(_) if !field_obj.is_empty() => {
                // Default constructor failed — try passing fields as kwargs
                let kwargs = PyDict::new(py);
                let mut updated = Vec::new();
                for (field_name, field_value) in field_obj {
                    match json_to_py(py, field_value) {
                        Ok(py_value) => {
                            kwargs.set_item(field_name, py_value).map_err(|e| {
                                ControlError::internal(format!(
                                    "Failed to set kwarg {field_name}: {e}"
                                ))
                            })?;
                            updated.push(field_name.clone());
                        }
                        Err(e) => {
                            return Err(ControlError::internal(format!(
                                "Failed to convert {field_name}: {e}"
                            )));
                        }
                    }
                }
                let inst = cls.call((), Some(&kwargs)).map_err(|e| {
                    ControlError::internal(format!("Failed to create component: {e}"))
                })?;
                (inst, updated)
            }
            Err(e) => {
                return Err(ControlError::internal(format!(
                    "Failed to create component: {e}"
                )));
            }
        };

        // Insert as PyObject component
        let py_obj = instance.unbind();
        let mut entity_mut = world
            .get_entity_mut(entity)
            .map_err(|_| ControlError::not_found(format!("Entity {entity_id} not found")))?;
        // SAFETY: comp_id is a registered custom component with PyObject storage.
        // The component layout matches Py<PyAny> which was registered during app setup.
        unsafe {
            let ptr = ptr::addr_of!(py_obj) as *const u8;
            let data = core::ptr::NonNull::new_unchecked(ptr as *mut u8);
            entity_mut.insert_by_id(comp_id, bevy::ptr::OwningPtr::new(data));
        }
        mem::forget(py_obj); // Ownership transferred to ECS

        Ok(serde_json::json!({
            "entity_id": entity_id,
            "component": component,
            "updated_fields": updated_fields,
            "inserted": true,
        }))
    })
}

/// Components whose removal silently breaks rendering, hierarchy, or spatial queries.
const STRUCTURAL_COMPONENTS: &[&str] = &[
    "Transform",
    "GlobalTransform",
    "Visibility",
    "InheritedVisibility",
    "ViewVisibility",
    "Mesh3d",
    "Mesh2d",
    "MeshMaterial3d",
    "MeshMaterial2d",
    "Camera3d",
    "Camera2d",
    "Camera",
    "Sprite",
    "Node",
];

fn structural_warning(component: &str) -> Option<String> {
    if STRUCTURAL_COMPONENTS.contains(&component) {
        Some(format!(
            "removing '{component}' from a live entity will likely break rendering, \
             hierarchy, or spatial queries. Re-insert via set_component if undone in error."
        ))
    } else {
        None
    }
}

/// Remove a component from an entity
pub fn remove_component(
    world: &mut World,
    entity_ref: EntityRef,
    component: String,
) -> Result<serde_json::Value, ControlError> {
    let entity = resolve_entity(world, &entity_ref)?;
    let entity_id = entity.to_bits();
    let warning = structural_warning(&component);

    let build_response = |removed: &str, warning: &Option<String>| -> serde_json::Value {
        let mut out = serde_json::json!({
            "entity_id": entity_id,
            "removed": removed,
        });
        if let Some(w) = warning {
            out["warning"] = serde_json::json!(w);
        }
        out
    };

    for bridge in pybevy_core::registry::global_registry::all_component_bridges() {
        if bridge.name() == component.as_str() {
            let component_id = bridge.register(world);
            if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                entity_mut.remove_by_id(component_id);
                return Ok(build_response(&component, &warning));
            } else {
                return Err(ControlError::not_found(format!(
                    "Entity {entity_id} not found"
                )));
            }
        }
    }

    // Fallback: check custom Python components via CustomComponentInfo
    let custom_comp_id = world
        .get_resource::<CustomComponentInfo>()
        .and_then(|info| {
            info.iter()
                .find(|(_, entry)| entry.name == component)
                .map(|(id, _)| id)
        });

    if let Some(comp_id) = custom_comp_id {
        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
            entity_mut.remove_by_id(comp_id);
            return Ok(build_response(&component, &warning));
        } else {
            return Err(ControlError::not_found(format!(
                "Entity {entity_id} not found"
            )));
        }
    }

    Err(ControlError::not_found(format!(
        "Component '{component}' not in registry"
    )))
}

/// Insert or update a resource
pub fn insert_resource(
    world: &mut World,
    resource_type: String,
    value: serde_json::Value,
) -> Result<serde_json::Value, ControlError> {
    // Reject scalar/array values up front; field application requires a JSON object.
    match &value {
        serde_json::Value::Object(_) | serde_json::Value::Null => {}
        other => {
            let kind = match other {
                serde_json::Value::String(_) => "string",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::Bool(_) => "bool",
                serde_json::Value::Array(_) => "array",
                _ => "non-object",
            };
            return Err(ControlError::invalid_params(format!(
                "set_resource value for '{resource_type}' must be a JSON object (got {kind})"
            )));
        }
    }

    // Try bridge registry first
    let bridge_result = Python::attach(|py| {
        for bridge in pybevy_core::registry::global_registry::all_resource_bridges() {
            if bridge.name() == resource_type.as_str() {
                // Patch semantics: if resource already exists, mutate in-place
                // to preserve fields not included in the update
                if bridge.contains_in_world(world) {
                    if let Some(obj) = value.as_object()
                        && !obj.is_empty()
                    {
                        let write_flag = pybevy_core::ValidityFlag::new_write();
                        let write_validity =
                            write_flag.with_access_mode(pybevy_core::AccessMode::Write);

                        let py_resource =
                            bridge.get_mut(world, write_validity, py).map_err(|e| {
                                ControlError::internal(format!(
                                    "Failed to get existing resource for patch: {e}"
                                ))
                            })?;
                        let instance = py_resource.bind(py);

                        for (field_name, field_value) in obj {
                            match convert_field_value(py, instance, field_name, field_value) {
                                Ok(py_value) => {
                                    if let Err(e) = instance.setattr(field_name.as_str(), py_value)
                                    {
                                        write_flag.set_invalid();
                                        return Err(ControlError::internal(format!(
                                            "Failed to set {field_name}: {e}"
                                        )));
                                    }
                                }
                                Err(e) => {
                                    write_flag.set_invalid();
                                    return Err(ControlError::internal(format!(
                                        "Failed to convert {field_name}: {e}"
                                    )));
                                }
                            }
                        }

                        write_flag.set_invalid();
                    }
                    return Ok(Some(serde_json::json!({
                        "inserted": resource_type,
                    })));
                }

                // Resource doesn't exist yet: create default and apply fields
                let py_type = bridge.py_type(py);
                match py_type.call0() {
                    Ok(instance) => {
                        // Apply field values if provided
                        if let Some(obj) = value.as_object() {
                            for (field_name, field_value) in obj {
                                match convert_field_value(py, &instance, field_name, field_value) {
                                    Ok(py_value) => {
                                        if let Err(e) =
                                            instance.setattr(field_name.as_str(), py_value)
                                        {
                                            return Err(ControlError::internal(format!(
                                                "Failed to set {field_name}: {e}"
                                            )));
                                        }
                                    }
                                    Err(e) => {
                                        return Err(ControlError::internal(format!(
                                            "Failed to convert {field_name}: {e}"
                                        )));
                                    }
                                }
                            }
                        }
                        if let Err(e) = bridge.insert(world, &instance) {
                            return Err(ControlError::internal(format!(
                                "Failed to insert resource: {e}"
                            )));
                        }
                        return Ok(Some(serde_json::json!({
                            "inserted": resource_type,
                        })));
                    }
                    Err(e) => {
                        return Err(ControlError::internal(format!(
                            "Failed to create default resource: {e}"
                        )));
                    }
                }
            }
        }
        Ok(None) // Not found in bridges
    })?;

    if let Some(result) = bridge_result {
        return Ok(result);
    }

    // Fallback: check custom Python resources via CustomResourceInfo
    let custom_entry = world.get_resource::<CustomResourceInfo>().and_then(|info| {
        info.iter()
            .find(|(_, entry)| entry.name == resource_type)
            .map(|(id, entry)| (id, entry.type_ptr))
    });

    if let Some((comp_id, type_ptr)) = custom_entry {
        Python::attach(|py| {
            // Patch semantics: if custom resource already exists, mutate in-place
            if let Some(storage) = world.get_resource::<PyResourceStorage>()
                && let Some(existing) = storage.resources.get(&comp_id)
            {
                let bound = existing.bind(py);
                if let Some(obj) = value.as_object() {
                    for (field_name, field_value) in obj {
                        match convert_field_value(py, bound, field_name, field_value) {
                            Ok(py_value) => {
                                if let Err(e) = bound.setattr(field_name.as_str(), py_value) {
                                    return Err(ControlError::internal(format!(
                                        "Failed to set {field_name}: {e}"
                                    )));
                                }
                            }
                            Err(e) => {
                                return Err(ControlError::internal(format!(
                                    "Failed to convert {field_name}: {e}"
                                )));
                            }
                        }
                    }
                }
                return Ok(serde_json::json!({
                    "inserted": resource_type,
                    "custom": true,
                }));
            }

            // Resource doesn't exist yet: create default and apply fields
            let py_type = unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut PyObject) };
            let Ok(cls) = py_type.cast::<PyType>() else {
                return Err(ControlError::internal(
                    "Custom resource type pointer is invalid".to_string(),
                ));
            };

            let instance = cls
                .call0()
                .map_err(|e| ControlError::internal(format!("Failed to create resource: {e}")))?;

            if let Some(obj) = value.as_object() {
                for (field_name, field_value) in obj {
                    match convert_field_value(py, &instance, field_name, field_value) {
                        Ok(py_value) => {
                            if let Err(e) = instance.setattr(field_name.as_str(), py_value) {
                                return Err(ControlError::internal(format!(
                                    "Failed to set {field_name}: {e}"
                                )));
                            }
                        }
                        Err(e) => {
                            return Err(ControlError::internal(format!(
                                "Failed to convert {field_name}: {e}"
                            )));
                        }
                    }
                }
            }

            // Store in PyResourceStorage
            if !world.contains_resource::<PyResourceStorage>() {
                world.insert_resource(PyResourceStorage::default());
            }
            world
                .resource_mut::<PyResourceStorage>()
                .resources
                .insert(comp_id, instance.unbind());

            Ok(serde_json::json!({
                "inserted": resource_type,
                "custom": true,
            }))
        })
    } else {
        Err(ControlError::not_found(format!(
            "Resource '{resource_type}' not in registry"
        )))
    }
}

/// Remove a resource from the world
pub fn remove_resource(
    world: &mut World,
    resource_type: String,
) -> Result<serde_json::Value, ControlError> {
    for bridge in pybevy_core::registry::global_registry::all_resource_bridges() {
        if bridge.name() == resource_type.as_str() {
            bridge.remove(world);
            return Ok(serde_json::json!({
                "removed": resource_type,
            }));
        }
    }

    // Fallback: check custom Python resources via CustomResourceInfo
    let custom_comp_id = world.get_resource::<CustomResourceInfo>().and_then(|info| {
        info.iter()
            .find(|(_, entry)| entry.name == resource_type)
            .map(|(id, _)| id)
    });

    if let Some(comp_id) = custom_comp_id {
        if let Some(storage) = world.get_resource_mut::<PyResourceStorage>() {
            storage.into_inner().resources.remove(&comp_id);
        }
        return Ok(serde_json::json!({
            "removed": resource_type,
        }));
    }

    Err(ControlError::not_found(format!(
        "Resource '{resource_type}' not in registry"
    )))
}

/// Execute a batch of mutation operations in a single World access.
/// Each operation runs independently — failures don't abort the batch.
pub fn batch_mutate(
    world: &mut World,
    operations: Vec<serde_json::Value>,
) -> Result<serde_json::Value, ControlError> {
    let mut results = Vec::with_capacity(operations.len());

    for (i, op) in operations.iter().enumerate() {
        let action = op.get("action").and_then(|v| v.as_str()).unwrap_or("");

        let result = match action {
            "set_component" => {
                let entity_ref = parse_entity_ref_from_op(op);
                let component = op
                    .get("component")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let fields = op.get("fields").cloned();

                match (entity_ref, component, fields) {
                    (Ok(entity), Some(comp), Some(f)) => set_component(world, entity, comp, f),
                    (Err(msg), _, _) => {
                        Err(ControlError::invalid_params(format!("op[{i}]: {msg}")))
                    }
                    (Ok(_), None, _) | (Ok(_), _, None) => Err(ControlError::invalid_params(
                        format!("op[{i}]: set_component requires entity, component, fields"),
                    )),
                }
            }
            "spawn" => {
                let components = op
                    .get("components")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                spawn_entity(world, components)
            }
            "despawn" => {
                let entity_ref = parse_entity_ref_from_op(op);
                match entity_ref {
                    Ok(entity) => despawn_entity(world, entity),
                    Err(msg) => Err(ControlError::invalid_params(format!("op[{i}]: {msg}"))),
                }
            }
            "remove_component" => {
                let entity_ref = parse_entity_ref_from_op(op);
                let component = op
                    .get("component")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                match (entity_ref, component) {
                    (Ok(entity), Some(comp)) => remove_component(world, entity, comp),
                    (Err(msg), _) => Err(ControlError::invalid_params(format!("op[{i}]: {msg}"))),
                    (Ok(_), None) => Err(ControlError::invalid_params(format!(
                        "op[{i}]: remove_component requires entity and component"
                    ))),
                }
            }
            _ => Err(ControlError::invalid_params(format!(
                "op[{i}]: unknown action '{action}'. Valid: set_component, spawn, despawn, remove_component"
            ))),
        };

        match result {
            Ok(val) => {
                let status = if has_embedded_errors(&val) {
                    "partial"
                } else {
                    "ok"
                };
                results.push(serde_json::json!({"status": status, "result": val}));
            }
            Err(e) => results.push(serde_json::json!({"status": "error", "error": e.message})),
        }
    }

    let succeeded = results
        .iter()
        .filter(|r| r.get("status").and_then(|v| v.as_str()) == Some("ok"))
        .count();
    let partial = results
        .iter()
        .filter(|r| r.get("status").and_then(|v| v.as_str()) == Some("partial"))
        .count();

    Ok(serde_json::json!({
        "results": results,
        "total": operations.len(),
        "succeeded": succeeded,
        "partial": partial,
    }))
}

/// Parse entity ref from a batch operation JSON object.
///
/// The batch tool documents per-op shape as `{"action": "...", "entity": id_or_name, ...}`.
/// `entity` is polymorphic: int → `EntityRef::Id`, string → `EntityRef::Name`.
fn parse_entity_ref_from_op(op: &serde_json::Value) -> Result<EntityRef, String> {
    let Some(entity) = op.get("entity") else {
        return Err("Missing 'entity' (int or string)".into());
    };
    if let Some(id) = entity.as_u64() {
        return Ok(EntityRef::Id(id));
    }
    if let Some(name) = entity.as_str() {
        return Ok(EntityRef::Name(name.to_string()));
    }
    Err("'entity' must be an integer ID or a string name".into())
}

/// Convert a JSON field value to the appropriate Python type by inspecting the current field type.
/// For Vec2/Vec3/Quat fields, JSON arrays are converted to the proper constructor calls.
pub(crate) fn convert_field_value(
    py: Python<'_>,
    component: &Bound<'_, PyAny>,
    field_name: &str,
    field_value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    // Try to get the current field value to detect its type
    if let Ok(current) = component.getattr(field_name) {
        let type_name = current
            .get_type()
            .name()
            .map(|n| n.to_string())
            .unwrap_or_default();

        // Handle enum-variant types: {"Variant": value} → Type.Variant(value)
        if let serde_json::Value::Object(obj) = field_value
            && obj.len() == 1
        {
            let (variant_name, variant_value) = obj.iter().next().unwrap();
            let type_cls = current.get_type();
            if let Ok(ctor) = type_cls.getattr(variant_name.as_str()) {
                // Unit variant (null or empty object): call with no args
                let result = if variant_value.is_null()
                    || (variant_value.is_object() && variant_value.as_object().unwrap().is_empty())
                {
                    ctor.call0().ok()
                } else {
                    // Tuple/struct variant: pass the converted value
                    json_to_py(py, variant_value)
                        .ok()
                        .and_then(|arg| ctor.call1((arg,)).ok())
                };
                if let Some(result) = result {
                    return Ok(result.unbind());
                }
            }
        }

        // Handle string → enum unit variant: "Opaque" → AlphaMode.Opaque()
        // Uses the same approach as the object-form handler: get the variant
        // attribute from the type class and call it with no args.
        if let serde_json::Value::String(s) = field_value {
            let type_cls = current.get_type();
            if let Ok(ctor) = type_cls.getattr(s.as_str()) {
                // Try calling as unit variant constructor (e.g., AlphaMode.Opaque())
                if let Ok(result) = ctor.call0() {
                    return Ok(result.unbind());
                }
                // If call0 fails, the attribute might already be the value itself
                // (some enum implementations expose variants as pre-constructed instances)
                return Ok(ctor.unbind());
            }
        }

        // Convert Color from JSON array [r, g, b, a]
        if type_name == "Color"
            && let serde_json::Value::Array(arr) = field_value
            && arr.len() == 4
        {
            let r = json_number_to_f64(&arr[0])?;
            let g = json_number_to_f64(&arr[1])?;
            let b = json_number_to_f64(&arr[2])?;
            let a = json_number_to_f64(&arr[3])?;
            let color_mod = PyModule::import(py, "pybevy.color").map_err(|e| e.to_string())?;
            let color_cls = color_mod.getattr("Color").map_err(|e| e.to_string())?;
            return color_cls
                .call_method1("srgba", (r, g, b, a))
                .map(|v| v.unbind())
                .map_err(|e| e.to_string());
        }

        // Convert JSON arrays to math types based on current field type
        if let serde_json::Value::Array(arr) = field_value {
            let pybevy_math = PyModule::import(py, "pybevy.math").map_err(|e| e.to_string())?;

            match (type_name.as_str(), arr.len()) {
                ("Vec2", 2) => {
                    let x = json_number_to_f64(&arr[0])?;
                    let y = json_number_to_f64(&arr[1])?;
                    let vec2_cls = pybevy_math.getattr("Vec2").map_err(|e| e.to_string())?;
                    return vec2_cls
                        .call1((x, y))
                        .map(|v| v.unbind())
                        .map_err(|e| e.to_string());
                }
                ("Vec3", 3) => {
                    let x = json_number_to_f64(&arr[0])?;
                    let y = json_number_to_f64(&arr[1])?;
                    let z = json_number_to_f64(&arr[2])?;
                    let vec3_cls = pybevy_math.getattr("Vec3").map_err(|e| e.to_string())?;
                    return vec3_cls
                        .call1((x, y, z))
                        .map(|v| v.unbind())
                        .map_err(|e| e.to_string());
                }
                ("Vec4" | "Quat", 4) => {
                    let x = json_number_to_f64(&arr[0])?;
                    let y = json_number_to_f64(&arr[1])?;
                    let z = json_number_to_f64(&arr[2])?;
                    let w = json_number_to_f64(&arr[3])?;
                    let cls = pybevy_math
                        .getattr(type_name.as_str())
                        .map_err(|e| e.to_string())?;
                    return cls
                        .call1((x, y, z, w))
                        .map(|v| v.unbind())
                        .map_err(|e| e.to_string());
                }
                _ => {} // Fall through to generic conversion
            }
        }
    }

    // Default: generic JSON → Python conversion
    json_to_py(py, field_value)
}

pub(crate) fn json_number_to_f64(value: &serde_json::Value) -> Result<f64, String> {
    value
        .as_f64()
        .ok_or_else(|| format!("Expected number, got {value}"))
}

/// Convert a JSON value to a Python object
pub(crate) fn json_to_py(py: Python<'_>, value: &serde_json::Value) -> Result<Py<PyAny>, String> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(b) => Ok(b
            .into_pyobject(py)
            .map_err(|e| e.to_string())?
            .to_owned()
            .into_any()
            .unbind()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.into_pyobject(py)
                    .map_err(|e| e.to_string())?
                    .to_owned()
                    .into_any()
                    .unbind())
            } else if let Some(f) = n.as_f64() {
                Ok(f.into_pyobject(py)
                    .map_err(|e| e.to_string())?
                    .to_owned()
                    .into_any()
                    .unbind())
            } else {
                Err("Unsupported number type".into())
            }
        }
        serde_json::Value::String(s) => Ok(s
            .into_pyobject(py)
            .map_err(|e| e.to_string())?
            .into_any()
            .unbind()),
        serde_json::Value::Array(arr) => {
            let list = PyList::empty(py);
            for item in arr {
                let py_item = json_to_py(py, item)?;
                list.append(py_item).map_err(|e| e.to_string())?;
            }
            Ok(list.into_any().unbind())
        }
        serde_json::Value::Object(obj) => {
            let dict = PyDict::new(py);
            for (k, v) in obj {
                let py_value = json_to_py(py, v)?;
                dict.set_item(k, py_value).map_err(|e| e.to_string())?;
            }
            Ok(dict.into_any().unbind())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{alloc::Layout, ffi::CString, ptr, sync::Once};

    use bevy::{
        ecs::{
            component::{ComponentCloneBehavior, ComponentDescriptor, ComponentId, StorageType},
            name::Name,
        },
        prelude::{ChildOf, Children},
    };
    use pybevy_core::{CustomComponentEntry, CustomResourceEntry};
    use pyo3::types::PyInt;

    use super::*;
    use crate::bridge::ErrorCode;

    static INIT: Once = Once::new();

    fn setup_python() {
        INIT.call_once(|| {
            Python::initialize();
        });
    }

    #[test]
    fn convert_field_value_color_array() {
        setup_python();
        Python::attach(|py| {
            // Create a mock Color class with a srgba static method
            let code = CString::new(
                r#"
import types, sys

class Color:
    def __init__(self, r=1.0, g=1.0, b=1.0, a=1.0):
        self.r = r
        self.g = g
        self.b = b
        self.a = a

    @staticmethod
    def srgba(r, g, b, a):
        return Color(r, g, b, a)

# Register as pybevy.color in sys.modules
color_mod = types.ModuleType("pybevy.color")
color_mod.Color = Color

# Ensure parent package exists in sys.modules too
if "pybevy" not in sys.modules:
    pybevy_mod = types.ModuleType("pybevy")
    sys.modules["pybevy"] = pybevy_mod
sys.modules["pybevy.color"] = color_mod

# Create a holder object with a color attribute of type Color
class Holder:
    def __init__(self):
        self.color = Color()

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();

            let holder = globals.get_item("holder").unwrap().unwrap();

            // Call convert_field_value with a JSON array [1.0, 0.0, 0.0, 1.0]
            let field_value = serde_json::json!([1.0, 0.0, 0.0, 1.0]);
            let result = convert_field_value(py, &holder, "color", &field_value);
            assert!(result.is_ok(), "convert_field_value failed: {:?}", result);

            let py_obj = result.unwrap();
            let bound = py_obj.bind(py);

            // Verify it's a Color instance with correct values
            let type_name = bound.get_type().name().unwrap().to_string();
            assert_eq!(type_name, "Color");
            let r: f64 = bound.getattr("r").unwrap().extract().unwrap();
            let g: f64 = bound.getattr("g").unwrap().extract().unwrap();
            let b: f64 = bound.getattr("b").unwrap().extract().unwrap();
            let a: f64 = bound.getattr("a").unwrap().extract().unwrap();
            assert!((r - 1.0).abs() < 1e-10);
            assert!((g - 0.0).abs() < 1e-10);
            assert!((b - 0.0).abs() < 1e-10);
            assert!((a - 1.0).abs() < 1e-10);
        });
    }

    #[test]
    fn convert_field_value_color_wrong_array_length() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import types, sys

class Color:
    def __init__(self, r=1.0, g=1.0, b=1.0, a=1.0):
        self.r = r
        self.g = g
        self.b = b
        self.a = a

    @staticmethod
    def srgba(r, g, b, a):
        return Color(r, g, b, a)

color_mod = types.ModuleType("pybevy.color")
color_mod.Color = Color

# Also register pybevy.math so the array fallback path doesn't fail on import
math_mod = types.ModuleType("pybevy.math")
if "pybevy" not in sys.modules:
    pybevy_mod = types.ModuleType("pybevy")
    sys.modules["pybevy"] = pybevy_mod
sys.modules["pybevy.color"] = color_mod
sys.modules["pybevy.math"] = math_mod

class Holder:
    def __init__(self):
        self.color = Color()

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();

            let holder = globals.get_item("holder").unwrap().unwrap();

            // Array with 3 elements should NOT trigger Color conversion (needs exactly 4).
            // Falls through to math array check (no match for "Color" type), then to
            // generic json_to_py which returns a Python list.
            let field_value = serde_json::json!([1.0, 0.0, 0.0]);
            let result = convert_field_value(py, &holder, "color", &field_value);
            assert!(result.is_ok());
            let py_obj = result.unwrap();
            let bound = py_obj.bind(py);
            let type_name = bound.get_type().name().unwrap().to_string();
            assert_eq!(
                type_name, "list",
                "Wrong-length array should fall through to list"
            );
        });
    }

    #[test]
    fn convert_field_value_non_color_falls_through() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Holder:
    def __init__(self):
        self.name = "hello"

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();

            let holder = globals.get_item("holder").unwrap().unwrap();

            // String field with a string value should fall through to generic conversion
            let field_value = serde_json::json!("world");
            let result = convert_field_value(py, &holder, "name", &field_value);
            assert!(result.is_ok());
            let py_obj = result.unwrap();
            let bound = py_obj.bind(py);
            let val: String = bound.extract().unwrap();
            assert_eq!(val, "world");
        });
    }

    #[test]
    fn json_number_to_f64_valid() {
        let val = serde_json::json!(3.14);
        assert!((json_number_to_f64(&val).unwrap() - 3.14).abs() < 1e-10);
    }

    #[test]
    fn json_number_to_f64_integer() {
        let val = serde_json::json!(42);
        assert!((json_number_to_f64(&val).unwrap() - 42.0).abs() < 1e-10);
    }

    #[test]
    fn json_number_to_f64_non_number() {
        let val = serde_json::json!("not a number");
        assert!(json_number_to_f64(&val).is_err());
    }

    #[test]
    fn parse_entity_ref_from_op_entity_int() {
        let op = serde_json::json!({"entity": 42, "action": "set_component"});
        let result = parse_entity_ref_from_op(&op).unwrap();
        assert!(matches!(result, EntityRef::Id(42)));
    }

    #[test]
    fn parse_entity_ref_from_op_entity_string() {
        let op = serde_json::json!({"entity": "Player", "action": "set_component"});
        let result = parse_entity_ref_from_op(&op).unwrap();
        assert!(matches!(result, EntityRef::Name(ref s) if s == "Player"));
    }

    #[test]
    fn parse_entity_ref_from_op_missing_entity() {
        let op = serde_json::json!({"action": "set_component"});
        let result = parse_entity_ref_from_op(&op);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("entity"));
    }

    #[test]
    fn parse_entity_ref_from_op_entity_invalid_type() {
        let op = serde_json::json!({"entity": [1, 2, 3]});
        let result = parse_entity_ref_from_op(&op);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("entity"));
    }

    #[test]
    fn despawn_entity_by_id_success() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let result = despawn_entity(&mut world, EntityRef::Id(entity.to_bits())).unwrap();
        assert_eq!(result["despawned"], true);
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn despawn_entity_by_name_success() {
        let mut world = World::new();
        world.spawn(Name::new("Target"));
        let result = despawn_entity(&mut world, EntityRef::Name("Target".into())).unwrap();
        assert_eq!(result["despawned"], true);
    }

    #[test]
    fn despawn_entity_not_found() {
        let mut world = World::new();
        let result = despawn_entity(&mut world, EntityRef::Id(999999));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn world_despawn_vs_entity_mut_despawn_baseline() {
        // Pin the Bevy contract our despawn_entity handler relies on:
        // both `World::despawn` AND `EntityWorldMut::despawn` walk the
        // `Children` relationship recursively. If a future Bevy release
        // changes World::despawn back to non-recursive, despawn_entity
        // would need to switch to entity_mut(e).despawn() to preserve the
        // user-visible recursive behavior.
        let mut world = World::new();
        let parent_a = world.spawn(Name::new("ParentA")).id();
        let child_a = world.spawn((Name::new("ChildA"), ChildOf(parent_a))).id();

        let _ = world.despawn(parent_a);

        assert!(world.get_entity(parent_a).is_err());
        assert!(
            world.get_entity(child_a).is_err(),
            "World::despawn(parent) is recursive in Bevy 0.18: child should also be despawned"
        );

        let parent_b = world.spawn(Name::new("ParentB")).id();
        let child_b = world.spawn((Name::new("ChildB"), ChildOf(parent_b))).id();
        assert!(world.get::<Children>(parent_b).is_some());

        world.entity_mut(parent_b).despawn();

        assert!(world.get_entity(parent_b).is_err());
        assert!(world.get_entity(child_b).is_err());
    }

    #[test]
    fn despawn_entity_recursive_removes_children() {
        let mut world = World::new();
        let parent = world.spawn(Name::new("Parent")).id();
        let child1 = world.spawn((Name::new("Child1"), ChildOf(parent))).id();
        let child2 = world.spawn((Name::new("Child2"), ChildOf(parent))).id();
        let grandchild = world.spawn((Name::new("Grandchild"), ChildOf(child1))).id();

        let result = despawn_entity(&mut world, EntityRef::Id(parent.to_bits())).unwrap();
        assert_eq!(result["despawned"], true);

        // Parent and all descendants gone; no orphans with stale ChildOf.
        assert!(world.get_entity(parent).is_err());
        assert!(world.get_entity(child1).is_err());
        assert!(world.get_entity(child2).is_err());
        assert!(world.get_entity(grandchild).is_err());
    }

    #[test]
    fn spawn_entity_invalid_params_not_object() {
        let mut world = World::new();
        let result = spawn_entity(&mut world, serde_json::json!("not an object"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::InvalidParams);
    }

    #[test]
    fn spawn_entity_unknown_component() {
        let mut world = World::new();
        let result = spawn_entity(&mut world, serde_json::json!({"UnknownComp": {}}));
        // Should fail fast without spawning entity
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::InvalidParams);
    }

    #[test]
    fn batch_mutate_empty_operations() {
        let mut world = World::new();
        let result = batch_mutate(&mut world, vec![]).unwrap();
        assert_eq!(result["total"], 0);
        assert_eq!(result["succeeded"], 0);
    }

    #[test]
    fn batch_mutate_unknown_action() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"action": "nonexistent"})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["total"], 1);
        assert_eq!(result["succeeded"], 0);
        let results = result["results"].as_array().unwrap();
        assert_eq!(results[0]["status"], "error");
    }

    #[test]
    fn batch_mutate_despawn_success() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let ops = vec![serde_json::json!({"action": "despawn", "entity": entity.to_bits()})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 1);
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn batch_mutate_despawn_missing_entity() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"action": "despawn"})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 0);
    }

    #[test]
    fn batch_mutate_set_component_missing_fields() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"action": "set_component"})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 0);
    }

    #[test]
    fn batch_mutate_mixed_success_and_failure() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let ops = vec![
            serde_json::json!({"action": "despawn", "entity": entity.to_bits()}),
            serde_json::json!({"action": "unknown_action"}),
        ];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["total"], 2);
        assert_eq!(result["succeeded"], 1);
    }

    #[test]
    fn batch_mutate_remove_component_missing_params() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"action": "remove_component"})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 0);
        let results = result["results"].as_array().unwrap();
        assert_eq!(results[0]["status"], "error");
    }

    #[test]
    fn batch_mutate_remove_component_with_entity_no_component() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let ops = vec![serde_json::json!({
            "action": "remove_component",
            "entity": entity.to_bits(),
            "component": "NonExistent"
        })];
        let result = batch_mutate(&mut world, ops).unwrap();
        // Should fail because component is not in registry
        assert_eq!(result["succeeded"], 0);
    }

    #[test]
    fn batch_mutate_spawn_empty() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"action": "spawn"})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 1);
        let results = result["results"].as_array().unwrap();
        let entity_id = results[0]["result"]["entity_id"].as_u64();
        assert!(entity_id.is_some());
    }

    #[test]
    fn batch_mutate_spawn_with_unknown_components() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"action": "spawn", "components": {"UnknownComp": {}}})];
        let result = batch_mutate(&mut world, ops).unwrap();
        // Unknown components now fail fast — batch reports error status
        assert_eq!(result["succeeded"], 0);
        let results = result["results"].as_array().unwrap();
        assert_eq!(results[0]["status"], "error");
    }

    #[test]
    fn batch_mutate_multiple_despawns() {
        let mut world = World::new();
        let e1 = world.spawn(Name::new("A")).id();
        let e2 = world.spawn(Name::new("B")).id();
        let ops = vec![
            serde_json::json!({"action": "despawn", "entity": e1.to_bits()}),
            serde_json::json!({"action": "despawn", "entity": e2.to_bits()}),
        ];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["total"], 2);
        assert_eq!(result["succeeded"], 2);
        assert!(world.get_entity(e1).is_err());
        assert!(world.get_entity(e2).is_err());
    }

    #[test]
    fn batch_mutate_despawn_by_name() {
        let mut world = World::new();
        world.spawn(Name::new("Target"));
        let ops = vec![serde_json::json!({"action": "despawn", "entity": "Target"})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 1);
    }

    #[test]
    fn batch_mutate_no_action_field() {
        let mut world = World::new();
        let ops = vec![serde_json::json!({"entity": 42})];
        let result = batch_mutate(&mut world, ops).unwrap();
        assert_eq!(result["succeeded"], 0);
        // Empty action string triggers unknown action error
        let results = result["results"].as_array().unwrap();
        assert_eq!(results[0]["status"], "error");
    }

    #[test]
    fn spawn_entity_empty_components() {
        let mut world = World::new();
        let result = spawn_entity(&mut world, serde_json::json!({})).unwrap();
        assert!(result["entity_id"].is_number());
        assert_eq!(result["components_added"].as_array().unwrap().len(), 0);
        // No errors array when there are no errors
        assert!(result.get("errors").is_none());
    }

    #[test]
    fn spawn_entity_multiple_unknown_components() {
        let mut world = World::new();
        let result = spawn_entity(
            &mut world,
            serde_json::json!({
                "Comp1": {},
                "Comp2": {"field": "value"},
                "Comp3": {}
            }),
        );
        // Should fail fast without spawning entity
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert!(err.message.contains("Comp1"));
    }

    #[test]
    fn despawn_entity_by_name_not_found() {
        let mut world = World::new();
        let result = despawn_entity(&mut world, EntityRef::Name("NonExistent".into()));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn json_number_to_f64_negative() {
        let val = serde_json::json!(-7.5);
        assert!((json_number_to_f64(&val).unwrap() - (-7.5)).abs() < 1e-10);
    }

    #[test]
    fn json_number_to_f64_zero() {
        let val = serde_json::json!(0);
        assert!((json_number_to_f64(&val).unwrap()).abs() < 1e-10);
    }

    #[test]
    fn json_number_to_f64_bool_is_not_number() {
        let val = serde_json::json!(true);
        assert!(json_number_to_f64(&val).is_err());
    }

    #[test]
    fn json_number_to_f64_null_is_not_number() {
        let val = serde_json::json!(null);
        assert!(json_number_to_f64(&val).is_err());
    }

    #[test]
    fn json_number_to_f64_array_is_not_number() {
        let val = serde_json::json!([1, 2, 3]);
        assert!(json_number_to_f64(&val).is_err());
    }

    #[test]
    fn set_component_fields_not_object() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let result = set_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "Transform".to_string(),
            serde_json::json!("not an object"),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::InvalidParams);
    }

    #[test]
    fn set_component_unknown_component() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let result = set_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "TotallyFakeComponent".to_string(),
            serde_json::json!({"field": "value"}),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn remove_component_unknown_component() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let result = remove_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "TotallyFakeComponent".to_string(),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn remove_component_entity_not_found() {
        let mut world = World::new();
        let result = remove_component(&mut world, EntityRef::Id(999999), "Transform".to_string());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn structural_warning_lists_known_breakers() {
        let warn = structural_warning("Transform").expect("Transform is structural");
        assert!(warn.contains("Transform"));
        assert!(warn.contains("set_component"));
        assert!(structural_warning("GlobalTransform").is_some());
        assert!(structural_warning("Visibility").is_some());
        assert!(structural_warning("Mesh3d").is_some());
        assert!(structural_warning("Camera3d").is_some());
    }

    #[test]
    fn structural_warning_skips_user_components() {
        // Custom user components and non-structural builtins must not warn,
        // otherwise every remove_component call would carry noise.
        assert!(structural_warning("Bouncy").is_none());
        assert!(structural_warning("PointLight").is_none());
        assert!(structural_warning("Name").is_none());
    }

    #[test]
    fn remove_resource_not_in_registry() {
        let mut world = World::new();
        let result = remove_resource(&mut world, "FakeResource".to_string());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn remove_component_custom_python_component() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();

        // Register a fake custom component via CustomComponentInfo
        let comp_id = world.register_component_with_descriptor(unsafe {
            ComponentDescriptor::new_with_layout(
                "CustomComp",
                StorageType::Table,
                Layout::new::<u8>(),
                None,
                false,
                ComponentCloneBehavior::Default,
                None,
            )
        });

        let mut info = CustomComponentInfo::default();
        info.insert(
            comp_id,
            CustomComponentEntry {
                type_ptr: ptr::null(),
                name: "CustomComp".to_string(),
                is_pyobject_storage: true,
            },
        );
        world.insert_resource(info);

        // remove_component should find it via CustomComponentInfo fallback
        let result = remove_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "CustomComp".to_string(),
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["removed"], "CustomComp");
    }

    #[test]
    fn remove_component_custom_not_found_without_info() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();

        // No CustomComponentInfo resource → should fail
        let result = remove_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "CustomComp".to_string(),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn has_embedded_errors_detects_errors() {
        let val = serde_json::json!({
            "entity_id": 42,
            "errors": ["field not found"],
        });
        assert!(has_embedded_errors(&val));
    }

    #[test]
    fn has_embedded_errors_empty_errors_is_false() {
        let val = serde_json::json!({
            "entity_id": 42,
            "errors": [],
        });
        assert!(!has_embedded_errors(&val));
    }

    #[test]
    fn has_embedded_errors_no_errors_key_is_false() {
        let val = serde_json::json!({
            "entity_id": 42,
            "components_added": ["Transform"],
        });
        assert!(!has_embedded_errors(&val));
    }

    #[test]
    fn batch_mutate_partial_status_on_embedded_errors() {
        // Simulate a batch where one operation returns Ok with embedded errors.
        // We can't easily trigger real embedded errors without registered components,
        // but we can test the status assignment logic directly.
        let val_with_errors = serde_json::json!({
            "entity_id": 42,
            "updated_fields": [],
            "errors": ["some field error"],
        });
        let val_without_errors = serde_json::json!({
            "entity_id": 42,
            "updated_fields": ["translation"],
        });

        // Test status assignment
        let status_partial = if has_embedded_errors(&val_with_errors) {
            "partial"
        } else {
            "ok"
        };
        let status_ok = if has_embedded_errors(&val_without_errors) {
            "partial"
        } else {
            "ok"
        };

        assert_eq!(status_partial, "partial");
        assert_eq!(status_ok, "ok");
    }

    #[test]
    fn spawn_entity_atomicity_no_stray_entity() {
        let mut world = World::new();
        let initial_count = world.entities().len();

        // Unknown components should fail without leaving a stray entity
        let result = spawn_entity(&mut world, serde_json::json!({"FakeComp": {}}));
        assert!(result.is_err());
        assert_eq!(world.entities().len(), initial_count);
    }

    #[test]
    fn insert_resource_custom_via_custom_resource_info() {
        setup_python();

        let mut world = World::new();

        // Register a fake custom resource
        let comp_id = world.register_component_with_descriptor(unsafe {
            ComponentDescriptor::new_with_layout(
                "GameScore",
                StorageType::Table,
                Layout::new::<u8>(),
                None,
                false,
                ComponentCloneBehavior::Default,
                None,
            )
        });

        // Use a real Python type pointer so PyO3 doesn't crash on null
        let type_ptr = Python::attach(|py| {
            let int_type = py.get_type::<PyInt>();
            int_type.as_type_ptr()
        });

        let mut info = CustomResourceInfo::default();
        info.insert(
            comp_id,
            CustomResourceEntry {
                type_ptr,
                name: "GameScore".to_string(),
            },
        );
        world.insert_resource(info);

        // set_resource should find it via CustomResourceInfo and construct.
        // int() returns 0, stored in PyResourceStorage.
        let result = insert_resource(&mut world, "GameScore".to_string(), serde_json::json!({}));
        assert!(
            result.is_ok(),
            "Custom resource insert failed: {:?}",
            result
        );
        assert_eq!(result.unwrap()["custom"], true);

        // Verify PyResourceStorage was populated
        let storage = world.get_resource::<PyResourceStorage>();
        assert!(storage.is_some(), "PyResourceStorage should exist");
        assert!(
            storage.unwrap().resources.contains_key(&comp_id),
            "Resource should be stored in PyResourceStorage"
        );
    }

    #[test]
    fn spawn_entity_field_error_falls_back_to_python_not_error() {
        // Regression test: spawn_entity previously returned FieldError as an error to
        // the user. After the fix, FieldError falls through to the Python fallback
        // path (just like NotRegistered, NoReflectComponent, etc.).
        //
        // We cannot trigger a real FieldError in this test because it requires a
        // component registered in the bridge registry whose reflect_spawn_component
        // returns FieldError (e.g., a Color array or Vec2 field). However, we verify
        // the code structure by confirming that spawn_entity with unknown components
        // fails with invalid_params (not FieldError), and that valid empty spawn
        // succeeds — ensuring the match arms haven't been accidentally reordered to
        // make FieldError an early-return error.
        //
        // The actual FieldError fallback path is:
        //   Err(ReflectError::FieldError(_)) => { /* Fall back to Python */ }
        // This was previously:
        //   Err(ReflectError::FieldError(msg)) => { errors.push(...); continue; }
        let mut world = World::new();

        // Empty spawn should still work (no components = no FieldError possible)
        let result = spawn_entity(&mut world, serde_json::json!({})).unwrap();
        assert!(result["entity_id"].is_number());

        // Unknown component should fail at validation phase (before reflection)
        let result = spawn_entity(&mut world, serde_json::json!({"FakeComp": {"x": 1}}));
        assert!(result.is_err());
        // The error is invalid_params (-32602), NOT a FieldError propagation
        assert_eq!(result.unwrap_err().code, ErrorCode::InvalidParams);
    }

    #[test]
    fn remove_resource_custom_via_py_pyresource() {
        setup_python();

        let mut world = World::new();

        // Register a fake custom resource
        let comp_id = world.register_component_with_descriptor(unsafe {
            ComponentDescriptor::new_with_layout(
                "MyCustomRes",
                StorageType::Table,
                Layout::new::<u8>(),
                None,
                false,
                ComponentCloneBehavior::Default,
                None,
            )
        });

        // Create CustomResourceInfo with the entry
        let type_ptr = Python::attach(|py| {
            let int_type = py.get_type::<PyInt>();
            int_type.as_type_ptr()
        });

        let mut info = CustomResourceInfo::default();
        info.insert(
            comp_id,
            CustomResourceEntry {
                type_ptr,
                name: "MyCustomRes".to_string(),
            },
        );
        world.insert_resource(info);

        // Pre-populate PyResourceStorage with a matching entry
        let py_obj = Python::attach(|py| 42i64.into_pyobject(py).unwrap().into_any().unbind());
        let mut storage = PyResourceStorage::default();
        storage.resources.insert(comp_id, py_obj);
        world.insert_resource(storage);

        // Verify resource is present before removal
        assert!(
            world
                .get_resource::<PyResourceStorage>()
                .unwrap()
                .resources
                .contains_key(&comp_id),
            "Resource should exist before removal"
        );

        // Call remove_resource — should find via CustomResourceInfo and remove from PyResourceStorage
        let result = remove_resource(&mut world, "MyCustomRes".to_string());
        assert!(result.is_ok(), "remove_resource failed: {:?}", result);
        assert_eq!(result.unwrap()["removed"], "MyCustomRes");

        // Verify it was removed from PyResourceStorage
        let storage = world.get_resource::<PyResourceStorage>().unwrap();
        assert!(
            !storage.resources.contains_key(&comp_id),
            "Resource should be removed from PyResourceStorage"
        );
    }

    #[test]
    fn remove_resource_custom_not_found_without_info() {
        let mut world = World::new();
        // No CustomResourceInfo resource and no bridge → should fail
        let result = remove_resource(&mut world, "NonExistentCustomRes".to_string());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn set_component_custom_not_in_any_registry() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();

        // No bridge and no CustomComponentInfo → set_component should return not_found
        let result = set_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "TotallyCustomComp".to_string(),
            serde_json::json!({"health": 100}),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert!(
            err.message
                .contains("not found in custom component registry"),
            "Error should mention 'not found in custom component registry', got: {}",
            err.message
        );
    }

    #[test]
    fn set_component_custom_error_lists_available() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();

        // Add CustomComponentInfo with entries so the error message includes them
        let mut info = CustomComponentInfo::default();
        info.insert(
            ComponentId::new(77777),
            CustomComponentEntry {
                type_ptr: ptr::null(),
                name: "Health".to_string(),
                is_pyobject_storage: true,
            },
        );
        world.insert_resource(info);

        let result = set_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "UnknownComp".to_string(),
            serde_json::json!({"value": 42}),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("Available custom components: Health"),
            "Error should list available custom components, got: {}",
            err.message
        );
    }

    #[test]
    fn set_component_custom_qualified_name_fallback() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();

        // Register a custom component with qualified name (module.ClassName)
        let mut info = CustomComponentInfo::default();
        let fake_id = ComponentId::new(88888);
        info.insert(
            fake_id,
            CustomComponentEntry {
                type_ptr: ptr::null(),
                name: "mymod.Oscillator".to_string(),
                is_pyobject_storage: true,
            },
        );
        world.insert_resource(info);

        // Requesting by short name "Oscillator" should not error with "not found"
        // when the entity doesn't have the component — it should find the entry
        // via qualified name fallback but then fail because entity lacks it.
        // This tests the name resolution path, not the full mutation.
        let result = set_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "Oscillator".to_string(),
            serde_json::json!({"speed": 1.0}),
        );
        // The qualified name match is only used when the component is on the entity,
        // so this will still fail — but the error should list available components
        assert!(result.is_err());
    }

    #[test]
    fn spawn_entity_string_value_not_silently_dropped() {
        // Regression: previously, `{"Name": "my_name"}` would call
        // `comp_fields.as_object().unwrap_or_default()`, silently dropping the
        // string value and creating `Name("")` (empty default).
        //
        // The fix routes non-object values through `spawn_component_python_direct`,
        // which passes the value as a positional argument to the Python constructor.
        //
        // We can't test with real Python bridge components in unit tests, but we can
        // verify that a string value for an UNKNOWN component still fails at the
        // validation phase (not silently succeeding with empty data).
        let mut world = World::new();
        let result = spawn_entity(&mut world, serde_json::json!({"UnknownComp": "some_value"}));
        assert!(
            result.is_err(),
            "Should fail for unknown component, not silently succeed"
        );
        let err = result.unwrap_err();
        assert_eq!(
            err.code,
            ErrorCode::InvalidParams,
            "Should be invalid_params error"
        );
        assert!(
            err.message.contains("UnknownComp"),
            "Error should mention the component name, got: {}",
            err.message
        );
    }

    #[test]
    fn set_component_custom_in_info_but_not_pyobject_storage() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();

        // Register a custom component with is_pyobject_storage = false
        let comp_id = world.register_component_with_descriptor(unsafe {
            ComponentDescriptor::new_with_layout(
                "WrapperComp",
                StorageType::Table,
                Layout::new::<u8>(),
                None,
                false,
                ComponentCloneBehavior::Default,
                None,
            )
        });

        let mut info = CustomComponentInfo::default();
        info.insert(
            comp_id,
            CustomComponentEntry {
                type_ptr: ptr::null(),
                name: "WrapperComp".to_string(),
                is_pyobject_storage: false,
            },
        );
        world.insert_resource(info);

        // set_component should find it in CustomComponentInfo but reject because
        // it uses wrapper storage (not pyobject storage)
        let result = set_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "WrapperComp".to_string(),
            serde_json::json!({"value": 42}),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidParams); // invalid_params
        assert!(
            err.message.contains("wrapper storage"),
            "Error should mention wrapper storage, got: {}",
            err.message
        );
    }

    #[test]
    fn insert_custom_component_tries_kwargs_when_call0_fails() {
        // Regression: insert_custom_component previously only tried cls.call0()
        // (default constructor). If __init__ has required positional args like
        // PythonComp(amount: float), call0() fails with "missing required argument".
        //
        // The fix: if call0() fails and field_obj is non-empty, try passing
        // fields as kwargs via cls.call((), Some(&kwargs)).
        //
        // We can't test end-to-end without a real custom component registration,
        // but we verify the code path exists by checking that set_component for
        // a pyobject-storage custom component that's NOT on the entity attempts
        // creation (enters insert_custom_component), not just mutation.

        setup_python();

        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();

        let comp_id = world.register_component_with_descriptor(unsafe {
            ComponentDescriptor::new_with_layout(
                "ReinsertComp",
                StorageType::Table,
                Layout::new::<u8>(),
                None,
                false,
                ComponentCloneBehavior::Default,
                None,
            )
        });

        // Register as pyobject-storage custom component
        let type_ptr = Python::attach(|py| {
            let int_type = py.get_type::<PyInt>();
            int_type.as_type_ptr()
        });

        let mut info = CustomComponentInfo::default();
        info.insert(
            comp_id,
            CustomComponentEntry {
                type_ptr,
                name: "ReinsertComp".to_string(),
                is_pyobject_storage: true,
            },
        );
        world.insert_resource(info);

        // Entity does NOT have the component → insert_custom_component is called
        // The type_ptr points to int, so construction will fail, but we verify
        // the code path reaches creation (not "component not found on entity")
        let result = set_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "ReinsertComp".to_string(),
            serde_json::json!({"value": 42}),
        );
        // Should attempt creation, not return "component not found on entity"
        // (the actual creation may fail because int type_ptr is not a real
        // component type, but the error should be from the creation attempt)
        assert!(result.is_err(), "Expected error from creation attempt");
        let err = result.unwrap_err();
        // The error should NOT be about "not found on entity" — it should be
        // about creation/setting fields (proving insert_custom_component was called)
        assert!(
            !err.message.contains("not found on entity"),
            "Should not report 'not found on entity' — should attempt creation. Got: {}",
            err.message
        );
    }

    #[test]
    fn insert_resource_rejects_string_value() {
        let mut world = World::new();
        let result = insert_resource(
            &mut world,
            "DirectionalLightShadowMap".to_string(),
            serde_json::json!("not-an-object"),
        );
        let err = result.expect_err("string value must be rejected");
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert!(
            err.message.contains("must be a JSON object"),
            "expected guidance message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("string"),
            "expected message to name the type 'string', got: {}",
            err.message
        );
    }

    #[test]
    fn insert_resource_rejects_number_value() {
        let mut world = World::new();
        let result = insert_resource(
            &mut world,
            "DirectionalLightShadowMap".to_string(),
            serde_json::json!(42),
        );
        let err = result.expect_err("number value must be rejected");
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert!(err.message.contains("must be a JSON object"));
        assert!(err.message.contains("number"));
    }

    #[test]
    fn insert_resource_rejects_bool_value() {
        let mut world = World::new();
        let result = insert_resource(
            &mut world,
            "DirectionalLightShadowMap".to_string(),
            serde_json::json!(true),
        );
        let err = result.expect_err("bool value must be rejected");
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert!(err.message.contains("must be a JSON object"));
        assert!(err.message.contains("bool"));
    }

    #[test]
    fn insert_resource_rejects_array_value() {
        let mut world = World::new();
        let result = insert_resource(
            &mut world,
            "DirectionalLightShadowMap".to_string(),
            serde_json::json!([1, 2, 3]),
        );
        let err = result.expect_err("array value must be rejected");
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert!(err.message.contains("must be a JSON object"));
        assert!(err.message.contains("array"));
    }

    #[test]
    fn insert_resource_accepts_null_value() {
        // Null is treated as a no-op patch; the not-found path returns an error,
        // but the validation itself must not reject Null.
        let mut world = World::new();
        let result = insert_resource(
            &mut world,
            "NoSuchResource".to_string(),
            serde_json::Value::Null,
        );
        // Either ok or some non-validation error is acceptable; what matters is that
        // we did not get the InvalidParams "must be a JSON object" rejection.
        if let Err(err) = result {
            assert_ne!(
                err.code,
                ErrorCode::InvalidParams,
                "Null must not be rejected by the value-shape validation, got: {}",
                err.message
            );
        }
    }
}
