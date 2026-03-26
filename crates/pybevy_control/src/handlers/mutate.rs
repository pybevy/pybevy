use std::sync::Arc;

use bevy::ecs::world::World;
use pybevy_core::ComponentBridge;
use pyo3::{prelude::*, types::PyModule};

use super::{
    reflect_mutate::{self, ReflectError},
    scene::resolve_entity,
};
use crate::bridge::{ControlError, EntityRef};

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
                let kwargs = pyo3::types::PyDict::new(py);
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
        .get_resource::<pybevy_core::CustomComponentInfo>()
        .and_then(|info| {
            info.iter()
                .find(|(_, entry)| entry.name == component)
                .map(|(id, entry)| (id, entry.is_pyobject_storage))
        });

    // Fallback: check for qualified name variants (e.g. "module.Oscillator" vs "Oscillator")
    // and scan the entity's archetype for matching custom components
    let custom_info = custom_info.or_else(|| {
        let info = world.get_resource::<pybevy_core::CustomComponentInfo>()?;
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
                && comp_info.type_id().is_some() {
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
            .get_resource::<pybevy_core::CustomComponentInfo>()
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
        .get_resource::<pybevy_core::CustomComponentInfo>()
        .and_then(|info| info.get(comp_id).map(|entry| entry.type_ptr))
        .ok_or_else(|| ControlError::internal("Custom component info lost".to_string()))?;

    Python::attach(|py| {
        let py_type =
            unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };
        let Ok(cls) = py_type.cast::<pyo3::types::PyType>() else {
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
                let kwargs = pyo3::types::PyDict::new(py);
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
            let ptr = std::ptr::addr_of!(py_obj) as *const u8;
            let data = core::ptr::NonNull::new_unchecked(ptr as *mut u8);
            entity_mut.insert_by_id(comp_id, bevy::ptr::OwningPtr::new(data));
        }
        std::mem::forget(py_obj); // Ownership transferred to ECS

        Ok(serde_json::json!({
            "entity_id": entity_id,
            "component": component,
            "updated_fields": updated_fields,
            "inserted": true,
        }))
    })
}

/// Remove a component from an entity
pub fn remove_component(
    world: &mut World,
    entity_ref: EntityRef,
    component: String,
) -> Result<serde_json::Value, ControlError> {
    let entity = resolve_entity(world, &entity_ref)?;
    let entity_id = entity.to_bits();

    for bridge in pybevy_core::registry::global_registry::all_component_bridges() {
        if bridge.name() == component.as_str() {
            let component_id = bridge.register(world);
            if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                entity_mut.remove_by_id(component_id);
                return Ok(serde_json::json!({
                    "entity_id": entity_id,
                    "removed": component,
                }));
            } else {
                return Err(ControlError::not_found(format!(
                    "Entity {entity_id} not found"
                )));
            }
        }
    }

    // Fallback: check custom Python components via CustomComponentInfo
    let custom_comp_id = world
        .get_resource::<pybevy_core::CustomComponentInfo>()
        .and_then(|info| {
            info.iter()
                .find(|(_, entry)| entry.name == component)
                .map(|(id, _)| id)
        });

    if let Some(comp_id) = custom_comp_id {
        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
            entity_mut.remove_by_id(comp_id);
            return Ok(serde_json::json!({
                "entity_id": entity_id,
                "removed": component,
            }));
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
    // Try bridge registry first
    let bridge_result = Python::attach(|py| {
        for bridge in pybevy_core::registry::global_registry::all_resource_bridges() {
            if bridge.name() == resource_type.as_str() {
                // Patch semantics: if resource already exists, mutate in-place
                // to preserve fields not included in the update
                if bridge.contains_in_world(world) {
                    if let Some(obj) = value.as_object()
                        && !obj.is_empty() {
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
                                        if let Err(e) =
                                            instance.setattr(field_name.as_str(), py_value)
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
    let custom_entry = world
        .get_resource::<pybevy_core::CustomResourceInfo>()
        .and_then(|info| {
            info.iter()
                .find(|(_, entry)| entry.name == resource_type)
                .map(|(id, entry)| (id, entry.type_ptr))
        });

    if let Some((comp_id, type_ptr)) = custom_entry {
        Python::attach(|py| {
            // Patch semantics: if custom resource already exists, mutate in-place
            if let Some(storage) = world.get_resource::<pybevy_core::PyResourceStorage>()
                && let Some(existing) = storage.resources.get(&comp_id) {
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
            let py_type =
                unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut pyo3::ffi::PyObject) };
            let Ok(cls) = py_type.cast::<pyo3::types::PyType>() else {
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
            if !world.contains_resource::<pybevy_core::PyResourceStorage>() {
                world.insert_resource(pybevy_core::PyResourceStorage::default());
            }
            world
                .resource_mut::<pybevy_core::PyResourceStorage>()
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
    let custom_comp_id = world
        .get_resource::<pybevy_core::CustomResourceInfo>()
        .and_then(|info| {
            info.iter()
                .find(|(_, entry)| entry.name == resource_type)
                .map(|(id, _)| id)
        });

    if let Some(comp_id) = custom_comp_id {
        if let Some(storage) = world.get_resource_mut::<pybevy_core::PyResourceStorage>() {
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
                    _ => Err(ControlError::invalid_params(format!(
                        "op[{i}]: set_component requires entity_id/name, component, fields"
                    ))),
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
                    _ => Err(ControlError::invalid_params(format!(
                        "op[{i}]: remove_component requires entity_id/name and component"
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

/// Parse entity ref from a batch operation JSON object
fn parse_entity_ref_from_op(op: &serde_json::Value) -> Result<EntityRef, String> {
    if let Some(id) = op.get("entity_id").and_then(|v| v.as_u64()) {
        return Ok(EntityRef::Id(id));
    }
    if let Some(name) = op.get("name").and_then(|v| v.as_str()) {
        return Ok(EntityRef::Name(name.to_string()));
    }
    Err("Missing 'entity_id' or 'name'".into())
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
            let list = pyo3::types::PyList::empty(py);
            for item in arr {
                let py_item = json_to_py(py, item)?;
                list.append(py_item).map_err(|e| e.to_string())?;
            }
            Ok(list.into_any().unbind())
        }
        serde_json::Value::Object(obj) => {
            let dict = pyo3::types::PyDict::new(py);
            for (k, v) in obj {
                let py_value = json_to_py(py, v)?;
                dict.set_item(k, py_value).map_err(|e| e.to_string())?;
            }
            Ok(dict.into_any().unbind())
        }
    }
}
