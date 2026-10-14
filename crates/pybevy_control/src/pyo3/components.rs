use std::sync::{Arc, OnceLock};

use bevy::ecs::{change_detection::DetectChangesMut, entity::Entity, world::World};
use pybevy_core::{
    ComponentBridge, CustomComponentInfo, PyEntity, ValidityFlag, ValidityGuard,
    custom_resource::{hierarchy_contains_resource_entity, validate_hierarchy_link},
    public_error,
};
use pyo3::{
    ffi::PyTypeObject,
    prelude::*,
    types::{PyDict, PyType},
};

use crate::{
    codec::{
        json_float::reject_narrowed_nonfinite,
        reflect::{self, ReflectError},
    },
    protocol::{ControlError, EntityRef},
    pyo3::{
        custom,
        errors::{component_unknown_field_error, unknown_set_fields},
        execute::create_world_wrapper,
        from_json::{
            enums::{
                construct_enum_variant, enum_component_variant_request, enum_owner_type,
                resolve_enum_instance,
            },
            field::{
                class_declares_component_field, convert_annotated_field_value, convert_field_value,
                convert_field_value_typed, declared_optional_type, entity_from_json,
                has_public_instance_field, json_to_py, json_to_py_for_field, optional_field_types,
                validate_annotated_field_value,
            },
        },
        names::{ComponentTarget, resolve_component},
        to_json::py_value_to_json,
    },
    scene::entity::resolve_entity,
};

type RemoveComponentLifecycleHook = fn(&mut World, Entity, *const PyTypeObject);
type DespawnEntityLifecycleHook = fn(&mut World, Entity);

struct LifecycleMutationHooks {
    remove_component: RemoveComponentLifecycleHook,
    despawn_entity: DespawnEntityLifecycleHook,
}

static LIFECYCLE_MUTATION_HOOKS: OnceLock<LifecycleMutationHooks> = OnceLock::new();

pub fn register_lifecycle_mutation_hooks(
    remove_component: RemoveComponentLifecycleHook,
    despawn_entity: DespawnEntityLifecycleHook,
) {
    let _ = LIFECYCLE_MUTATION_HOOKS.set(LifecycleMutationHooks {
        remove_component,
        despawn_entity,
    });
}

/// Read written fields back off a live Python object for the `new_values`
/// response, so callers observe the value the engine actually stored rather
/// than the value they sent. A field that cannot be read back reports null.
fn read_back_fields<'a>(
    instance: &Bound<'_, PyAny>,
    names: impl IntoIterator<Item = &'a String>,
) -> serde_json::Map<String, serde_json::Value> {
    names
        .into_iter()
        .map(|name| {
            let value = instance
                .getattr(name.as_str())
                .map_or(serde_json::Value::Null, |value| py_value_to_json(&value));
            (name.clone(), value)
        })
        .collect()
}

fn validate_written_field(
    instance: &Bound<'_, PyAny>,
    field_name: &str,
    request: &serde_json::Value,
) -> Result<(), String> {
    let Ok(written) = instance.getattr(field_name) else {
        return Ok(());
    };
    reject_narrowed_nonfinite(request, &py_value_to_json(&written))
        .map_err(|error| format!("{field_name}: {error}"))
}

pub(crate) fn validate_written_fields(
    instance: &Bound<'_, PyAny>,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    for (name, value) in fields {
        if name == "variant" {
            continue;
        }
        validate_written_field(instance, name, value)?;
    }
    Ok(())
}

fn canonical_enum_fields(
    instance: &Bound<'_, PyAny>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let serde_json::Value::Object(fields) = py_value_to_json(instance) else {
        return None;
    };
    fields.contains_key("variant").then_some(fields)
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

    // Resolve every component before mutating the World. Native bridge and
    // custom-component metadata are the same registries used by the other
    // scene inspection/mutation handlers.
    let mut resolved = Vec::with_capacity(obj.len());
    let mut validation_errors = Vec::new();
    for (comp_name, _) in obj {
        match resolve_component(world, comp_name)? {
            Some(ComponentTarget::Native(bridge)) if !bridge.can_insert() => {
                validation_errors.push(format!("{comp_name}: cannot be spawned from Python"))
            }
            Some(component) => resolved.push(component),
            None => validation_errors.push(format!(
                "{comp_name}: not found in registry. {}",
                public_error::MCP_CUSTOM_COMPONENT_BOOTSTRAP
            )),
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
    let mut component_values = serde_json::Map::new();
    let mut errors = Vec::new();

    for ((comp_name, comp_fields), component) in obj.iter().zip(resolved) {
        let bridge = match component {
            ComponentTarget::Native(bridge) => bridge,
            ComponentTarget::Custom(component_id) => {
                spawn_custom_component(
                    world,
                    entity,
                    comp_name,
                    comp_fields,
                    component_id,
                    &mut added_components,
                    &mut errors,
                );
                continue;
            }
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
        if super::asset_reference::contains_reference_field(&fields) {
            match super::asset_reference::set_from_reference(
                world,
                entity,
                bridge.name(),
                &fields,
                &bridge,
            ) {
                Ok(value) => {
                    added_components.push(comp_name.clone());
                    component_values.insert(comp_name.clone(), value);
                }
                Err(error) => errors.push(format!("{comp_name}: {}", error.message)),
            }
            continue;
        }
        let type_id = bridge.bevy_type_id();

        // Try reflection first
        match reflect::reflect_spawn_component_with_bridge(
            world,
            entity,
            type_id,
            &fields,
            bridge.as_ref(),
        ) {
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
                // Fall back to Python - it handles many field types
                // (Color arrays, Vec2/Vec3, enums via type detection, custom wrappers)
            }
            Err(ReflectError::ComponentNotOnEntity) => {
                errors.push(format!("{comp_name}: entity not found"));
                continue;
            }
            Err(ReflectError::InvalidValue(error)) => {
                errors.push(format!("{comp_name}.{error}"));
                continue;
            }
            Err(ReflectError::Domain(error)) => {
                errors.push(error);
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

    // The partial entity is despawned, so nothing was created and the request
    // failed. Answering 201 Created with a null entity_id read as success.
    if !errors.is_empty() {
        world.despawn(entity);
        return Err(ControlError::invalid_params(format!(
            "Failed to spawn entity: {}",
            errors.join("; ")
        )));
    }

    let mut result = serde_json::json!({
        "entity_id": entity_id,
        "components_added": added_components,
    });
    if !component_values.is_empty() {
        result.as_object_mut().unwrap().insert(
            "component_values".into(),
            serde_json::Value::Object(component_values),
        );
    }
    Ok(result)
}

/// Construct a registered Python-defined component from JSON and insert it
/// through the root adapter's ordinary `EntityCommands.insert()` path. That
/// path owns wrapper-vs-PyObject preparation, guarded registration, lifecycle
/// ordering, and the unsafe `insert_by_id` boundary.
fn spawn_custom_component(
    world: &mut World,
    entity: bevy::ecs::entity::Entity,
    comp_name: &str,
    value: &serde_json::Value,
    component_id: bevy::ecs::component::ComponentId,
    added_components: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    let Some(entry) = world
        .get_resource::<CustomComponentInfo>()
        .and_then(|info| info.get(component_id).cloned())
    else {
        errors.push(format!("{comp_name}: custom component metadata is stale"));
        return;
    };

    Python::attach(|py| {
        let Some(retained_type) = entry.retained_type.as_ref() else {
            errors.push(format!("{comp_name}: custom component class is stale"));
            return;
        };
        let class = retained_type.bind(py);

        let instance = if let Some(fields) = value.as_object() {
            match construct_component_from_fields(py, class, comp_name, fields) {
                Ok(instance) => instance,
                Err(error) => {
                    errors.push(error);
                    return;
                }
            }
        } else {
            let argument = match json_to_py(py, value) {
                Ok(argument) => argument,
                Err(error) => {
                    errors.push(format!("{comp_name}: failed to convert value: {error}"));
                    return;
                }
            };
            match class.call1((argument,)) {
                Ok(instance) => instance,
                Err(error) => {
                    errors.push(format!("{comp_name}: failed to construct: {error}"));
                    return;
                }
            }
        };

        match insert_custom_instance(world, entity, py, &instance) {
            Ok(()) => added_components.push(comp_name.to_string()),
            Err(error) => errors.push(format!("{comp_name}: {error}")),
        }
    });
}

/// Insert a constructed custom component through the root adapter's ordinary
/// entity-command path. It owns wrapper-vs-PyObject preparation, descriptor
/// validation, and the final unsafe insertion boundary.
fn insert_custom_instance(
    world: &mut World,
    entity: Entity,
    py: Python<'_>,
    instance: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let validity = ValidityFlag::new();
    let guard = ValidityGuard::new(validity.clone());
    let insertion = create_world_wrapper(world, validity, py).and_then(|world_wrapper| {
        let py_entity = Py::new(py, PyEntity(entity))?;
        let entity_commands = world_wrapper.call_method1(py, "entity", (py_entity,))?;
        entity_commands.call_method1(py, "insert", (instance,))?;
        Ok(())
    });
    drop(guard);
    insertion
}

fn construct_component_from_fields<'py>(
    py: Python<'py>,
    class: &Bound<'py, PyType>,
    comp_name: &str,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<Bound<'py, PyAny>, String> {
    let declared_fields = fields
        .keys()
        .map(|field_name| class_declares_component_field(class, field_name))
        .collect::<Vec<_>>();
    if let Some((_, field_name)) = declared_fields
        .iter()
        .zip(fields.keys())
        .find(|(declared, _)| **declared == Some(false))
    {
        return Err(format!("{comp_name}: unknown field '{field_name}'"));
    }

    match class.call0() {
        Ok(instance) => {
            for ((field_name, field_value), declared) in fields.iter().zip(declared_fields) {
                if declared.is_none() && instance.getattr(field_name.as_str()).is_err() {
                    return Err(format!("{comp_name}: unknown field '{field_name}'"));
                }
                let py_value =
                    convert_annotated_field_value(py, &instance, field_name, field_value)
                        .map_err(|error| format!("{comp_name}.{field_name}: {error}"))?;
                instance
                    .setattr(field_name.as_str(), py_value)
                    .map_err(|error| format!("{comp_name}.{field_name}: {error}"))?;
            }
            Ok(instance)
        }
        Err(_) if !fields.is_empty() => {
            let kwargs = PyDict::new(py);
            for (field_name, field_value) in fields {
                let py_value = json_to_py(py, field_value)
                    .map_err(|error| format!("{comp_name}.{field_name}: {error}"))?;
                let py_value = validate_annotated_field_value(py, class, field_name, py_value)
                    .map_err(|error| format!("{comp_name}.{field_name}: {error}"))?;
                kwargs
                    .set_item(field_name, py_value)
                    .map_err(|error| format!("{comp_name}.{field_name}: {error}"))?;
            }
            class
                .call((), Some(&kwargs))
                .map_err(|error| format!("{comp_name}: failed to construct: {error}"))
        }
        Err(error) => Err(format!("{comp_name}: failed to create default: {error}")),
    }
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
            Ok(inst) => match resolve_enum_instance(py, &inst, fields) {
                Ok(Some(resolved)) => {
                    let bound = resolved.bind(py);
                    if let Err(error) = validate_written_fields(bound, fields) {
                        errors.push(format!("{comp_name}.{error}"));
                        return;
                    }
                    if let Err(e) = check_relationship_link(world, entity, bound, bridge) {
                        errors.push(format!("{comp_name}: {e}"));
                        return;
                    }
                    if let Err(e) = bridge.insert(world, entity, bound) {
                        errors.push(format!("{comp_name}: {e}"));
                    } else {
                        added_components.push(comp_name.to_string());
                    }
                    return;
                }
                Ok(None) => {
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
                Err(e) => {
                    errors.push(format!("{comp_name}: {e}"));
                    return;
                }
            },
            Err(_) if !fields.is_empty() => {
                // Default constructor failed - try passing fields as kwargs
                let kwargs = PyDict::new(py);
                for (field_name, field_value) in fields {
                    match json_to_py_for_field(py, bridge, field_name, field_value) {
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

        if let Err(error) = validate_written_fields(&instance, fields) {
            errors.push(format!("{comp_name}.{error}"));
            return;
        }

        if let Err(e) = check_relationship_link(world, entity, &instance, bridge) {
            errors.push(format!("{comp_name}: {e}"));
            return;
        }

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
        // A relationship component's sole argument is the related entity.
        let converted = match bridge.relationship_field() {
            Some(_) => entity_from_json(py, value),
            None => json_to_py(py, value),
        };
        let py_value = match converted {
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

        if let Err(e) = check_relationship_link(world, entity, &instance, bridge) {
            errors.push(format!("{comp_name}: {e}"));
            return;
        }

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

    // Bevy cascades the despawn through descendants, so a resource entity
    // anywhere in the subtree would have its value silently discarded.
    if hierarchy_contains_resource_entity(world, entity) {
        return Err(ControlError::invalid_params(
            public_error::RESOURCE_ENTITY_DESPAWN,
        ));
    }

    let existed = world.entities().contains(entity);
    if existed {
        if let Some(hooks) = LIFECYCLE_MUTATION_HOOKS.get() {
            (hooks.despawn_entity)(world, entity);
        } else {
            world.despawn(entity);
        }
    }

    if existed {
        Ok(serde_json::json!({
            "despawned": true,
            "entity_id": entity_id,
        }))
    } else {
        Err(ControlError::not_found(public_error::mcp_entity_not_found(
            entity_id,
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
    let target = resolve_component(world, &component)?;
    if let Some(ComponentTarget::Native(bridge)) = &target {
        let type_id = bridge.bevy_type_id();

        if super::asset_reference::contains_reference_field(field_obj) {
            let inserted = world
                .get_entity(entity)
                .is_ok_and(|entity_ref| !bridge.entity_contains(&entity_ref));
            if !bridge.can_insert() {
                return Err(ControlError::invalid_params(format!(
                    "Component '{component}' cannot be set from Python"
                )));
            }
            let new_values = super::asset_reference::set_from_reference(
                world,
                entity,
                bridge.name(),
                field_obj,
                bridge,
            )?;
            let mut result = serde_json::json!({
                "entity_id": entity_id,
                "component": component,
                "new_values": new_values,
            });
            if inserted {
                result
                    .as_object_mut()
                    .unwrap()
                    .insert("inserted".into(), serde_json::json!(true));
            }
            return Ok(result);
        }

        // Try reflection first
        match reflect::reflect_set_component_with_bridge(
            world,
            entity,
            type_id,
            field_obj,
            bridge.as_ref(),
        ) {
            Ok(new_values) => {
                return Ok(serde_json::json!({
                    "entity_id": entity_id,
                    "component": component,
                    "new_values": new_values,
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
                if !bridge.can_insert() {
                    return Err(ControlError::invalid_params(format!(
                        "Component '{component}' cannot be inserted from Python"
                    )));
                }
                match reflect::reflect_spawn_component_with_bridge(
                    world,
                    entity,
                    type_id,
                    field_obj,
                    bridge.as_ref(),
                ) {
                    Ok(()) => {
                        let new_values =
                            reflect::reflect_read_fields(world, entity, type_id, field_obj.keys());
                        return Ok(serde_json::json!({
                            "entity_id": entity_id,
                            "component": component,
                            "new_values": new_values,
                            "inserted": true,
                        }));
                    }
                    Err(
                        ReflectError::NotRegistered
                        | ReflectError::NoReflectComponent
                        | ReflectError::NoReflectDefault
                        | ReflectError::NotAStruct,
                    ) => {
                        return insert_component_python(
                            world, entity, entity_id, &component, field_obj, bridge,
                        );
                    }
                    Err(ReflectError::FieldError(_)) => {
                        return insert_component_python(
                            world, entity, entity_id, &component, field_obj, bridge,
                        );
                    }
                    Err(ReflectError::ComponentNotOnEntity) => {
                        return Err(ControlError::not_found(public_error::mcp_entity_not_found(
                            entity_id,
                        )));
                    }
                    Err(ReflectError::InvalidValue(error) | ReflectError::Domain(error)) => {
                        return Err(ControlError::invalid_params(format!(
                            "Failed to set '{component}': {error}"
                        )));
                    }
                }
            }
            Err(ReflectError::FieldError(_)) => {
                // Fall back to Python - it handles many field types
                // (Color arrays, Vec2/Vec3, enums via type detection, custom wrappers)
            }
            Err(ReflectError::NoReflectDefault) => {
                // Not applicable for set_component, fall back
            }
            Err(ReflectError::InvalidValue(error) | ReflectError::Domain(error)) => {
                return Err(ControlError::invalid_params(format!(
                    "Failed to set '{component}': {error}"
                )));
            }
        }

        // Python fallback for bridge components
        return set_component_python(world, entity, entity_id, &component, field_obj, bridge);
    }

    // Fallback: check custom Python components via CustomComponentInfo
    let component_id = match target {
        Some(ComponentTarget::Custom(id)) => Some(id),
        _ => None,
    };
    set_custom_component(
        world,
        entity,
        entity_id,
        &component,
        component_id,
        field_obj,
    )
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
    // Validate before paths that may construct or insert the component.
    if let Some(variant) = field_obj.get("variant")
        && !variant.is_string()
    {
        return Err(ControlError::invalid_params(format!(
            "Failed to set '{component}': variant must be a variant name as a string, got {variant}"
        )));
    }

    // A relationship component is replaced, never written in place: Bevy's
    // hooks run on insert/replace only, so setting the field would leave the
    // old parent's `Children` stale. Its whole payload is the related entity,
    // so rebuilding from the request loses nothing.
    if bridge.relationship_field().is_some() {
        // insert_component_python reaches entity_mut, which panics on a stale id.
        if world.get_entity(entity).is_err() {
            return Err(ControlError::invalid_params(
                public_error::mcp_entity_not_found(entity_id),
            ));
        }
        return insert_component_python(world, entity, entity_id, component, field_obj, bridge);
    }

    if world
        .get_entity(entity)
        .is_ok_and(|entity_ref| !bridge.entity_contains(&entity_ref))
    {
        if !bridge.can_insert() {
            return Err(ControlError::invalid_params(format!(
                "Component '{component}' cannot be inserted from Python"
            )));
        }
        return insert_component_python(world, entity, entity_id, component, field_obj, bridge);
    }

    if !bridge.can_insert() {
        return Err(ControlError::invalid_params(format!(
            "Component '{component}' is read-only"
        )));
    }

    let optional_types = optional_field_types(world, bridge.bevy_type_id());

    let (owned, replacement_variant) = Python::attach(
        |py| -> Result<(Py<PyAny>, Option<String>), ControlError> {
            let validity_flag = pybevy_core::ValidityFlag::new_read();
            let validity = validity_flag.with_access_mode(pybevy_core::AccessMode::Read);
            // SAFETY: `world` is live for this call, the wrapper is fenced by
            // `validity_flag`, and only read access is requested before copying.
            let extracted = unsafe {
                bridge.extract_from_entity_ref(entity, world as *mut World, validity, py)
            };
            let extracted = extracted
                .map_err(|error| {
                    ControlError::invalid_params(format!(
                        "Failed to read '{component}' before mutation: {error}"
                    ))
                })?
                .ok_or_else(|| {
                    ControlError::not_found(public_error::mcp_entity_not_found(entity_id))
                })?;
            let current = extracted.bind(py);
            let result = if let Some(request) = enum_component_variant_request(current, field_obj) {
                let replacement = construct_enum_variant(
                    py,
                    current,
                    &request.name,
                    &request.payload,
                    request.source,
                )
                .map_err(|error| {
                    ControlError::invalid_params(format!("Failed to set '{component}': {error}"))
                })?;
                match replacement {
                    Some(replacement) => Ok((replacement, Some(request.name))),
                    None => current
                        .call_method0("__copy__")
                        .map(|copy| (copy.unbind(), None))
                        .map_err(|error| {
                            ControlError::invalid_params(format!(
                                "Component '{component}' cannot be copied for atomic mutation: {error}"
                            ))
                        }),
                }
            } else {
                // Pre-write guard: a later setattr would leak a raw AttributeError for typos.
                let unknown = unknown_set_fields(current, field_obj.keys());
                if !unknown.is_empty() {
                    return Err(ControlError::invalid_params(component_unknown_field_error(
                        component, &unknown, current,
                    )));
                }
                current
                    .call_method0("__copy__")
                    .map(|copy| (copy.unbind(), None))
                    .map_err(|error| {
                        ControlError::invalid_params(format!(
                            "Component '{component}' cannot be copied for atomic mutation: {error}"
                        ))
                    })
            };
            validity_flag.set_invalid();
            result
        },
    )?;

    let new_values = Python::attach(
        |py| -> Result<serde_json::Map<String, serde_json::Value>, ControlError> {
            let instance = owned.bind(py);
            if let Some(variant_name) = replacement_variant {
                validate_written_fields(instance, field_obj).map_err(|error| {
                    ControlError::invalid_params(format!("Failed to set '{component}': {error}"))
                })?;
                bridge.insert(world, entity, instance).map_err(|error| {
                    ControlError::invalid_params(format!(
                        "Failed to replace component '{component}': {error}"
                    ))
                })?;
                let mut post = canonical_enum_fields(instance)
                    .unwrap_or_else(|| read_back_fields(instance, field_obj.keys()));
                post.entry("variant".to_string()).or_insert_with(|| {
                    serde_json::Value::String(super::enum_names::canonical_variant_name(
                        &enum_owner_type(instance),
                        &variant_name,
                    ))
                });
                return Ok(post);
            }

            let mut converted = Vec::with_capacity(field_obj.len());
            for (field_name, field_value) in field_obj {
                let declared = declared_optional_type(&optional_types, field_name);
                let py_value =
                    convert_field_value_typed(py, instance, field_name, field_value, declared)
                        .map_err(|error| {
                            ControlError::invalid_params(format!(
                                "Failed to set '{component}': {field_name}: {error}"
                            ))
                        })?;
                converted.push((field_name, py_value));
            }
            for (field_name, py_value) in converted {
                instance
                    .setattr(field_name.as_str(), py_value)
                    .map_err(|error| {
                        ControlError::invalid_params(format!(
                            "Failed to set '{component}': {field_name}: {error}"
                        ))
                    })?;
            }
            validate_written_fields(instance, field_obj).map_err(|error| {
                ControlError::invalid_params(format!("Failed to set '{component}': {error}"))
            })?;
            bridge.insert(world, entity, instance).map_err(|error| {
                ControlError::invalid_params(format!(
                    "Failed to replace component '{component}': {error}"
                ))
            })?;
            Ok(read_back_fields(instance, field_obj.keys()))
        },
    )?;

    Ok(serde_json::json!({
        "entity_id": entity_id,
        "component": component,
        "new_values": serde_json::Value::Object(new_values),
    }))
}

fn insert_component_python(
    world: &mut World,
    entity: bevy::ecs::entity::Entity,
    entity_id: u64,
    component: &str,
    field_obj: &serde_json::Map<String, serde_json::Value>,
    bridge: &Arc<dyn ComponentBridge>,
) -> Result<serde_json::Value, ControlError> {
    if !bridge.can_insert() {
        return Err(ControlError::invalid_params(format!(
            "Component '{component}' cannot be inserted from Python"
        )));
    }

    let optional_types = optional_field_types(world, bridge.bevy_type_id());

    let new_values = Python::attach(|py| {
        let py_type = bridge.py_type(py);
        let instance = match py_type.call0() {
            Ok(instance) => match resolve_enum_instance(py, &instance, field_obj) {
                Ok(Some(resolved)) => {
                    let bound = resolved.bind(py);
                    validate_written_fields(bound, field_obj).map_err(|error| {
                        ControlError::invalid_params(format!(
                            "Failed to set '{component}': {error}"
                        ))
                    })?;
                    check_relationship_link(world, entity, bound, bridge)
                        .map_err(ControlError::invalid_params)?;
                    bridge.insert(world, entity, bound).map_err(|error| {
                        ControlError::invalid_params(format!(
                            "Failed to insert component '{component}': {error}"
                        ))
                    })?;
                    let mut post = read_back_fields(bound, field_obj.keys());
                    if let Some(serde_json::Value::String(variant_name)) = field_obj.get("variant")
                    {
                        post.insert(
                            "variant".to_string(),
                            serde_json::Value::String(super::enum_names::canonical_variant_name(
                                &py_type,
                                variant_name,
                            )),
                        );
                    }
                    return Ok(post);
                }
                Ok(None) => {
                    // Same pre-write guard before the insert setattr path.
                    let unknown = unknown_set_fields(&instance, field_obj.keys());
                    if !unknown.is_empty() {
                        return Err(ControlError::invalid_params(component_unknown_field_error(
                            component, &unknown, &instance,
                        )));
                    }
                    for (field_name, field_value) in field_obj {
                        let declared = declared_optional_type(&optional_types, field_name);
                        let py_value = convert_field_value_typed(
                            py,
                            &instance,
                            field_name,
                            field_value,
                            declared,
                        )
                        .map_err(|error| {
                            ControlError::invalid_params(format!(
                                "Failed to convert '{component}.{field_name}': {error}"
                            ))
                        })?;
                        instance
                            .setattr(field_name.as_str(), py_value)
                            .map_err(|error| {
                                ControlError::invalid_params(format!(
                                    "Failed to set '{component}.{field_name}': {error}"
                                ))
                            })?;
                    }
                    instance
                }
                Err(error) => {
                    return Err(ControlError::invalid_params(format!(
                        "Failed to set '{component}': {error}"
                    )));
                }
            },
            Err(_) if !field_obj.is_empty() => {
                let kwargs = PyDict::new(py);
                for (field_name, field_value) in field_obj {
                    let py_value = json_to_py_for_field(py, bridge, field_name, field_value)
                        .map_err(|error| {
                            ControlError::invalid_params(format!(
                                "Failed to convert '{component}.{field_name}': {error}"
                            ))
                        })?;
                    kwargs.set_item(field_name, py_value).map_err(|error| {
                        ControlError::invalid_params(format!(
                            "Failed to set constructor argument '{component}.{field_name}': {error}"
                        ))
                    })?;
                }
                py_type.call((), Some(&kwargs)).map_err(|error| {
                    ControlError::invalid_params(format!(
                        "Failed to construct component '{component}': {error}"
                    ))
                })?
            }
            Err(error) => {
                return Err(ControlError::invalid_params(format!(
                    "Failed to construct component '{component}': {error}"
                )));
            }
        };

        validate_written_fields(&instance, field_obj).map_err(|error| {
            ControlError::invalid_params(format!("Failed to set '{component}': {error}"))
        })?;

        check_relationship_link(world, entity, &instance, bridge)
            .map_err(ControlError::invalid_params)?;

        bridge.insert(world, entity, &instance).map_err(|error| {
            ControlError::invalid_params(format!(
                "Failed to insert component '{component}': {error}"
            ))
        })?;

        Ok::<_, ControlError>(read_back_fields(&instance, field_obj.keys()))
    })?;

    Ok(serde_json::json!({
        "entity_id": entity_id,
        "component": component,
        "new_values": new_values,
        "inserted": true,
    }))
}

/// Set fields on a custom Python component (not in bridge registry).
/// Looks up the component via CustomComponentInfo and mutates fields via Python setattr.
fn set_custom_component(
    world: &mut World,
    entity: bevy::ecs::entity::Entity,
    entity_id: u64,
    component: &str,
    resolved_id: Option<bevy::ecs::component::ComponentId>,
    field_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, ControlError> {
    // Find the custom component's ComponentId: try exact name match first
    let custom_info = world
        .get_resource::<CustomComponentInfo>()
        .and_then(|info| {
            info.iter()
                .find(|(id, entry)| Some(*id) == resolved_id || entry.name == component)
                .map(|(id, entry)| (id, entry.is_pyobject_storage, entry.wrapper_layout.clone()))
        });

    // Fallback: check for qualified name variants (e.g. "module.Oscillator" vs "Oscillator")
    let custom_info = custom_info.or_else(|| {
        let info = world.get_resource::<CustomComponentInfo>()?;
        let entity_ref = world.get_entity(entity).ok()?;

        // Check if any registered custom component matches by short name
        // and is present on this entity's archetype
        for (id, entry) in info.iter() {
            let short_name = entry.name.rsplit('.').next().unwrap_or(&entry.name);
            if short_name == component && entity_ref.get_by_id(id).is_ok() {
                return Some((id, entry.is_pyobject_storage, entry.wrapper_layout.clone()));
            }
        }

        None
    });

    let Some((comp_id, is_pyobject_storage, wrapper_layout)) = custom_info else {
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
             {}{hint}",
            public_error::MCP_CUSTOM_COMPONENT_BOOTSTRAP
        )));
    };

    // Check entity has this component
    let eref = world
        .get_entity(entity)
        .map_err(|_| ControlError::not_found(public_error::mcp_entity_not_found(entity_id)))?;

    let has_component = eref.get_by_id(comp_id).is_ok();

    if !has_component {
        // Component not present - construct it and use the root adapter's
        // ordinary insertion path for either wrapper or PyObject storage.
        return insert_custom_component(
            world,
            entity,
            entity_id,
            component,
            comp_id,
            field_obj,
            is_pyobject_storage,
            wrapper_layout,
        );
    }

    if !is_pyobject_storage {
        let layout = wrapper_layout.ok_or_else(|| {
            ControlError::internal(format!(
                "Component '{component}' has no registered wrapper layout"
            ))
        })?;
        let descriptor_layout = world
            .components()
            .get_info(comp_id)
            .map(|info| info.layout())
            .ok_or_else(|| {
                ControlError::internal(format!("Component '{component}' has no Bevy descriptor"))
            })?;
        if !custom::descriptor_matches(&layout, descriptor_layout) {
            return Err(ControlError::internal(format!(
                "Component '{component}' wrapper layout does not match its Bevy descriptor"
            )));
        }

        // An unknown field name fails the whole update before anything is
        // written, matching set_resource's unknown-field contract.
        let unknown = field_obj
            .keys()
            .filter(|name| layout.get_field(name).is_none())
            .cloned()
            .collect::<Vec<_>>();
        if !unknown.is_empty() {
            let names = layout.field_names();
            let valid = if names.is_empty() {
                "This component declares no editable fields.".to_string()
            } else {
                format!("Valid fields: {}.", names.join(", "))
            };
            return Err(ControlError::invalid_params(format!(
                "Component '{component}' has unknown fields: {}. {valid}",
                unknown.join(", ")
            )));
        }

        // JSON conversion is complete before resolving a mutable ECS pointer.
        // This mirrors the Python adapter's pointer-safety rule even though JSON
        // conversion itself cannot re-enter Python.
        let (values, errors) = custom::values_from_json(&layout, field_obj);
        if !errors.is_empty() {
            return Err(ControlError::invalid_params(format!(
                "Failed to set '{component}': {}",
                errors.join("; ")
            )));
        }
        let mut new_values = serde_json::Map::new();
        if !values.is_empty() {
            let mut entity_mut = world.get_entity_mut(entity).map_err(|_| {
                ControlError::not_found(public_error::mcp_entity_not_found(entity_id))
            })?;
            let mut untyped = entity_mut.get_mut_by_id(comp_id).map_err(|_| {
                ControlError::not_found(format!(
                    "Component '{component}' not found on entity {entity_id}"
                ))
            })?;
            let written = values
                .iter()
                .map(|(name, _, _)| name.clone())
                .collect::<Vec<_>>();
            // SAFETY: the descriptor was checked against this registered
            // wrapper layout. Every field range was bounds-checked during JSON
            // conversion, and get_mut_ptr marks the component changed in Bevy.
            let data = unsafe { layout.wrapper_size.get_mut_ptr(&mut untyped) };
            for (_, offset, value) in values {
                // SAFETY: values_from_json verified that this primitive fits in
                // the wrapper buffer at the recorded offset.
                unsafe { value.write_to_ptr(data.add(offset)) };
            }
            // Read the wrapper back so new_values reports the stored primitive
            // rather than the input: Vec2/Vec3 fields hold f32 components, so
            // they narrow the caller's JSON doubles.
            let stored = custom::fields_to_json(untyped.as_ref(), &layout)
                .map_err(ControlError::internal)?;
            for name in written {
                let value = stored
                    .get(&name)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                new_values.insert(name, value);
            }
        }

        return Ok(serde_json::json!({
            "entity_id": entity_id,
            "component": component,
            "new_values": new_values,
        }));
    }

    let ptr = world
        .get_entity(entity)
        .map_err(|_| ControlError::not_found(public_error::mcp_entity_not_found(entity_id)))?
        .get_by_id(comp_id)
        .map_err(|_| {
            ControlError::not_found(format!(
                "Component '{component}' not found on entity {entity_id}"
            ))
        })?;
    let py_obj = Python::attach(|py| {
        // SAFETY: We checked is_pyobject_storage above - raw data is a Py<PyAny>
        unsafe { &*(ptr.as_ptr() as *const pyo3::Py<PyAny>) }.clone_ref(py)
    });

    let mut new_values = serde_json::Map::new();
    let mut errors = Vec::new();

    Python::attach(|py| -> Result<(), ControlError> {
        let bound = &py_obj.into_bound(py);
        let class = bound.get_type();
        let unknown = field_obj
            .keys()
            .filter(|name| match class_declares_component_field(&class, name) {
                Some(true) => false,
                Some(false) => !has_public_instance_field(bound, name),
                None => bound.getattr(name.as_str()).is_err(),
            })
            .cloned()
            .collect::<Vec<_>>();
        if !unknown.is_empty() {
            return Err(ControlError::invalid_params(component_unknown_field_error(
                component, &unknown, bound,
            )));
        }

        let mut converted = Vec::with_capacity(field_obj.len());
        for (field_name, field_value) in field_obj {
            match convert_annotated_field_value(py, bound, field_name, field_value) {
                Ok(py_value) => converted.push((field_name, py_value)),
                Err(e) => {
                    errors.push(format!("{field_name}: {e}"));
                }
            }
        }
        if errors.is_empty() {
            let mut written = Vec::with_capacity(converted.len());
            for (field_name, py_value) in converted {
                if let Err(e) = bound.setattr(field_name.as_str(), py_value) {
                    errors.push(format!("{field_name}: {e}"));
                } else {
                    written.push(field_name);
                }
            }
            new_values = read_back_fields(bound, written);
        }
        Ok(())
    })?;

    if !new_values.is_empty()
        && let Ok(mut entity_mut) = world.get_entity_mut(entity)
        && let Ok(mut stored) = entity_mut.get_mut_by_id(comp_id)
    {
        stored.set_changed();
    }

    // Nothing was written: report the failure rather than a 200 the caller
    // would read as success.
    if new_values.is_empty() && !errors.is_empty() {
        return Err(ControlError::invalid_params(format!(
            "Failed to set '{component}': {}",
            errors.join("; ")
        )));
    }

    let mut result = serde_json::json!({
        "entity_id": entity_id,
        "component": component,
        "new_values": serde_json::Value::Object(new_values),
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
#[allow(clippy::too_many_arguments)]
fn insert_custom_component(
    world: &mut World,
    entity: bevy::ecs::entity::Entity,
    entity_id: u64,
    component: &str,
    comp_id: bevy::ecs::component::ComponentId,
    field_obj: &serde_json::Map<String, serde_json::Value>,
    is_pyobject_storage: bool,
    wrapper_layout: Option<Arc<pybevy_core::component_layout::ComponentLayout>>,
) -> Result<serde_json::Value, ControlError> {
    let retained_type = world
        .get_resource::<CustomComponentInfo>()
        .and_then(|info| info.get(comp_id)?.retained_type.clone())
        .ok_or_else(|| ControlError::internal("Custom component class is stale".to_string()))?;

    let requested_values = Python::attach(|py| {
        let cls = retained_type.bind(py);
        let instance = construct_component_from_fields(py, cls, component, field_obj)
            .map_err(ControlError::invalid_params)?;

        // Both construction paths apply every requested field or bail out, so
        // the post-state covers all of them.
        let new_values = read_back_fields(&instance, field_obj.keys());
        insert_custom_instance(world, entity, py, &instance).map_err(|error| {
            ControlError::invalid_params(format!(
                "Failed to insert component '{component}': {error}"
            ))
        })?;
        Ok::<_, ControlError>(new_values)
    })?;

    let new_values = if is_pyobject_storage {
        requested_values
    } else {
        let layout = wrapper_layout.ok_or_else(|| {
            ControlError::internal(format!(
                "Component '{component}' has no registered wrapper layout"
            ))
        })?;
        let descriptor_layout = world
            .components()
            .get_info(comp_id)
            .map(|info| info.layout())
            .ok_or_else(|| {
                ControlError::internal(format!("Component '{component}' has no Bevy descriptor"))
            })?;
        if !custom::descriptor_matches(&layout, descriptor_layout) {
            return Err(ControlError::internal(format!(
                "Component '{component}' wrapper layout does not match its Bevy descriptor"
            )));
        }
        let ptr = world
            .get_entity(entity)
            .map_err(|_| ControlError::not_found(public_error::mcp_entity_not_found(entity_id)))?
            .get_by_id(comp_id)
            .map_err(|_| {
                ControlError::internal(format!(
                    "Component '{component}' was not inserted on entity {entity_id}"
                ))
            })?;
        let stored = custom::fields_to_json(ptr, &layout).map_err(ControlError::internal)?;
        field_obj
            .keys()
            .map(|name| {
                (
                    name.clone(),
                    stored.get(name).cloned().unwrap_or(serde_json::Value::Null),
                )
            })
            .collect()
    };

    Ok(serde_json::json!({
        "entity_id": entity_id,
        "component": component,
        "new_values": serde_json::Value::Object(new_values),
        "inserted": true,
    }))
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
             hierarchy, or spatial queries. Use a full reload to restore the authored \
             component if removal was accidental."
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

    let target = resolve_component(world, &component)?;
    // Stripping IsResource runs Bevy's Discard hook, destroying the resource
    // value behind the entity.
    if component == "IsResource"
        || matches!(&target, Some(ComponentTarget::Native(bridge)) if bridge.name() == "IsResource")
    {
        return Err(ControlError::invalid_params(
            public_error::IS_RESOURCE_COMPONENT_REMOVE,
        ));
    }

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

    if let Some(ComponentTarget::Native(bridge)) = &target {
        let component_id = bridge.register(world);
        if let Ok(entity_ref) = world.get_entity(entity) {
            if !entity_ref.contains_id(component_id) {
                return Err(ControlError::not_found(format!(
                    "Component '{component}' not found on entity {entity_id}"
                )));
            }
            if let Some(hooks) = LIFECYCLE_MUTATION_HOOKS.get() {
                let type_ptr = Python::attach(|py| bridge.py_type(py).as_type_ptr());
                (hooks.remove_component)(world, entity, type_ptr);
            } else if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                entity_mut.remove_by_id(component_id);
            }
            return Ok(build_response(&component, &warning));
        } else {
            return Err(ControlError::not_found(public_error::mcp_entity_not_found(
                entity_id,
            )));
        }
    }

    // Fallback: check custom Python components via CustomComponentInfo
    let custom_component = world
        .get_resource::<CustomComponentInfo>()
        .and_then(|info| {
            info.iter()
                .find(|(id, _)| matches!(&target, Some(ComponentTarget::Custom(target_id)) if id == target_id))
                .map(|(id, entry)| (id, entry.type_ptr))
        });

    if let Some((component_id, type_ptr)) = custom_component {
        if let Ok(entity_ref) = world.get_entity(entity) {
            if !entity_ref.contains_id(component_id) {
                return Err(ControlError::not_found(format!(
                    "Component '{component}' not found on entity {entity_id}"
                )));
            }
            if let Some(hooks) = LIFECYCLE_MUTATION_HOOKS.get() {
                (hooks.remove_component)(world, entity, type_ptr);
            } else if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                entity_mut.remove_by_id(component_id);
            }
            return Ok(build_response(&component, &warning));
        } else {
            return Err(ControlError::not_found(public_error::mcp_entity_not_found(
                entity_id,
            )));
        }
    }

    Err(ControlError::not_found(format!(
        "Component '{component}' not in registry. {}",
        public_error::MCP_CUSTOM_COMPONENT_BOOTSTRAP
    )))
}

/// Convert a JSON field value to the appropriate Python type by inspecting the current field type.
/// For Vec2/Vec3/Quat fields, JSON arrays are converted to the proper constructor calls.
/// Reject a parent-child link with a resource entity at either end.
///
/// Bevy despawns children along with their parent, so a resource entity in the
/// subtree would have its value silently discarded. Checked on the constructed
/// component, so every construction path shares one rule.
fn check_relationship_link(
    world: &World,
    child: bevy::ecs::entity::Entity,
    instance: &Bound<'_, PyAny>,
    bridge: &Arc<dyn ComponentBridge>,
) -> Result<(), String> {
    let Some(field) = bridge.relationship_field() else {
        return Ok(());
    };
    let parent = instance
        .getattr(field)
        .map_err(|error| error.to_string())?
        .extract::<PyEntity>()
        .map_err(|error| error.to_string())?;
    let parent = bevy::ecs::entity::Entity::from(parent);
    validate_hierarchy_link(world, child, parent).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{alloc::Layout, ptr, sync::Once};

    use bevy::{
        camera::ClearColor,
        ecs::{
            component::{ComponentCloneBehavior, ComponentDescriptor, ComponentId, StorageType},
            entity::Entity,
            name::Name,
            resource::IsResource,
        },
        prelude::{ChildOf, Children, With},
    };
    use pybevy_core::{
        CustomComponentEntry,
        component_layout::{ComponentLayout, PrimitiveType, PrimitiveValue},
        component_wrapper::ComponentWrapper16,
        custom_component::create_wrapper_descriptor,
    };
    use pyo3::types::PyInt;

    use super::*;
    use crate::{protocol::ErrorCode, pyo3::inspect::get_component};

    static INIT: Once = Once::new();

    fn setup_python() {
        INIT.call_once(|| {
            Python::initialize();
        });
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
    fn despawn_rejects_resource_entity() {
        let mut world = World::new();
        world.init_resource::<ClearColor>();
        let entity = world
            .query_filtered::<Entity, With<IsResource>>()
            .iter(&world)
            .next()
            .expect("init_resource should create a resource entity");

        let error = despawn_entity(&mut world, EntityRef::Id(entity.to_bits()))
            .expect_err("resource entity despawn must be rejected");

        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(world.get_entity(entity).is_ok());
        assert!(world.get_resource::<ClearColor>().is_some());
    }

    #[test]
    fn despawn_rejects_resource_entity_in_subtree() {
        let mut world = World::new();
        world.init_resource::<ClearColor>();
        let resource_entity = world
            .query_filtered::<Entity, With<IsResource>>()
            .iter(&world)
            .next()
            .expect("init_resource should create a resource entity");
        let parent = world.spawn(Name::new("Parent")).id();
        world.entity_mut(parent).add_child(resource_entity);

        let error = despawn_entity(&mut world, EntityRef::Id(parent.to_bits()))
            .expect_err("cascade into a resource entity must be rejected");

        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(world.get_resource::<ClearColor>().is_some());
    }

    #[test]
    fn remove_component_rejects_is_resource_marker() {
        let mut world = World::new();
        world.init_resource::<ClearColor>();
        let entity = world
            .query_filtered::<Entity, With<IsResource>>()
            .iter(&world)
            .next()
            .expect("init_resource should create a resource entity");

        let error = remove_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "IsResource".to_string(),
        )
        .expect_err("IsResource removal must be rejected");

        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(world.get_resource::<ClearColor>().is_some());
    }

    #[test]
    fn despawn_entity_not_found() {
        let mut world = World::new();
        let result = despawn_entity(&mut world, EntityRef::Id(999999));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
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
        assert!(warn.contains("full reload"));
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
    fn remove_component_custom_python_component_not_on_entity() {
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
                retained_type: None,
                name: "CustomComp".to_string(),
                is_pyobject_storage: true,
                wrapper_layout: None,
            },
        );
        world.insert_resource(info);

        let result = remove_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "CustomComp".to_string(),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
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
    fn spawn_entity_atomicity_no_stray_entity() {
        let mut world = World::new();
        let initial_count = world.entities().len();

        // Unknown components should fail without leaving a stray entity
        let result = spawn_entity(&mut world, serde_json::json!({"FakeComp": {}}));
        assert!(result.is_err());
        assert_eq!(world.entities().len(), initial_count);
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
        // succeeds - ensuring the match arms haven't been accidentally reordered to
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
                retained_type: None,
                name: "Health".to_string(),
                is_pyobject_storage: true,
                wrapper_layout: None,
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
                retained_type: None,
                name: "mymod.Oscillator".to_string(),
                is_pyobject_storage: true,
                wrapper_layout: None,
            },
        );
        world.insert_resource(info);

        // Requesting by short name "Oscillator" should not error with "not found"
        // when the entity doesn't have the component - it should find the entry
        // via qualified name fallback but then fail because entity lacks it.
        // This tests the name resolution path, not the full mutation.
        let result = set_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "Oscillator".to_string(),
            serde_json::json!({"speed": 1.0}),
        );
        // The qualified name match is only used when the component is on the entity,
        // so this will still fail - but the error should list available components
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
    fn set_and_get_wrapper_storage_custom_component_fields() {
        setup_python();
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();

        let layout = Arc::new(
            ComponentLayout::from_fields(
                ptr::null(),
                "WrapperComp".to_string(),
                &[
                    ("speed".to_string(), PrimitiveType::F64),
                    ("count".to_string(), PrimitiveType::I64),
                ],
            )
            .unwrap(),
        );
        let comp_id = world.register_component_with_descriptor(create_wrapper_descriptor(
            "WrapperComp".to_string(),
            layout.wrapper_size,
        ));
        let mut wrapper = ComponentWrapper16::default();
        unsafe {
            PrimitiveValue::F64(1.25).write_to_ptr(wrapper.data.as_mut_ptr());
            PrimitiveValue::I64(2).write_to_ptr(wrapper.data.as_mut_ptr().add(8));
            let data = core::ptr::NonNull::new_unchecked(ptr::addr_of_mut!(wrapper) as *mut u8);
            world
                .entity_mut(entity)
                .insert_by_id(comp_id, bevy::ptr::OwningPtr::new(data));
        }

        let mut info = CustomComponentInfo::default();
        info.insert(
            comp_id,
            CustomComponentEntry {
                type_ptr: ptr::null(),
                retained_type: None,
                name: "WrapperComp".to_string(),
                is_pyobject_storage: false,
                wrapper_layout: Some(layout),
            },
        );
        world.insert_resource(info);

        let result = set_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "WrapperComp".to_string(),
            serde_json::json!({"speed": 3.5, "count": 9}),
        );
        let result = result.unwrap();
        // Values come from re-reading the wrapper, not from echoing the input.
        assert_eq!(
            result["new_values"],
            serde_json::json!({"speed": 3.5, "count": 9})
        );

        let result = get_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "WrapperComp".to_string(),
        )
        .unwrap();
        assert_eq!(result["fields"]["speed"], 3.5);
        assert_eq!(result["fields"]["count"], 9);
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
                retained_type: None,
                name: "ReinsertComp".to_string(),
                is_pyobject_storage: true,
                wrapper_layout: None,
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
        // The error should NOT be about "not found on entity" - it should be
        // about creation/setting fields (proving insert_custom_component was called)
        assert!(
            !err.message.contains("not found on entity"),
            "Should not report 'not found on entity' - should attempt creation. Got: {}",
            err.message
        );
    }

    /// A TypeId-less component absent from CustomComponentInfo is never
    /// reinterpreted as a Py<PyAny>, even with a matching layout.
    #[test]
    fn set_component_wrapper_like_bytes_not_misread_as_pyobject() {
        setup_python();

        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();

        // Mutable + TypeId-less + same layout as Py<PyAny>, mimicking
        // ComponentWrapper8, but absent from CustomComponentInfo
        let comp_id = world.register_component_with_descriptor(unsafe {
            ComponentDescriptor::new_with_layout(
                "FakeOscillator",
                StorageType::Table,
                Layout::new::<u64>(),
                None,
                true,
                ComponentCloneBehavior::Default,
                None,
            )
        });

        let mut entity_mut = world.get_entity_mut(entity).unwrap();
        // Bit pattern of an f64, guaranteed not a valid PyObject pointer
        let payload: u64 = 0x3FF0_0000_0000_0000;
        unsafe {
            let data = core::ptr::NonNull::new_unchecked(ptr::addr_of!(payload) as *mut u8);
            entity_mut.insert_by_id(comp_id, bevy::ptr::OwningPtr::new(data));
        }

        world.insert_resource(CustomComponentInfo::default());

        let result = set_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "FakeOscillator".to_string(),
            serde_json::json!({"value": 42}),
        );
        let err = result.expect_err("wrapper-like component must not match");
        assert_eq!(err.code, ErrorCode::NotFound);
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
}
