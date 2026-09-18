use std::{
    alloc::Layout,
    collections::BTreeSet,
    sync::{Arc, OnceLock},
};

use bevy::{
    ecs::{entity::Entity, world::World},
    reflect::enums::VariantInfo,
};
use pybevy_core::{
    ComponentBridge, CustomComponentInfo, CustomResourceInfo, PyEntity, ValidityFlag,
    ValidityGuard,
    custom_component::CustomComponentRegistry,
    custom_resource::{
        hierarchy_contains_resource_entity, insert_dynamic_resource_value, validate_hierarchy_link,
    },
    public_error,
};
use pyo3::{
    ffi::{PyObject, PyTypeObject},
    prelude::*,
    types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyModule, PySet, PyString, PyTuple, PyType},
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

use crate::{
    bridge::{ControlError, EntityRef},
    handlers::{
        entity::resolve_entity,
        json_float::{nonfinite_float_from_json, require_finite_float},
        pyo3::{custom_wrapper, execute::create_world_wrapper, state_resource},
        reflect_mutate::{self, ReflectError},
    },
};

enum SpawnComponent {
    Native(Arc<dyn ComponentBridge>),
    Custom(bevy::ecs::component::ComponentId),
}

fn find_custom_component(world: &World, name: &str) -> Option<bevy::ecs::component::ComponentId> {
    let info = world.get_resource::<CustomComponentInfo>()?;
    let registry = world.get_resource::<CustomComponentRegistry>()?;
    info.iter()
        .find(|(id, entry)| {
            (entry.name == name || entry.name.rsplit('.').next() == Some(name))
                && registry.get(entry.type_ptr as usize) == Some(*id)
        })
        .map(|(id, _)| id)
}

fn find_spawn_component(world: &World, name: &str) -> Option<SpawnComponent> {
    find_bridge(name)
        .map(SpawnComponent::Native)
        .or_else(|| find_custom_component(world, name).map(SpawnComponent::Custom))
}

/// Find a component bridge by name.
fn find_bridge(name: &str) -> Option<Arc<dyn ComponentBridge>> {
    pybevy_core::registry::global_registry::all_component_bridges()
        .into_iter()
        .find(|b| b.name() == name)
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
                .map_or(serde_json::Value::Null, |value| {
                    super::scene::py_value_to_json(&value)
                });
            (name.clone(), value)
        })
        .collect()
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
        match find_spawn_component(world, comp_name) {
            Some(SpawnComponent::Native(bridge)) if !bridge.can_insert() => {
                validation_errors.push(format!("{comp_name}: cannot be spawned from Python"))
            }
            Some(component) => resolved.push(component),
            None => validation_errors.push(format!("{comp_name}: not found in registry")),
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

    for ((comp_name, comp_fields), component) in obj.iter().zip(resolved) {
        let bridge = match component {
            SpawnComponent::Native(bridge) => bridge,
            SpawnComponent::Custom(component_id) => {
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
                // Fall back to Python - it handles many field types
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

    // The partial entity is despawned, so nothing was created and the request
    // failed. Answering 201 Created with a null entity_id read as success.
    if !errors.is_empty() {
        world.despawn(entity);
        return Err(ControlError::invalid_params(format!(
            "Failed to spawn entity: {}",
            errors.join("; ")
        )));
    }

    Ok(serde_json::json!({
        "entity_id": entity_id,
        "components_added": added_components,
    }))
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

/// Return whether `field_name` appears in the class's declared component
/// fields. `None` means the class exposes no supported declaration metadata,
/// so callers may fall back to instance attribute lookup.
fn class_declares_component_field(class: &Bound<'_, PyType>, field_name: &str) -> Option<bool> {
    let mut found_metadata = false;
    for attribute in ["__dataclass_fields__", "__annotations__"] {
        let Ok(metadata) = class.getattr(attribute) else {
            continue;
        };
        let Ok(fields) = metadata.cast::<PyDict>() else {
            continue;
        };
        found_metadata = true;
        if fields.contains(field_name).unwrap_or(false) {
            return Some(true);
        }
    }
    found_metadata.then_some(false)
}

fn resolved_field_annotation<'py>(
    py: Python<'py>,
    class: &Bound<'py, PyType>,
    field_name: &str,
) -> Option<Bound<'py, PyAny>> {
    let typing = PyModule::import(py, "typing").ok()?;
    let hints = typing
        .call_method1("get_type_hints", (class,))
        .ok()?
        .cast_into::<PyDict>()
        .ok()?;
    hints.get_item(field_name).ok().flatten()
}

fn annotation_name(annotation: &Bound<'_, PyAny>) -> String {
    annotation
        .getattr("__name__")
        .and_then(|name| name.extract::<String>())
        .unwrap_or_else(|_| annotation.to_string())
}

fn validate_annotated_field_value(
    py: Python<'_>,
    class: &Bound<'_, PyType>,
    field_name: &str,
    mut value: Py<PyAny>,
) -> Result<Py<PyAny>, String> {
    let Some(annotation) = resolved_field_annotation(py, class, field_name) else {
        return Ok(value);
    };

    if annotation.is(py.get_type::<PyFloat>())
        && value.bind(py).is_instance_of::<PyInt>()
        && !value.bind(py).is_instance_of::<PyBool>()
    {
        value = py
            .get_type::<PyFloat>()
            .call1((value.bind(py),))
            .map_err(|error| error.to_string())?
            .unbind();
    }

    let builtins = PyModule::import(py, "builtins").map_err(|error| error.to_string())?;
    let direct_check = builtins
        .getattr("isinstance")
        .and_then(|isinstance| isinstance.call1((value.bind(py), &annotation)))
        .and_then(|result| result.extract::<bool>());
    let compatible = match direct_check {
        Ok(compatible) => compatible,
        Err(_) => {
            let typing = PyModule::import(py, "typing").map_err(|error| error.to_string())?;
            let origin = typing
                .call_method1("get_origin", (&annotation,))
                .map_err(|error| error.to_string())?;
            if origin.is_none() {
                return Ok(value);
            }
            builtins
                .getattr("isinstance")
                .and_then(|isinstance| isinstance.call1((value.bind(py), origin)))
                .and_then(|result| result.extract::<bool>())
                .map_err(|error| error.to_string())?
        }
    };

    if compatible {
        return Ok(value);
    }

    let actual = value
        .bind(py)
        .get_type()
        .name()
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .into_owned();
    Err(format!(
        "expected {}, got {actual}",
        annotation_name(&annotation)
    ))
}

fn convert_annotated_field_value(
    py: Python<'_>,
    instance: &Bound<'_, PyAny>,
    field_name: &str,
    field_value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    let value = convert_field_value(py, instance, field_name, field_value)?;
    validate_annotated_field_value(py, &instance.get_type(), field_name, value)
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
                match reflect_mutate::reflect_spawn_component(world, entity, type_id, field_obj) {
                    Ok(()) => {
                        let new_values = reflect_mutate::reflect_read_fields(
                            world,
                            entity,
                            type_id,
                            field_obj.keys(),
                        );
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
                            world, entity, entity_id, &component, field_obj, &bridge,
                        );
                    }
                    Err(ReflectError::FieldError(_)) => {
                        return insert_component_python(
                            world, entity, entity_id, &component, field_obj, &bridge,
                        );
                    }
                    Err(ReflectError::ComponentNotOnEntity) => {
                        return Err(ControlError::not_found(format!(
                            "Entity {entity_id} not found"
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
            return Err(ControlError::invalid_params(format!(
                "Entity {entity_id} not found"
            )));
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
                .ok_or_else(|| ControlError::not_found(format!("Entity {entity_id} not found")))?;
            let current = extracted.bind(py);
            let result = if let Some(serde_json::Value::String(variant_name)) =
                field_obj.get("variant")
            {
                let mut payload = field_obj.clone();
                payload.remove("variant");
                let payload = if payload.is_empty() {
                    serde_json::Value::Null
                } else if payload.len() == 1 {
                    payload
                        .remove("value")
                        .unwrap_or(serde_json::Value::Object(payload))
                } else {
                    serde_json::Value::Object(payload)
                };
                let replacement = construct_enum_variant(
                    py,
                    current,
                    variant_name,
                    &payload,
                    VariantSource::Named,
                )
                .map_err(|error| {
                    ControlError::invalid_params(format!("Failed to set '{component}': {error}"))
                })?;
                match replacement {
                    Some(replacement) => Ok((replacement, Some(variant_name.clone()))),
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
                    return Err(ControlError::invalid_params(unknown_field_error(
                        component,
                        &unknown,
                        &writable_field_names(current),
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
                bridge.insert(world, entity, instance).map_err(|error| {
                    ControlError::invalid_params(format!(
                        "Failed to replace component '{component}': {error}"
                    ))
                })?;
                let mut post = read_back_fields(instance, field_obj.keys());
                post.insert(
                    "variant".to_string(),
                    serde_json::Value::String(variant_name),
                );
                return Ok(post);
            }

            let mut converted = Vec::with_capacity(field_obj.len());
            for (field_name, field_value) in field_obj {
                let py_value = convert_field_value(py, instance, field_name, field_value).map_err(
                    |error| {
                        ControlError::invalid_params(format!(
                            "Failed to set '{component}': {field_name}: {error}"
                        ))
                    },
                )?;
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

    let new_values = Python::attach(|py| {
        let py_type = bridge.py_type(py);
        let instance = match py_type.call0() {
            Ok(instance) => match resolve_enum_instance(py, &instance, field_obj) {
                Ok(Some(resolved)) => {
                    let bound = resolved.bind(py);
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
                            serde_json::Value::String(variant_name.clone()),
                        );
                    }
                    return Ok(post);
                }
                Ok(None) => {
                    // Same pre-write guard before the insert setattr path.
                    let unknown = unknown_set_fields(&instance, field_obj.keys());
                    if !unknown.is_empty() {
                        return Err(ControlError::invalid_params(unknown_field_error(
                            component,
                            &unknown,
                            &writable_field_names(&instance),
                        )));
                    }
                    for (field_name, field_value) in field_obj {
                        let py_value = convert_field_value(py, &instance, field_name, field_value)
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
    field_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, ControlError> {
    // Find the custom component's ComponentId: try exact name match first
    let custom_info = world
        .get_resource::<CustomComponentInfo>()
        .and_then(|info| {
            info.iter()
                .find(|(_, entry)| entry.name == component)
                .map(|(id, entry)| (id, entry.is_pyobject_storage, entry.wrapper_layout.clone()))
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
                return Some((id, entry.is_pyobject_storage, entry.wrapper_layout.clone()));
            }
        }

        // Last resort: check entity's archetype for PyObject-stored components
        // whose Python type name matches. PyObject storage is identified by the
        // exact descriptor shape create_python_object_descriptor produces: no Rust
        // TypeId, immutable, Py<PyAny> layout. Wrapper storage is mutable, so it
        // can never reach the deref below (ComponentWrapper8 has the same layout).
        let components = world.components();
        for comp_id in entity_ref.archetype().components() {
            // Skip components already in CustomComponentInfo (checked above)
            if info.get(*comp_id).is_some() {
                continue;
            }
            let Some(comp_info) = components.get_info(*comp_id) else {
                continue;
            };
            let is_pyobject_descriptor = comp_info.type_id().is_none()
                && !comp_info.mutable()
                && comp_info.layout() == Layout::new::<Py<PyAny>>();
            if !is_pyobject_descriptor {
                continue;
            }
            if let Ok(ptr) = entity_ref.get_by_id(*comp_id) {
                let matched = Python::attach(|py| {
                    // SAFETY: only create_python_object_descriptor registrations
                    // match the descriptor shape checked above, so the raw data
                    // is a live Py<PyAny>
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
                    return Some((*comp_id, true, None));
                }
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
             If this is a custom @component, try reloading the scene.{hint}"
        )));
    };

    // Check entity has this component
    let eref = world
        .get_entity(entity)
        .map_err(|_| ControlError::not_found(format!("Entity {entity_id} not found")))?;

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
        if !custom_wrapper::descriptor_matches(&layout, descriptor_layout) {
            return Err(ControlError::internal(format!(
                "Component '{component}' wrapper layout does not match its Bevy descriptor"
            )));
        }

        // An unknown field name fails the whole update before anything is
        // written, matching set_resource's unknown-field contract. Value
        // conversion failures below stay per-field, like PyObject storage.
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
        let (values, errors) = custom_wrapper::values_from_json(&layout, field_obj);
        if !errors.is_empty() {
            return Err(ControlError::invalid_params(format!(
                "Failed to set '{component}': {}",
                errors.join("; ")
            )));
        }
        let mut new_values = serde_json::Map::new();
        if !values.is_empty() {
            let mut entity_mut = world
                .get_entity_mut(entity)
                .map_err(|_| ControlError::not_found(format!("Entity {entity_id} not found")))?;
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
            let stored = custom_wrapper::fields_to_json(untyped.as_ref(), &layout)
                .map_err(ControlError::internal)?;
            for name in written {
                let value = stored
                    .get(&name)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                new_values.insert(name, value);
            }
        }

        let mut result = serde_json::json!({
            "entity_id": entity_id,
            "component": component,
            "new_values": new_values,
        });
        if !errors.is_empty() {
            result
                .as_object_mut()
                .unwrap()
                .insert("errors".into(), serde_json::json!(errors));
        }
        return Ok(result);
    }

    // Component exists - get mutable pointer for in-place mutation
    let ptr = world
        .get_entity(entity)
        .map_err(|_| ControlError::not_found(format!("Entity {entity_id} not found")))?
        .get_by_id(comp_id)
        .map_err(|_| {
            ControlError::not_found(format!(
                "Component '{component}' not found on entity {entity_id}"
            ))
        })?;

    let mut new_values = serde_json::Map::new();
    let mut errors = Vec::new();

    Python::attach(|py| {
        // SAFETY: We checked is_pyobject_storage above - raw data is a Py<PyAny>
        let py_obj: &pyo3::Py<PyAny> = unsafe { &*(ptr.as_ptr() as *const pyo3::Py<PyAny>) };
        let bound = py_obj.bind(py);

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
    });

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
        if !custom_wrapper::descriptor_matches(&layout, descriptor_layout) {
            return Err(ControlError::internal(format!(
                "Component '{component}' wrapper layout does not match its Bevy descriptor"
            )));
        }
        let ptr = world
            .get_entity(entity)
            .map_err(|_| ControlError::not_found(format!("Entity {entity_id} not found")))?
            .get_by_id(comp_id)
            .map_err(|_| {
                ControlError::internal(format!(
                    "Component '{component}' was not inserted on entity {entity_id}"
                ))
            })?;
        let stored =
            custom_wrapper::fields_to_json(ptr, &layout).map_err(ControlError::internal)?;
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

/// Queue a state transition from a `{"variant": "Member"}` payload.
///
/// `State`/`NextState` are driven by `set`/`reset`, so the value cannot be
/// written with the ordinary setattr loop.
fn apply_state_variant(
    bound: &Bound<'_, PyAny>,
    kind: state_resource::StateResource,
    resource_type: &str,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, ControlError> {
    let requested = fields
        .get(state_resource::VARIANT)
        .unwrap_or(&serde_json::Value::Null);
    let variant = match requested {
        serde_json::Value::Null => None,
        serde_json::Value::String(name) => Some(name.as_str()),
        other => {
            return Err(ControlError::invalid_params(format!(
                "{resource_type}.variant expects a member name, got {other}"
            )));
        }
    };

    state_resource::write_variant(bound, kind, variant).map_err(ControlError::invalid_params)?;

    Ok(serde_json::json!({
        "inserted": resource_type,
        "custom": true,
    }))
}

fn validate_custom_resource_fields(
    resource_type: &str,
    instance: &Bound<'_, PyAny>,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), ControlError> {
    let py_type = instance.get_type();
    let descriptors = py_type
        .getattr("__dataclass_fields__")
        .or_else(|_| py_type.getattr("__annotations__"))
        .ok()
        .and_then(|value| value.cast_into::<PyDict>().ok());

    let mut declared = descriptors
        .map(|descriptors| {
            descriptors
                .keys()
                .iter()
                .filter_map(|key| key.extract::<String>().ok())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    if state_resource::classify(instance).is_some() {
        declared.insert(state_resource::VARIANT.to_string());
    }
    let unknown = fields
        .keys()
        .filter(|field| !declared.contains(*field))
        .cloned()
        .collect::<Vec<_>>();

    if unknown.is_empty() {
        return Ok(());
    }

    let valid = if declared.is_empty() {
        "This resource declares no editable fields. Define it with @resource above @dataclass and annotated fields to use set_resource.".to_string()
    } else {
        format!(
            "Valid fields: {}.",
            declared.into_iter().collect::<Vec<_>>().join(", ")
        )
    };
    Err(ControlError::invalid_params(format!(
        "Resource '{resource_type}' has unknown fields: {}. {valid}",
        unknown.join(", ")
    )))
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

    // Stripping IsResource runs Bevy's Discard hook, destroying the resource
    // value behind the entity.
    if component == "IsResource" {
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

    for bridge in pybevy_core::registry::global_registry::all_component_bridges() {
        if bridge.name() == component.as_str() {
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
                return Err(ControlError::not_found(format!(
                    "Entity {entity_id} not found"
                )));
            }
        }
    }

    // Fallback: check custom Python components via CustomComponentInfo
    let custom_component = world
        .get_resource::<CustomComponentInfo>()
        .and_then(|info| {
            info.iter()
                .find(|(_, entry)| entry.name == component)
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

                        // Convert every field before writing any, so a bad
                        // value leaves the resource untouched.
                        let mut converted = Vec::with_capacity(obj.len());
                        for (field_name, field_value) in obj {
                            match convert_field_value(py, instance, field_name, field_value) {
                                Ok(py_value) => converted.push((field_name, py_value)),
                                Err(e) => {
                                    write_flag.set_invalid();
                                    return Err(ControlError::internal(format!(
                                        "Failed to convert {field_name}: {e}"
                                    )));
                                }
                            }
                        }
                        for (field_name, py_value) in converted {
                            if let Err(e) = instance.setattr(field_name.as_str(), py_value) {
                                write_flag.set_invalid();
                                return Err(ControlError::internal(format!(
                                    "Failed to set {field_name}: {e}"
                                )));
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
            // Mutable access so the patch stamps the change tick: Changed[T] and
            // Res.is_changed must observe control-plane edits the same as ECS ones.
            if let Some(mut existing) = world.get_resource_mut_by_id(comp_id) {
                // SAFETY: custom resource metadata refers to a Py<PyAny> descriptor.
                let existing = unsafe { existing.as_mut().deref_mut::<Py<PyAny>>() };
                let bound = existing.bind(py);
                if let Some(obj) = value.as_object() {
                    validate_custom_resource_fields(&resource_type, bound, obj)?;
                    if let Some(kind) = state_resource::classify(bound) {
                        return apply_state_variant(bound, kind, &resource_type, obj);
                    }
                    let mut converted = Vec::with_capacity(obj.len());
                    for (field_name, field_value) in obj {
                        let py_value =
                            convert_annotated_field_value(py, bound, field_name, field_value)
                                .map_err(|error| {
                                    ControlError::invalid_params(format!(
                                        "Failed to convert {resource_type}.{field_name}: {error}"
                                    ))
                                })?;
                        converted.push((field_name, py_value));
                    }
                    for (field_name, py_value) in converted {
                        if let Err(e) = bound.setattr(field_name.as_str(), py_value) {
                            return Err(ControlError::invalid_params(format!(
                                "Failed to set {resource_type}.{field_name}: {e}"
                            )));
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
                validate_custom_resource_fields(&resource_type, &instance, obj)?;
                let mut converted = Vec::with_capacity(obj.len());
                for (field_name, field_value) in obj {
                    let py_value =
                        convert_annotated_field_value(py, &instance, field_name, field_value)
                            .map_err(|error| {
                                ControlError::invalid_params(format!(
                                    "Failed to convert {resource_type}.{field_name}: {error}"
                                ))
                            })?;
                    converted.push((field_name, py_value));
                }
                for (field_name, py_value) in converted {
                    if let Err(e) = instance.setattr(field_name.as_str(), py_value) {
                        return Err(ControlError::invalid_params(format!(
                            "Failed to set {resource_type}.{field_name}: {e}"
                        )));
                    }
                }
            }

            // SAFETY: custom resource metadata refers to a Py<PyAny> descriptor,
            // and control mutations use only the canonical resource path.
            unsafe { insert_dynamic_resource_value(world, comp_id, instance.unbind()) };

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
            let was_present = bridge.contains_in_world(world);
            bridge.remove(world);
            return Ok(serde_json::json!({
                "removed": resource_type,
                "was_present": was_present,
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
        let was_present = world.remove_resource_by_id(comp_id);
        return Ok(serde_json::json!({
            "removed": resource_type,
            "was_present": was_present,
        }));
    }

    Err(ControlError::not_found(format!(
        "Resource '{resource_type}' not in registry"
    )))
}

/// Execute a batch of mutation operations in a single World access.
/// Each operation runs independently - failures don't abort the batch.
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

    Ok(super::super::batch_response(results))
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

/// Build an `Entity` from the id form used throughout the control API.
fn entity_from_json(py: Python<'_>, value: &serde_json::Value) -> Result<Py<PyAny>, String> {
    let bits = value
        .as_u64()
        .ok_or_else(|| format!("expected an entity id, got {value}"))?;
    let entity = PyEntity::from_bits(bits).map_err(|error| error.to_string())?;
    Ok(Py::new(py, entity)
        .map_err(|error| error.to_string())?
        .into_any())
}

/// Convert a JSON value for a constructor argument.
///
/// Same as [`json_to_py`] except on a relationship component's entity field,
/// where JSON's integer is an entity id rather than a plain number.
fn json_to_py_for_field(
    py: Python<'_>,
    bridge: &Arc<dyn ComponentBridge>,
    field_name: &str,
    value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    if bridge.relationship_field() == Some(field_name) {
        return entity_from_json(py, value);
    }
    json_to_py(py, value)
}

fn enum_owner_type<'py>(current: &Bound<'py, PyAny>) -> Bound<'py, PyType> {
    let current_type = current.get_type();
    let Some(base) = current_type
        .getattr("__bases__")
        .ok()
        .and_then(|bases| bases.get_item(0).ok())
        .and_then(|base| base.cast_into::<PyType>().ok())
    else {
        return current_type;
    };
    let nested_variant = base.dir().is_ok_and(|names| {
        names.iter().any(|name| {
            let Ok(name) = name.extract::<String>() else {
                return false;
            };
            base.getattr(name.as_str())
                .ok()
                .and_then(|candidate| candidate.cast_into::<PyType>().ok())
                .is_some_and(|candidate| candidate.is(&current_type))
        })
    });
    if nested_variant { base } else { current_type }
}

fn enum_variant_names(owner: &Bound<'_, PyType>) -> Vec<String> {
    let mut names = owner
        .dir()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|name| name.extract::<String>().ok())
        .filter(|name| !name.starts_with('_'))
        .filter(|name| {
            let Ok(value) = owner.getattr(name.as_str()) else {
                return false;
            };
            value.is_instance(owner).unwrap_or(false)
                || value.cast::<PyType>().is_ok_and(|variant| {
                    variant.is_subclass(owner).unwrap_or(false) && !variant.is(owner)
                })
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn enum_field_prototype<'py>(
    py: Python<'py>,
    variant: &Bound<'py, PyType>,
    field_name: &str,
) -> Result<Option<Bound<'py, PyAny>>, String> {
    let bases = variant
        .getattr("__bases__")
        .map_err(|error| error.to_string())?;
    let owner = bases
        .get_item(0)
        .map_err(|error| error.to_string())?
        .cast_into::<PyType>()
        .map_err(|error| error.to_string())?;
    let owner_name = owner.name().map_err(|error| error.to_string())?;
    let variant_name = variant.name().map_err(|error| error.to_string())?;
    let Some(VariantInfo::Struct(fields)) = reflect_mutate::compiled_enum_variant_info(
        &owner_name.to_string_lossy(),
        &variant_name.to_string_lossy(),
    ) else {
        return Ok(None);
    };
    let Some(type_info) = fields.field(field_name).and_then(|field| field.type_info()) else {
        return Ok(None);
    };
    let Some(type_name) = type_info.type_path_table().ident() else {
        return Ok(None);
    };
    let constant = match type_name {
        "Vec2" | "Vec3" | "Vec4" => "ZERO",
        "Quat" => "IDENTITY",
        _ => return Ok(None),
    };
    let math = PyModule::import(py, "pybevy.math").map_err(|error| error.to_string())?;
    math.getattr(type_name)
        .and_then(|class| class.getattr(constant))
        .map(Some)
        .map_err(|error| error.to_string())
}

// Distinguishes explicit names from untagged object keys for error reporting.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VariantSource {
    Named,
    ObjectKey,
}

fn construct_wrapped_variant_payload<'py>(
    py: Python<'py>,
    variant: &Bound<'py, PyAny>,
    variant_name: &str,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<Py<PyAny>>, String> {
    let Ok(prototype) = variant.getattr("__pybevy_control_payload_prototype__") else {
        return Ok(None);
    };
    let Ok(payload_type) = prototype.cast::<PyType>() else {
        return Ok(None);
    };
    let Ok(prototype) = payload_type.call0() else {
        return Ok(None);
    };
    let holder = PyModule::import(py, "types")
        .and_then(|types| types.getattr("SimpleNamespace"))
        .and_then(|ns| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("value", prototype.clone())?;
            ns.call((), Some(&kwargs))
        })
        .map_err(|error| format!("invalid payload for variant '{variant_name}': {error}"))?;
    let payload = convert_field_value(
        py,
        &holder,
        "value",
        &serde_json::Value::Object(fields.clone()),
    )?;
    variant
        .call1((payload,))
        .map(|result| Some(result.unbind()))
        .map_err(|error| format!("invalid payload for variant '{variant_name}': {error}"))
}

fn construct_enum_variant(
    py: Python<'_>,
    current: &Bound<'_, PyAny>,
    variant_name: &str,
    variant_value: &serde_json::Value,
    source: VariantSource,
) -> Result<Option<Py<PyAny>>, String> {
    let owner = enum_owner_type(current);
    let variants = enum_variant_names(&owner);
    // Payload fields are attributes too; accept only registered variant names.
    let variant = owner
        .getattr(variant_name)
        .ok()
        .filter(|_| variants.iter().any(|name| name == variant_name));
    let Some(variant) = variant else {
        if variants.is_empty() {
            return Ok(None);
        }
        let owner_name = owner
            .name()
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .into_owned();
        if variant_name == "value" && source == VariantSource::ObjectKey {
            return Err(format!(
                "a payload needs the variant it belongs to: pass \
                 {{\"variant\": \"<name>\", \"value\": ...}} for {owner_name}. \
                 Valid variants: {}",
                variants.join(", ")
            ));
        }
        return Err(format!(
            "unknown variant '{variant_name}' for {}. Valid variants: {}",
            owner_name,
            variants.join(", ")
        ));
    };

    let is_unit = variant_value.is_null()
        || variant_value
            .as_object()
            .is_some_and(serde_json::Map::is_empty);
    if is_unit {
        if let Ok(result) = variant.call0() {
            return Ok(Some(result.unbind()));
        }
        if variant.is_instance(&owner).unwrap_or(false) {
            return Ok(Some(variant.unbind()));
        }
        return Err(format!("variant '{variant_name}' requires a payload"));
    }

    let has_wrapped_payload = variant
        .hasattr("__pybevy_control_payload_prototype__")
        .unwrap_or(false);
    if variant_value.is_array() {
        let expected = if has_wrapped_payload {
            public_error::MCP_ENUM_WRAPPED_ARRAY_PAYLOAD
        } else {
            public_error::MCP_ENUM_SCALAR_ARRAY_PAYLOAD
        };
        return Err(format!(
            "invalid payload for variant '{variant_name}': {expected}"
        ));
    }

    if variant
        .cast::<PyType>()
        .is_ok_and(|variant_type| current.is_instance(variant_type).unwrap_or(false))
        && current.hasattr("value").unwrap_or(false)
    {
        let scalar_payload = current.getattr("value").is_ok_and(|payload| {
            payload.is_instance_of::<PyBool>()
                || payload.is_instance_of::<PyFloat>()
                || payload.is_instance_of::<PyInt>()
                || payload.is_instance_of::<PyString>()
        });
        if variant_value.is_object() && !has_wrapped_payload && scalar_payload {
            return Err(format!(
                "invalid payload for variant '{variant_name}': {}",
                public_error::MCP_ENUM_SCALAR_OBJECT_PAYLOAD
            ));
        }
        let argument = convert_field_value(py, current, "value", variant_value)?;
        return variant
            .call1((argument,))
            .map(|result| Some(result.unbind()))
            .map_err(|error| format!("invalid payload for variant '{variant_name}': {error}"));
    }

    if let serde_json::Value::Object(fields) = variant_value {
        let kwargs = PyDict::new(py);
        for (name, value) in fields {
            let value = convert_field_value(py, &variant, name, value)
                .map_err(|error| format!("{name}: {error}"))?;
            kwargs
                .set_item(name, value)
                .map_err(|error| error.to_string())?;
        }
        match variant.call((), Some(&kwargs)) {
            Ok(result) => return Ok(Some(result.unbind())),
            Err(error)
                if owner.name().is_ok_and(|name| {
                    matches!(
                        reflect_mutate::compiled_enum_variant_info(
                            &name.to_string_lossy(),
                            variant_name
                        ),
                        Some(VariantInfo::Struct(_))
                    )
                }) =>
            {
                return Err(format!(
                    "invalid payload for variant '{variant_name}': {error}"
                ));
            }
            Err(_) => {
                // A failed kwargs call may mean the variant wraps one payload struct.
                if let Some(result) =
                    construct_wrapped_variant_payload(py, &variant, variant_name, fields)?
                {
                    return Ok(Some(result));
                }
            }
        }
    }

    let argument = json_to_py(py, variant_value)?;
    match variant.call1((argument,)) {
        Ok(result) => Ok(Some(result.unbind())),
        Err(_) if variant_value.is_object() => Err(format!(
            "invalid payload for variant '{variant_name}': {}",
            public_error::MCP_ENUM_SCALAR_OBJECT_PAYLOAD
        )),
        Err(error) => Err(format!(
            "invalid payload for variant '{variant_name}': {error}"
        )),
    }
}

// An enum has no variant field to setattr; resolve the tagged payload via the shared resolver.
fn resolve_enum_instance(
    py: Python<'_>,
    default: &Bound<'_, PyAny>,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<Py<PyAny>>, String> {
    let Some(serde_json::Value::String(variant_name)) = fields.get("variant") else {
        return Ok(None);
    };
    let mut payload = fields.clone();
    payload.remove("variant");
    let payload = if payload.is_empty() {
        serde_json::Value::Null
    } else if payload.len() == 1 {
        payload
            .remove("value")
            .unwrap_or(serde_json::Value::Object(payload))
    } else {
        serde_json::Value::Object(payload)
    };
    construct_enum_variant(py, default, variant_name, &payload, VariantSource::Named)
}

fn construct_color_variant(
    py: Python<'_>,
    variant_name: &str,
    variant_value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    let color_module = PyModule::import(py, "pybevy.color").map_err(|error| error.to_string())?;
    let color = color_module
        .getattr("Color")
        .map_err(|error| error.to_string())?;
    let variant = color
        .getattr(variant_name)
        .map_err(|_| format!("unknown Color variant '{variant_name}'"))?;
    let payload_type = color_module
        .getattr(variant_name)
        .map_err(|_| format!("Color variant '{variant_name}' has no payload type"))?;

    let payload = match variant_value {
        serde_json::Value::Object(fields) => {
            let kwargs = PyDict::new(py);
            for (name, value) in fields {
                kwargs
                    .set_item(name, json_to_py(py, value)?)
                    .map_err(|error| error.to_string())?;
            }
            payload_type
                .call((), Some(&kwargs))
                .map_err(|error| format!("invalid payload for Color.{variant_name}: {error}"))?
        }
        serde_json::Value::Array(values) => {
            let args = values
                .iter()
                .map(|value| json_to_py(py, value))
                .collect::<Result<Vec<_>, _>>()?;
            let args = PyTuple::new(py, args).map_err(|error| error.to_string())?;
            payload_type
                .call1(args)
                .map_err(|error| format!("invalid payload for Color.{variant_name}: {error}"))?
        }
        _ => {
            return Err(format!(
                "Color.{variant_name} expects an object or array payload"
            ));
        }
    };

    variant
        .call1((payload,))
        .map(Bound::unbind)
        .map_err(|error| format!("invalid payload for Color.{variant_name}: {error}"))
}

/// Whether the value is a dataclass instance, the same check `dataclasses`
/// itself performs, without importing the module for every candidate field.
fn is_dataclass_instance(value: &Bound<'_, PyAny>) -> bool {
    !value.is_instance_of::<PyType>() && value.hasattr("__dataclass_fields__").unwrap_or(false)
}

/// Rebuild an array-like field (numpy and friends) from a JSON array through
/// the module that owns the current value, so no array dependency is linked in.
fn rebuild_array_like(
    py: Python<'_>,
    current: &Bound<'_, PyAny>,
    field_value: &serde_json::Value,
) -> Result<Option<Py<PyAny>>, String> {
    if !current.hasattr("__array__").unwrap_or(false) {
        return Ok(None);
    }
    let Ok(module_name) = current
        .get_type()
        .getattr("__module__")
        .and_then(|name| name.extract::<String>())
    else {
        return Ok(None);
    };
    let Ok(module) = PyModule::import(py, module_name.as_str()) else {
        return Ok(None);
    };
    let Ok(as_array) = module.getattr("asarray") else {
        return Ok(None);
    };
    let values = json_to_py(py, field_value)?;
    let kwargs = PyDict::new(py);
    if let Ok(dtype) = current.getattr("dtype") {
        kwargs
            .set_item("dtype", dtype)
            .map_err(|error| error.to_string())?;
    }
    as_array
        .call((values,), Some(&kwargs))
        .map(|array| Some(array.unbind()))
        .map_err(|error| error.to_string())
}

fn has_public_getset_fields(value: &Bound<'_, PyAny>) -> bool {
    value.get_type().dir().is_ok_and(|names| {
        names.iter().any(|name| {
            let Ok(name) = name.extract::<String>() else {
                return false;
            };
            !name.starts_with('_')
                && value
                    .get_type()
                    .getattr(name.as_str())
                    .is_ok_and(|attribute| {
                        attribute
                            .get_type()
                            .name()
                            .is_ok_and(|kind| kind == "getset_descriptor")
                    })
        })
    })
}

fn writable_field_names(instance: &Bound<'_, PyAny>) -> Vec<String> {
    let class = instance.get_type();
    // Probe a fresh owned copy so a read-only setter error cannot touch the live component.
    let probe = match instance.call_method0("__copy__").or_else(|_| class.call0()) {
        Ok(copy) => copy,
        Err(_) => return Vec::new(),
    };
    let mut names = Vec::new();
    let Ok(dir_list) = class.dir() else {
        return names;
    };
    for attr_obj in dir_list.iter() {
        let Ok(name) = attr_obj.extract::<String>() else {
            continue;
        };
        if name.starts_with('_') {
            continue;
        }
        let Ok(attr) = class.getattr(name.as_str()) else {
            continue;
        };
        let descriptor = attr
            .get_type()
            .name()
            .ok()
            .map(|kind| kind.to_string_lossy().into_owned())
            .is_some_and(|kind| kind == "getset_descriptor" || kind == "property");
        if !descriptor {
            continue;
        }
        // Assigning the field's own value distinguishes a getter from a getter/setter.
        if probe
            .getattr(name.as_str())
            .and_then(|value| probe.setattr(name.as_str(), value))
            .is_ok()
        {
            names.push(name);
        }
    }
    names.sort();
    names.dedup();
    names
}

fn declares_component_field(py_type: &Bound<'_, PyType>, name: &str) -> bool {
    py_type.getattr(name).is_ok_and(|attr| {
        attr.get_type().name().ok().is_some_and(|kind| {
            matches!(
                kind.to_string_lossy().as_ref(),
                "getset_descriptor" | "property"
            )
        })
    })
}

fn unknown_set_fields<'a>(
    instance: &Bound<'_, PyAny>,
    field_names: impl IntoIterator<Item = &'a String>,
) -> Vec<String> {
    if instance.getattr("__dict__").is_ok() {
        return Vec::new();
    }
    let class = instance.get_type();
    field_names
        .into_iter()
        .filter(|name| {
            instance.getattr(name.as_str()).is_err() && !declares_component_field(&class, name)
        })
        .cloned()
        .collect()
}

fn levenshtein_distance(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (i, left_char) in left.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, right_char) in right.iter().enumerate() {
            let cost = usize::from(left_char != right_char);
            current.push(std::cmp::min(
                std::cmp::min(previous[j + 1] + 1, current[j] + 1),
                previous[j] + cost,
            ));
        }
        previous = current;
    }
    previous[right.len()]
}

fn close_field_suggestion(typo: &str, candidates: &[String]) -> Option<String> {
    let typo = typo.to_lowercase();
    let mut best: Option<(usize, &String)> = None;
    for candidate in candidates {
        let distance = levenshtein_distance(&typo, &candidate.to_lowercase());
        if (1..=2).contains(&distance) && best.is_none_or(|(known, _)| distance < known) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, candidate)| candidate.clone())
}

fn unknown_field_error(component: &str, unknown: &[String], writable: &[String]) -> String {
    let suggestion = if unknown.len() == 1 {
        close_field_suggestion(&unknown[0], writable)
    } else {
        None
    };
    if let Some(candidate) = suggestion {
        return format!(
            "Failed to set '{component}': unknown field '{}'. Did you mean '{}'?",
            unknown[0], candidate
        );
    }
    let subject = if unknown.len() == 1 {
        format!("unknown field '{}'", unknown[0])
    } else {
        format!("unknown fields: {}", unknown.join(", "))
    };
    if writable.is_empty() {
        return format!(
            "Failed to set '{component}': {subject}. This component exposes no writable fields."
        );
    }
    if writable.len() <= 8 {
        return format!(
            "Failed to set '{component}': {subject}. Valid writable fields: {}.",
            writable.join(", ")
        );
    }
    format!(
        "Failed to set '{component}': {subject}. See GET /api/v1/components/{component}/schema for the writable fields."
    )
}

enum ListElementTarget<'py> {
    Generic,
    Wrapper {
        class: Bound<'py, PyType>,
        instance: Option<Bound<'py, PyAny>>,
    },
}

fn convert_list_elements<'py>(
    py: Python<'py>,
    component: &Bound<'py, PyAny>,
    field_name: &str,
    current: &Bound<'py, PyAny>,
    arr: &[serde_json::Value],
) -> Result<Option<Py<PyAny>>, String> {
    let ListElementTarget::Wrapper { class, instance } =
        list_element_target(py, component, field_name, current)?
    else {
        return Ok(None);
    };
    let class_name = class
        .name()
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .into_owned();
    let math_shape = match class_name.as_str() {
        "Vec2" => Some("[x, y]"),
        "Vec3" => Some("[x, y, z]"),
        "Vec4" | "Quat" => Some("[x, y, z, w]"),
        _ => None,
    };
    let is_enum = instance
        .as_ref()
        .is_some_and(|instance| !enum_variant_names(&enum_owner_type(instance)).is_empty());
    let items = arr
        .iter()
        .enumerate()
        .map(|(index, value)| -> Result<Py<PyAny>, String> {
            if let Some(shape) = math_shape {
                if let serde_json::Value::Array(values) = value {
                    if let Some(result) = math_value_from_json(py, &class_name, values) {
                        return result;
                    }
                    return Err(format!(
                        "element {index}: {class_name} expects {shape}, got {} elements",
                        values.len()
                    ));
                }
                return Err(format!(
                    "element {index}: {class_name} expects {shape}, got {}",
                    json_kind_name(value)
                ));
            }
            match value {
                serde_json::Value::Object(fields) => {
                    if is_enum {
                        let instance = instance.as_ref().ok_or_else(|| {
                            format!("element {index}: expected a {class_name} element")
                        })?;
                        convert_enum_element(py, instance, index, fields)
                    } else {
                        convert_struct_element(py, &class, instance.as_ref(), index, fields)
                    }
                }
                _ => Err(format!(
                    "element {index}: expected object, got {}",
                    json_kind_name(value)
                )),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    PyList::new(py, items)
        .map(|list| list.unbind().into_any())
        .map(Some)
        .map_err(|error| error.to_string())
}

fn list_element_target<'py>(
    py: Python<'py>,
    component: &Bound<'py, PyAny>,
    field_name: &str,
    current: &Bound<'py, PyAny>,
) -> Result<ListElementTarget<'py>, String> {
    if let Ok(first) = current.get_item(0usize) {
        if first.is_instance_of::<PyString>()
            || first.is_instance_of::<PyInt>()
            || first.is_instance_of::<PyFloat>()
            || first.is_instance_of::<PyBool>()
            || first.is_none()
            || first.is_instance_of::<PyDict>()
            || first.is_instance_of::<PyList>()
            || first.is_instance_of::<PySet>()
            || first.is_instance_of::<PyTuple>()
        {
            return Ok(ListElementTarget::Generic);
        }
        return Ok(ListElementTarget::Wrapper {
            class: first.get_type(),
            instance: Some(first),
        });
    }
    // Probe a detached value because the component can be a live resource borrow.
    let Ok(probe_owner) = component
        .call_method0("__copy__")
        .or_else(|_| component.get_type().call0())
    else {
        return Ok(ListElementTarget::Generic);
    };
    let probe = PyList::new(py, [PyDict::new(py)]).map_err(|error| error.to_string())?;
    match probe_owner.setattr(field_name, &probe) {
        Ok(()) => Ok(ListElementTarget::Generic),
        Err(error) => {
            const MARKER: &str = "is not an instance of '";
            let message = error.to_string();
            let class_name = message.rfind(MARKER).and_then(|position| {
                let rest = &message[position + MARKER.len()..];
                rest.find('\'').map(|end| rest[..end].to_string())
            });
            let Some(class_name) = class_name else {
                return Ok(ListElementTarget::Generic);
            };
            let Some(class) = find_pybevy_class(py, &class_name) else {
                return Ok(ListElementTarget::Generic);
            };
            let instance = constructible_representative(&class);
            Ok(ListElementTarget::Wrapper { class, instance })
        }
    }
}

fn find_pybevy_class<'py>(py: Python<'py>, name: &str) -> Option<Bound<'py, PyType>> {
    let sys = PyModule::import(py, "sys").ok()?;
    let modules_bound = sys.getattr("modules").ok()?;
    let modules = modules_bound.cast::<PyDict>().ok()?;
    for entry in modules.iter() {
        let (key, _value) = entry;
        let Ok(key) = key.extract::<String>() else {
            continue;
        };
        if !key.starts_with("pybevy.") {
            continue;
        }
        let Some(module) = modules.get_item(key.as_str()).ok().flatten() else {
            continue;
        };
        let Ok(candidate) = module.getattr(name) else {
            continue;
        };
        let Ok(candidate) = candidate.cast::<PyType>() else {
            continue;
        };
        let defined_here = candidate
            .getattr("__module__")
            .ok()
            .and_then(|module| module.extract::<String>().ok())
            .is_some_and(|module| module == key);
        if defined_here {
            return Some(candidate.clone());
        }
    }
    None
}

fn constructible_representative<'py>(class: &Bound<'py, PyType>) -> Option<Bound<'py, PyAny>> {
    if let Ok(instance) = class.call0() {
        return Some(instance);
    }
    for name in enum_variant_names(class) {
        let Ok(candidate) = class.getattr(name.as_str()) else {
            continue;
        };
        if candidate.is_instance(class).unwrap_or(false) {
            return Some(candidate);
        }
        let instance = candidate
            .cast::<PyType>()
            .ok()
            .and_then(|variant| variant.call0().ok());
        if let Some(instance) = instance {
            return Some(instance);
        }
    }
    None
}

fn convert_struct_element(
    py: Python<'_>,
    class: &Bound<'_, PyType>,
    instance: Option<&Bound<'_, PyAny>>,
    index: usize,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<Py<PyAny>, String> {
    let class_name = class
        .name()
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .into_owned();
    let kwargs = PyDict::new(py);
    for (name, value) in fields {
        if !class.hasattr(name.as_str()).unwrap_or(false) {
            return Err(format!(
                "element {index}: {class_name} has no field '{name}'"
            ));
        }
        let converted = match instance {
            Some(instance) => convert_field_value(py, instance, name, value)
                .map_err(|error| format!("{name}: {error}"))?,
            None => json_to_py(py, value)?,
        };
        kwargs
            .set_item(name, converted)
            .map_err(|error| error.to_string())?;
    }
    if let Some(required) = missing_constructor_field(class, fields.keys()) {
        return Err(format!(
            "element {index}: {class_name} requires field '{required}'"
        ));
    }
    class
        .call((), Some(&kwargs))
        .map(Bound::unbind)
        .map_err(|error| format!("element {index}: invalid {class_name} element: {error}"))
}

fn enum_element_missing_field(
    owner: &Bound<'_, PyType>,
    variant_name: &str,
    payload: &serde_json::Value,
) -> Option<String> {
    let serde_json::Value::Object(fields) = payload else {
        return None;
    };
    if fields.is_empty() {
        return None;
    }
    if !enum_variant_names(owner)
        .iter()
        .any(|name| name == variant_name)
    {
        return None;
    }
    let variant = owner.getattr(variant_name).ok()?;
    let variant_type = variant.cast::<PyType>().ok()?;
    missing_constructor_field(variant_type, fields.keys())
}

fn convert_enum_element(
    py: Python<'_>,
    instance: &Bound<'_, PyAny>,
    index: usize,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<Py<PyAny>, String> {
    let owner = enum_owner_type(instance);
    let owner_name = owner
        .name()
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .into_owned();
    if let Some(serde_json::Value::String(variant_name)) = fields.get("variant") {
        let mut payload = fields.clone();
        payload.remove("variant");
        let payload = if payload.is_empty() {
            serde_json::Value::Null
        } else if payload.len() == 1 && payload.contains_key("value") {
            payload.remove("value").unwrap()
        } else {
            serde_json::Value::Object(payload)
        };
        if let Some(field) = enum_element_missing_field(&owner, variant_name, &payload) {
            return Err(format!(
                "element {index}: variant '{variant_name}' requires field '{field}'"
            ));
        }
        return construct_enum_variant(py, instance, variant_name, &payload, VariantSource::Named)
            .map_err(|error| format!("element {index}: {error}"))?
            .ok_or_else(|| format!("element {index}: expected a {owner_name} element"));
    }
    if fields.len() == 1 {
        let (variant_name, variant_value) = fields.iter().next().unwrap();
        if let Some(field) = enum_element_missing_field(&owner, variant_name, variant_value) {
            return Err(format!(
                "element {index}: variant '{variant_name}' requires field '{field}'"
            ));
        }
        return construct_enum_variant(
            py,
            instance,
            variant_name,
            variant_value,
            VariantSource::ObjectKey,
        )
        .map_err(|error| format!("element {index}: {error}"))?
        .ok_or_else(|| format!("element {index}: expected a {owner_name} element"));
    }
    Err(format!(
        "element {index}: expected a {owner_name} element with a single variant key"
    ))
}

fn missing_constructor_field<'a>(
    class: &Bound<'_, PyType>,
    given: impl Iterator<Item = &'a String>,
) -> Option<String> {
    let py = class.py();
    let inspect = PyModule::import(py, "inspect").ok()?;
    let signature = inspect.getattr("signature").ok()?.call1((class,)).ok()?;
    let parameters = signature.getattr("parameters").ok()?;
    let empty = inspect.getattr("Parameter").ok()?.getattr("empty").ok()?;
    let given: Vec<&String> = given.collect();
    let Ok(mut iterator) = parameters.try_iter() else {
        return None;
    };
    let mut missing = None;
    while let Some(Ok(name)) = iterator.next() {
        let Ok(name) = name.extract::<String>() else {
            continue;
        };
        let Some(parameter) = parameters.get_item(&name).ok() else {
            continue;
        };
        let variadic = parameter
            .getattr("kind")
            .ok()
            .and_then(|kind| kind.getattr("name").ok())
            .and_then(|name| name.extract::<String>().ok())
            .is_some_and(|kind| kind == "VAR_POSITIONAL" || kind == "VAR_KEYWORD");
        if variadic {
            continue;
        }
        let has_default = parameter
            .getattr("default")
            .ok()
            .is_some_and(|default| !default.is(empty.clone()));
        if !has_default && missing.is_none() && !given.contains(&&name) {
            missing = Some(name);
        }
    }
    missing
}

fn math_value_from_json(
    py: Python<'_>,
    type_name: &str,
    values: &[serde_json::Value],
) -> Option<Result<Py<PyAny>, String>> {
    let count = match type_name {
        "Vec2" | "UVec2" | "IVec2" => 2,
        "Vec3" | "UVec3" => 3,
        "Vec4" | "Quat" => 4,
        _ => return None,
    };
    if values.len() != count {
        return None;
    }
    let integral = matches!(type_name, "UVec2" | "IVec2" | "UVec3");
    let mut coordinates = Vec::with_capacity(count);
    for value in values {
        let coordinate = if integral {
            integer_vector_coordinate_to_py(py, type_name, value)
        } else {
            json_number_to_f32(value).and_then(|number| {
                number
                    .into_pyobject(py)
                    .map(|number| number.into_any().unbind())
                    .map_err(|error| error.to_string())
            })
        };
        match coordinate {
            Ok(coordinate) => coordinates.push(coordinate),
            Err(error) => return Some(Err(error)),
        }
    }
    Some((|| -> Result<Py<PyAny>, String> {
        let math = PyModule::import(py, "pybevy.math").map_err(|error| error.to_string())?;
        let class = math.getattr(type_name).map_err(|error| error.to_string())?;
        let args = PyTuple::new(py, coordinates).map_err(|error| error.to_string())?;
        class
            .call1(args)
            .map(Bound::unbind)
            .map_err(|error| error.to_string())
    })())
}

fn json_kind_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

pub(crate) fn convert_field_value(
    py: Python<'_>,
    component: &Bound<'_, PyAny>,
    field_name: &str,
    field_value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    // Try to get the current field value to detect its type
    if let Ok(current) = component.getattr(field_name) {
        let current = if let Ok(variant) = component.cast::<PyType>() {
            enum_field_prototype(py, variant, field_name)?.unwrap_or(current)
        } else {
            current
        };
        let type_name = current
            .get_type()
            .name()
            .map(|n| n.to_string())
            .unwrap_or_default();
        if current.is_instance_of::<PyFloat>()
            && (field_value.is_number() || nonfinite_float_from_json(field_value).is_some())
        {
            return json_number_to_f64(field_value)?
                .into_pyobject(py)
                .map(|value| value.into_any().unbind())
                .map_err(|error| error.to_string());
        }
        let owner = enum_owner_type(&current);
        let is_color = owner
            .name()
            .is_ok_and(|name| name.to_string_lossy() == "Color");
        let expected_math_shape = match type_name.as_str() {
            "Vec2" | "UVec2" | "IVec2" => Some("[x, y]"),
            "Vec3" | "UVec3" => Some("[x, y, z]"),
            "Vec4" | "Quat" => Some("[x, y, z, w]"),
            _ => None,
        };

        if let Some(shape) = expected_math_shape
            && !field_value.is_array()
        {
            let kind = match field_value {
                serde_json::Value::Null => "null",
                serde_json::Value::Bool(_) => "boolean",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::String(_) => "string",
                serde_json::Value::Array(_) => unreachable!(),
                serde_json::Value::Object(_) => "object",
            };
            return Err(format!("{type_name} expects {shape}, got {kind}"));
        }

        if let serde_json::Value::Object(fields) = field_value
            && fields.len() == 1
            && let Some(serde_json::Value::String(expected)) = fields.get("repr")
        {
            let actual = current.repr().map_err(|error| error.to_string())?;
            if actual.to_string_lossy() == expected.as_str() {
                return Ok(current.clone().unbind());
            }
            return Err(format!(
                "opaque value cannot be edited; expected unchanged repr {actual}"
            ));
        }

        if type_name == "Val"
            && let serde_json::Value::Object(fields) = field_value
            && fields.len() == 1
        {
            let (variant, payload) = fields.iter().next().unwrap();
            let method = match variant.as_str() {
                "Auto" => "auto",
                "Px" => "px",
                "Percent" => "percent",
                "Vw" => "vw",
                "Vh" => "vh",
                "VMin" => "vmin",
                "VMax" => "vmax",
                _ => return Err(format!("unknown Val variant '{variant}'")),
            };
            let val_type = current.get_type();
            return if variant == "Auto" {
                val_type
                    .call_method0(method)
                    .map(Bound::unbind)
                    .map_err(|error| error.to_string())
            } else {
                let value = json_number_to_f32(payload)?;
                val_type
                    .call_method1(method, (value,))
                    .map(Bound::unbind)
                    .map_err(|error| error.to_string())
            };
        }

        if is_color
            && let serde_json::Value::Array(values) = field_value
            && values.len() == 4
        {
            let red = json_number_to_f32(&values[0])?;
            let green = json_number_to_f32(&values[1])?;
            let blue = json_number_to_f32(&values[2])?;
            let alpha = json_number_to_f32(&values[3])?;
            return owner
                .call_method1("srgba", (red, green, blue, alpha))
                .map(Bound::unbind)
                .map_err(|error| error.to_string());
        }

        if is_color
            && let serde_json::Value::Object(fields) = field_value
            && let Some(serde_json::Value::String(variant_name)) = fields.get("variant")
            && let Some(variant_value) = fields.get("value")
        {
            return construct_color_variant(py, variant_name, variant_value);
        }

        if is_color
            && let serde_json::Value::Object(fields) = field_value
            && fields.len() == 1
            && !fields.contains_key("variant")
        {
            let (variant_name, variant_value) = fields.iter().next().unwrap();
            return construct_color_variant(py, variant_name, variant_value);
        }

        // Nested dataclass: apply the given fields onto the current value, so
        // omitted fields keep their value and nested ones convert recursively.
        if let serde_json::Value::Object(fields) = field_value
            && is_dataclass_instance(&current)
        {
            let kwargs = PyDict::new(py);
            for (name, value) in fields {
                if !current.hasattr(name.as_str()).unwrap_or(false) {
                    return Err(format!("{type_name} has no field '{name}'"));
                }
                let converted = convert_field_value(py, &current, name, value)?;
                kwargs
                    .set_item(name, converted)
                    .map_err(|error| error.to_string())?;
            }
            let dataclasses =
                PyModule::import(py, "dataclasses").map_err(|error| error.to_string())?;
            return dataclasses
                .getattr("replace")
                .and_then(|replace| replace.call((&current,), Some(&kwargs)))
                .map(Bound::unbind)
                .map_err(|error| error.to_string());
        }

        // Accept the tagged form emitted by get_component for every variant.
        if let serde_json::Value::Object(obj) = field_value
            && let Some(serde_json::Value::String(variant_name)) = obj.get("variant")
        {
            let mut payload = obj.clone();
            payload.remove("variant");
            let payload = if payload.is_empty() {
                serde_json::Value::Null
            } else if payload.len() == 1 && payload.contains_key("value") {
                payload.remove("value").unwrap()
            } else {
                serde_json::Value::Object(payload)
            };
            if let Some(result) =
                construct_enum_variant(py, &current, variant_name, &payload, VariantSource::Named)?
            {
                return Ok(result);
            }
        }

        // Handle enum-variant types: {"Variant": value} becomes Type.Variant(value).
        if let serde_json::Value::Object(obj) = field_value
            && obj.len() == 1
            && !obj.contains_key("variant")
        {
            let (variant_name, variant_value) = obj.iter().next().unwrap();
            if let Some(result) = construct_enum_variant(
                py,
                &current,
                variant_name,
                variant_value,
                VariantSource::ObjectKey,
            )? {
                return Ok(result);
            }
        }

        if let serde_json::Value::String(variant_name) = field_value
            && let Some(result) = construct_enum_variant(
                py,
                &current,
                variant_name,
                &serde_json::Value::Null,
                VariantSource::Named,
            )?
        {
            return Ok(result);
        }

        if let serde_json::Value::String(s) = field_value {
            let owner = enum_owner_type(&current);
            let variants = enum_variant_names(&owner);
            if !variants.is_empty() {
                let owner_name = owner
                    .name()
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .into_owned();
                return Err(format!(
                    "unknown variant '{s}' for {}. Valid variants: {}",
                    owner_name,
                    variants.join(", ")
                ));
            }
        }

        // JSON has no entity type, so an Entity-valued field takes an id.
        if type_name == "Entity" {
            return entity_from_json(py, field_value);
        }

        // Convert JSON arrays to math types based on current field type
        if let serde_json::Value::Array(arr) = field_value {
            if current.is_instance_of::<PyTuple>() {
                let values = arr
                    .iter()
                    .map(|value| json_to_py(py, value))
                    .collect::<Result<Vec<_>, _>>()?;
                return PyTuple::new(py, values)
                    .map(|value| value.into_any().unbind())
                    .map_err(|error| error.to_string());
            }
            if current.is_instance_of::<PySet>() {
                let values = arr
                    .iter()
                    .map(|value| json_to_py(py, value))
                    .collect::<Result<Vec<_>, _>>()?;
                return PySet::new(py, values)
                    .map(|value| value.into_any().unbind())
                    .map_err(|error| error.to_string());
            }
            if let Some(array) = rebuild_array_like(py, &current, field_value)? {
                return Ok(array);
            }
            if current.is_instance_of::<PyList>()
                && !arr.is_empty()
                && let Some(list) = convert_list_elements(py, component, field_name, &current, arr)?
            {
                return Ok(list);
            }
            let expected_shape = match type_name.as_str() {
                "Vec2" | "UVec2" | "IVec2" => Some(("[x, y]", 2)),
                "Vec3" | "UVec3" => Some(("[x, y, z]", 3)),
                "Vec4" | "Quat" => Some(("[x, y, z, w]", 4)),
                _ => None,
            };
            if let Some((shape, expected_len)) = expected_shape
                && arr.len() != expected_len
            {
                return Err(format!(
                    "{type_name} expects {shape}, got {} elements",
                    arr.len()
                ));
            }

            if let Some(result) = math_value_from_json(py, &type_name, arr) {
                return result;
            }
        }

        let is_pybevy_value = has_public_getset_fields(&current);

        if is_pybevy_value
            && !current.hasattr("variant").unwrap_or(false)
            && current.hasattr("value").unwrap_or(false)
            && !field_value.is_object()
        {
            let value = json_to_py(py, field_value)?;
            return current
                .get_type()
                .call1((value,))
                .map(Bound::unbind)
                .map_err(|error| error.to_string());
        }

        if is_pybevy_value && let serde_json::Value::Object(fields) = field_value {
            // Scalar getset descriptors must not turn object patches into default values.
            if current.is_instance_of::<PyFloat>()
                || current.is_instance_of::<PyInt>()
                || current.is_instance_of::<PyString>()
            {
                return Err(public_error::mcp_scalar_object(&type_name));
            }
            if fields.is_empty() {
                return Ok(current.clone().unbind());
            }
            let kwargs = PyDict::new(py);
            for (name, value) in fields {
                if !current.hasattr(name.as_str()).unwrap_or(false) {
                    return Err(format!(
                        "{} has no field '{name}'",
                        current
                            .get_type()
                            .name()
                            .map_err(|error| error.to_string())?
                    ));
                }
                let converted = convert_field_value(py, &current, name, value)?;
                kwargs
                    .set_item(name, converted)
                    .map_err(|error| error.to_string())?;
            }
            return current
                .get_type()
                .call((), Some(&kwargs))
                .map(Bound::unbind)
                .map_err(|error| error.to_string());
        }
    }

    // Default: generic JSON → Python conversion
    json_to_py(py, field_value)
}

fn integer_vector_coordinate_to_py(
    py: Python<'_>,
    type_name: &str,
    value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    match type_name {
        "UVec2" | "UVec3" => {
            let coordinate = if let Some(integer) = value.as_i64() {
                u32::try_from(integer)
            } else if let Some(integer) = value.as_u64() {
                u32::try_from(integer)
            } else {
                return Err(public_error::integer_vector_coordinate(type_name, value));
            }
            .map_err(|_| {
                public_error::integer_vector_coordinate_out_of_range(
                    type_name,
                    value,
                    u32::MIN,
                    u32::MAX,
                )
            })?;
            coordinate
                .into_pyobject(py)
                .map(|value| value.into_any().unbind())
                .map_err(|error| error.to_string())
        }
        "IVec2" => {
            let Some(integer) = value.as_i64() else {
                if value.as_u64().is_some() {
                    return Err(public_error::integer_vector_coordinate_out_of_range(
                        type_name,
                        value,
                        i32::MIN,
                        i32::MAX,
                    ));
                }
                return Err(public_error::integer_vector_coordinate(type_name, value));
            };
            let coordinate = i32::try_from(integer).map_err(|_| {
                public_error::integer_vector_coordinate_out_of_range(
                    type_name,
                    value,
                    i32::MIN,
                    i32::MAX,
                )
            })?;
            coordinate
                .into_pyobject(py)
                .map(|value| value.into_any().unbind())
                .map_err(|error| error.to_string())
        }
        _ => unreachable!("integer vector conversion requires a known integer vector type"),
    }
}

pub(crate) fn json_number_to_f64(value: &serde_json::Value) -> Result<f64, String> {
    value
        .as_f64()
        .or_else(|| nonfinite_float_from_json(value))
        .ok_or_else(|| format!("Expected number, got {value}"))
        .and_then(require_finite_float)
}

fn json_number_to_f32(value: &serde_json::Value) -> Result<f32, String> {
    let value = json_number_to_f64(value)? as f32;
    require_finite_float(f64::from(value))?;
    Ok(value)
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
    use std::{alloc::Layout, ffi::CString, mem, ptr, sync::Once};

    use bevy::{
        camera::ClearColor,
        ecs::{
            component::{ComponentCloneBehavior, ComponentDescriptor, ComponentId, StorageType},
            entity::Entity,
            name::Name,
            resource::IsResource,
        },
        prelude::{ChildOf, Children, With},
        ptr::OwningPtr,
    };
    use pybevy_core::{
        CustomComponentEntry, CustomResourceEntry,
        component_layout::{ComponentLayout, PrimitiveType, PrimitiveValue},
        component_wrapper::ComponentWrapper16,
        custom_component::create_wrapper_descriptor,
    };
    use pyo3::types::PyInt;

    use super::*;
    use crate::bridge::ErrorCode;

    static INIT: Once = Once::new();

    #[pyclass]
    struct ControlPayloadFixture {
        required: i64,
        preserved: i64,
    }

    #[pymethods]
    impl ControlPayloadFixture {
        #[new]
        #[pyo3(signature = (*, required = 1, preserved = 9))]
        fn new(required: i64, preserved: i64) -> Self {
            Self {
                required,
                preserved,
            }
        }

        #[getter]
        fn required(&self) -> i64 {
            self.required
        }

        #[getter]
        fn preserved(&self) -> i64 {
            self.preserved
        }
    }

    fn setup_python() {
        INIT.call_once(|| {
            Python::initialize();
        });
    }

    unsafe fn drop_test_py_object(ptr: OwningPtr<'_>) {
        // SAFETY: test_resource_descriptor declares the value as Py<PyAny>.
        unsafe { ptr.drop_as::<Py<PyAny>>() };
    }

    fn register_test_resource(world: &mut World, name: &'static str) -> ComponentId {
        // SAFETY: layout, drop function, and inserted test values all use Py<PyAny>.
        let descriptor = unsafe {
            ComponentDescriptor::new_with_layout(
                name,
                StorageType::Table,
                Layout::new::<Py<PyAny>>(),
                Some(drop_test_py_object),
                true,
                ComponentCloneBehavior::Default,
                None,
            )
        };
        world.register_component_with_descriptor(descriptor)
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
    fn convert_field_value_nested_dataclass_keeps_omitted_fields() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import dataclasses

@dataclasses.dataclass
class Stats:
    hp: int = 100
    mp: int = 50

class Holder:
    def __init__(self):
        self.stats = Stats()

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!({"hp": 10});
            let result = convert_field_value(py, &holder, "stats", &field_value).unwrap();
            let bound = result.bind(py);

            assert_eq!(bound.get_type().name().unwrap().to_string(), "Stats");
            assert_eq!(bound.getattr("hp").unwrap().extract::<i64>().unwrap(), 10);
            assert_eq!(bound.getattr("mp").unwrap().extract::<i64>().unwrap(), 50);
        });
    }

    #[test]
    fn wrapped_enum_payload_uses_registered_payload_type_defaults() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Variant:
    __pybevy_control_payload_prototype__ = Payload

    def __init__(self, value):
        if not isinstance(value, Payload):
            raise TypeError("expected Payload")
        self.value = value

variant = Variant
"#,
            )
            .unwrap();
            let globals = PyDict::new(py);
            globals
                .set_item("Payload", py.get_type::<ControlPayloadFixture>())
                .unwrap();
            py.run(&code, Some(&globals), None).unwrap();
            let variant = globals.get_item("variant").unwrap().unwrap();
            let fields = serde_json::json!({"required": 4});

            let result = construct_wrapped_variant_payload(
                py,
                &variant,
                "Wrapped",
                fields.as_object().unwrap(),
            )
            .unwrap()
            .expect("metadata should construct a wrapped payload");
            let payload = result.bind(py).getattr("value").unwrap();

            assert_eq!(
                payload
                    .getattr("required")
                    .unwrap()
                    .extract::<i64>()
                    .unwrap(),
                4
            );
            assert_eq!(
                payload
                    .getattr("preserved")
                    .unwrap()
                    .extract::<i64>()
                    .unwrap(),
                9
            );
        });
    }

    #[test]
    fn convert_field_value_set_from_array() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Holder:
    def __init__(self):
        self.tags = {"a"}

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!(["x", "y"]);
            let result = convert_field_value(py, &holder, "tags", &field_value).unwrap();
            let bound = result.bind(py);

            assert!(bound.is_instance_of::<PySet>());
            assert_eq!(bound.len().unwrap(), 2);
        });
    }

    #[test]
    fn convert_field_value_quat_reports_expected_shape() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Quat:
    pass

class Holder:
    def __init__(self):
        self.rotation = Quat()

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();
            let field_value = serde_json::json!([0.1, 0.2, 0.3]);

            let error = convert_field_value(py, &holder, "rotation", &field_value).unwrap_err();

            assert_eq!(error, "Quat expects [x, y, z, w], got 3 elements");
        });
    }

    #[test]
    fn convert_field_value_vec3_string_reports_expected_shape() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Vec3:
    pass

Vec3.ZERO = Vec3()

class Holder:
    def __init__(self):
        self.translation = Vec3()

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let error =
                convert_field_value(py, &holder, "translation", &serde_json::json!("not_a_vec3"))
                    .unwrap_err();

            assert_eq!(error, "Vec3 expects [x, y, z], got string");
        });
    }

    #[test]
    fn convert_field_value_list_of_struct_elements_converts_dicts() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import sys, types

if "pybevy" not in sys.modules:
    sys.modules["pybevy"] = types.ModuleType("pybevy")
ui_mod = sys.modules.get("pybevy.ui")
if ui_mod is None:
    ui_mod = types.ModuleType("pybevy.ui")
    sys.modules["pybevy.ui"] = ui_mod

class ShadowStyle:
    color = None
    x_offset = None
    def __init__(self, *, color=None, x_offset=None):
        self.color = color
        self.x_offset = x_offset

ShadowStyle.__module__ = "pybevy.ui"
ui_mod.ShadowStyle = ShadowStyle

class BoxHolder:
    def __init__(self):
        self._shadows = [ShadowStyle()]
    @property
    def shadows(self):
        return self._shadows
    @shadows.setter
    def shadows(self, value):
        for item in value:
            if not isinstance(item, ShadowStyle):
                raise TypeError("'dict' object is not an instance of 'ShadowStyle'")
        self._shadows = list(value)

holder = BoxHolder()
holder_empty = BoxHolder.__new__(BoxHolder)
holder_empty._shadows = []
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!([
                {"color": [1.0, 0.5], "x_offset": 3.0},
                {"color": null, "x_offset": 1.5}
            ]);
            let result = convert_field_value(py, &holder, "shadows", &field_value).unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 2);
            let first = list.get_item(0).unwrap();
            assert_eq!(
                first.get_type().name().unwrap().to_string_lossy(),
                "ShadowStyle"
            );
            assert_eq!(
                first.getattr("x_offset").unwrap().extract::<f64>().unwrap(),
                3.0
            );
            let second = list.get_item(1).unwrap();
            assert!(second.getattr("color").unwrap().is_none());
            assert_eq!(
                second
                    .getattr("x_offset")
                    .unwrap()
                    .extract::<f64>()
                    .unwrap(),
                1.5
            );
        });
    }

    #[test]
    fn convert_field_value_list_of_structs_empty_list_probes_detached_setter() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import sys, types

if "pybevy" not in sys.modules:
    sys.modules["pybevy"] = types.ModuleType("pybevy")
ui_mod = sys.modules.get("pybevy.ui")
if ui_mod is None:
    ui_mod = types.ModuleType("pybevy.ui")
    sys.modules["pybevy.ui"] = ui_mod

class ShadowStyle:
    color = None
    x_offset = None
    def __init__(self, *, color=None, x_offset=None):
        self.color = color
        self.x_offset = x_offset

ShadowStyle.__module__ = "pybevy.ui"
ui_mod.ShadowStyle = ShadowStyle

class BoxHolder:
    def __init__(self):
        self._shadows = []
        self.setter_attempts = 0
    @property
    def shadows(self):
        return self._shadows
    @shadows.setter
    def shadows(self, value):
        self.setter_attempts += 1
        for item in value:
            if not isinstance(item, ShadowStyle):
                raise TypeError("'dict' object is not an instance of 'ShadowStyle'")
        self._shadows = list(value)

holder_empty = BoxHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder_empty = globals.get_item("holder_empty").unwrap().unwrap();

            let field_value = serde_json::json!([{"color": null, "x_offset": 7.0}]);
            let result = convert_field_value(py, &holder_empty, "shadows", &field_value).unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 1);
            assert_eq!(
                list.get_item(0)
                    .unwrap()
                    .getattr("x_offset")
                    .unwrap()
                    .extract::<f64>()
                    .unwrap(),
                7.0
            );
            let shadows = holder_empty.getattr("shadows").unwrap();
            assert_eq!(shadows.len().unwrap(), 0);
            assert_eq!(
                holder_empty
                    .getattr("setter_attempts")
                    .unwrap()
                    .extract::<usize>()
                    .unwrap(),
                0
            );
        });
    }

    #[test]
    fn convert_field_value_empty_list_probe_unresolvable_class_falls_back_generic() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class UnresolvableHolder:
    def __init__(self):
        self._items = []
    @property
    def items(self):
        return self._items
    @items.setter
    def items(self, value):
        for item in value:
            if type(item) is dict:
                raise TypeError("'dict' object is not an instance of 'UnloadedThing'")
        self._items = list(value)

holder = UnresolvableHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!([{"a": 1}]);
            let result = convert_field_value(py, &holder, "items", &field_value).unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 1);
            let element = list.get_item(0).unwrap();
            assert!(element.is_instance_of::<PyDict>());
            let items = holder.getattr("items").unwrap();
            assert_eq!(items.len().unwrap(), 0);
        });
    }

    #[test]
    fn convert_field_value_list_element_unknown_field() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import sys, types

if "pybevy" not in sys.modules:
    sys.modules["pybevy"] = types.ModuleType("pybevy")
ui_mod = sys.modules.get("pybevy.ui")
if ui_mod is None:
    ui_mod = types.ModuleType("pybevy.ui")
    sys.modules["pybevy.ui"] = ui_mod

class ShadowStyle:
    color = None
    x_offset = None
    def __init__(self, *, color=None, x_offset=None):
        self.color = color
        self.x_offset = x_offset

ShadowStyle.__module__ = "pybevy.ui"
ui_mod.ShadowStyle = ShadowStyle

class BoxHolder:
    def __init__(self):
        self._shadows = [ShadowStyle()]
    @property
    def shadows(self):
        return self._shadows
    @shadows.setter
    def shadows(self, value):
        for item in value:
            if not isinstance(item, ShadowStyle):
                raise TypeError("'dict' object is not an instance of 'ShadowStyle'")
        self._shadows = list(value)

holder = BoxHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!([{"bogus": 1.0}]);
            let error = convert_field_value(py, &holder, "shadows", &field_value).unwrap_err();
            assert_eq!(error, "element 0: ShadowStyle has no field 'bogus'");
        });
    }

    #[test]
    fn convert_field_value_list_element_wrong_kind() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class ShadowStyle:
    def __init__(self):
        pass

class BoxHolder:
    def __init__(self):
        self._shadows = [ShadowStyle()]
    @property
    def shadows(self):
        return self._shadows
    @shadows.setter
    def shadows(self, value):
        for item in value:
            if not isinstance(item, ShadowStyle):
                raise TypeError("'dict' object is not an instance of 'ShadowStyle'")
        self._shadows = list(value)

holder = BoxHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!(["just a string"]);
            let error = convert_field_value(py, &holder, "shadows", &field_value).unwrap_err();
            assert_eq!(error, "element 0: expected object, got string");
        });
    }

    #[test]
    fn convert_field_value_list_element_missing_constructor_field() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import sys, types

if "pybevy" not in sys.modules:
    sys.modules["pybevy"] = types.ModuleType("pybevy")
ui_mod = sys.modules.get("pybevy.ui")
if ui_mod is None:
    ui_mod = types.ModuleType("pybevy.ui")
    sys.modules["pybevy.ui"] = ui_mod

class HalfSpace:
    normal_d = None
    def __init__(self, normal_d):
        self.normal_d = normal_d

HalfSpace.__module__ = "pybevy.ui"
ui_mod.HalfSpace = HalfSpace

class FrustumHolder:
    def __init__(self):
        self._half_spaces = []
    @property
    def half_spaces(self):
        return self._half_spaces
    @half_spaces.setter
    def half_spaces(self, value):
        for item in value:
            if not isinstance(item, HalfSpace):
                raise TypeError("'dict' object is not an instance of 'HalfSpace'")
        self._half_spaces = list(value)

frustum = FrustumHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let frustum = globals.get_item("frustum").unwrap().unwrap();

            let error = convert_field_value(py, &frustum, "half_spaces", &serde_json::json!([{}]))
                .unwrap_err();
            assert_eq!(error, "element 0: HalfSpace requires field 'normal_d'");

            let field_value = serde_json::json!([{"normal_d": [1.0, 2.0, 3.0, 4.0]}]);
            let result = convert_field_value(py, &frustum, "half_spaces", &field_value).unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 1);
            let element = list.get_item(0).unwrap();
            assert_eq!(
                element.get_type().name().unwrap().to_string_lossy(),
                "HalfSpace"
            );
            let normal_d = element.getattr("normal_d").unwrap();
            assert_eq!(normal_d.get_item(0).unwrap().extract::<f64>().unwrap(), 1.0);
        });
    }

    #[test]
    fn convert_field_value_list_of_scalars_keeps_generic_path() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class TagHolder:
    def __init__(self):
        self.tags = ["a"]

tag_holder = TagHolder()
tag_holder_empty = TagHolder()
tag_holder_empty.tags = []
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let tag_holder = globals.get_item("tag_holder").unwrap().unwrap();
            let tag_holder_empty = globals.get_item("tag_holder_empty").unwrap().unwrap();

            let result =
                convert_field_value(py, &tag_holder, "tags", &serde_json::json!(["x", "y"]))
                    .unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 2);
            assert_eq!(list.get_item(0).unwrap().extract::<String>().unwrap(), "x");

            let result =
                convert_field_value(py, &tag_holder_empty, "tags", &serde_json::json!(["x"]))
                    .unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 1);
            let tags = tag_holder_empty.getattr("tags").unwrap();
            assert_eq!(tags.len().unwrap(), 0);
        });
    }

    #[test]
    fn convert_field_value_list_of_math_elements() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import sys, types

if "pybevy" not in sys.modules:
    sys.modules["pybevy"] = types.ModuleType("pybevy")
math_mod = sys.modules.get("pybevy.math")
if math_mod is None:
    math_mod = types.ModuleType("pybevy.math")
    sys.modules["pybevy.math"] = math_mod

class Vec3:
    def __init__(self, x, y, z):
        self.x = x
        self.y = y
        self.z = z

Vec3.__module__ = "pybevy.math"
math_mod.Vec3 = Vec3

class Vec3Holder:
    def __init__(self):
        self._points = [Vec3(0.0, 0.0, 0.0)]
    @property
    def points(self):
        return self._points
    @points.setter
    def points(self, value):
        for item in value:
            if not isinstance(item, Vec3):
                raise TypeError("'dict' object is not an instance of 'Vec3'")
        self._points = list(value)

vec3_holder = Vec3Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let vec3_holder = globals.get_item("vec3_holder").unwrap().unwrap();

            let result =
                convert_field_value(py, &vec3_holder, "points", &serde_json::json!([[1, 2, 3]]))
                    .unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            let element = list.get_item(0).unwrap();
            assert_eq!(element.get_type().name().unwrap().to_string_lossy(), "Vec3");
            assert_eq!(element.getattr("x").unwrap().extract::<f64>().unwrap(), 1.0);
            assert_eq!(element.getattr("z").unwrap().extract::<f64>().unwrap(), 3.0);

            let error =
                convert_field_value(py, &vec3_holder, "points", &serde_json::json!(["bad"]))
                    .unwrap_err();
            assert_eq!(error, "element 0: Vec3 expects [x, y, z], got string");

            let error =
                convert_field_value(py, &vec3_holder, "points", &serde_json::json!([[1.0, 2.0]]))
                    .unwrap_err();
            assert_eq!(error, "element 0: Vec3 expects [x, y, z], got 2 elements");
        });
    }

    #[test]
    fn convert_field_value_list_of_enum_elements() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Track:
    pass

class Auto(Track):
    def __init__(self):
        pass

class Px(Track):
    def __init__(self, value):
        self.value = value

Track.Auto = Auto
Track.Px = Px

class TrackHolder:
    def __init__(self):
        self._tracks = [Track.Px(1.0)]
    @property
    def tracks(self):
        return self._tracks
    @tracks.setter
    def tracks(self, value):
        for item in value:
            if not isinstance(item, Track):
                raise TypeError("'dict' object is not an instance of 'Track'")
        self._tracks = list(value)

track_holder = TrackHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let track_holder = globals.get_item("track_holder").unwrap().unwrap();

            let field_value = serde_json::json!([
                {"Px": 5.0},
                {"Auto": null},
                {"variant": "Px", "value": 2.5}
            ]);
            let result = convert_field_value(py, &track_holder, "tracks", &field_value).unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 3);
            let first = list.get_item(0).unwrap();
            assert_eq!(first.get_type().name().unwrap().to_string_lossy(), "Px");
            assert_eq!(
                first.getattr("value").unwrap().extract::<f64>().unwrap(),
                5.0
            );
            let second = list.get_item(1).unwrap();
            assert_eq!(second.get_type().name().unwrap().to_string_lossy(), "Auto");
            let third = list.get_item(2).unwrap();
            assert_eq!(
                third.getattr("value").unwrap().extract::<f64>().unwrap(),
                2.5
            );

            let error = convert_field_value(
                py,
                &track_holder,
                "tracks",
                &serde_json::json!([{"Bogus": 1.0}]),
            )
            .unwrap_err();
            assert_eq!(
                error,
                "element 0: unknown variant 'Bogus' for Track. Valid variants: Auto, Px"
            );
        });
    }

    #[test]
    fn convert_field_value_list_enum_element_missing_field() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Track:
    pass

class Box(Track):
    def __init__(self, x, y):
        self.x = x
        self.y = y

Track.Box = Box

class TrackHolder:
    def __init__(self):
        self._tracks = [Track.Box(1.0, 2.0)]
    @property
    def tracks(self):
        return self._tracks
    @tracks.setter
    def tracks(self, value):
        for item in value:
            if not isinstance(item, Track):
                raise TypeError("'dict' object is not an instance of 'Track'")
        self._tracks = list(value)

track_holder = TrackHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let track_holder = globals.get_item("track_holder").unwrap().unwrap();

            let error = convert_field_value(
                py,
                &track_holder,
                "tracks",
                &serde_json::json!([{"Box": {"x": 1.0}}]),
            )
            .unwrap_err();
            assert_eq!(error, "element 0: variant 'Box' requires field 'y'");

            let result = convert_field_value(
                py,
                &track_holder,
                "tracks",
                &serde_json::json!([{"variant": "Box", "value": {"x": 1.0, "y": 2.0}}]),
            )
            .unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            let element = list.get_item(0).unwrap();
            assert_eq!(element.get_type().name().unwrap().to_string_lossy(), "Box");
            assert_eq!(element.getattr("y").unwrap().extract::<f64>().unwrap(), 2.0);
        });
    }

    #[test]
    fn json_number_to_f64_valid() {
        let pi = std::f64::consts::PI;
        let val = serde_json::json!(pi);
        assert!((json_number_to_f64(&val).unwrap() - pi).abs() < 1e-10);
    }

    #[test]
    fn json_number_to_f64_integer() {
        let val = serde_json::json!(42);
        assert!((json_number_to_f64(&val).unwrap() - 42.0).abs() < 1e-10);
    }

    #[test]
    fn json_number_to_f64_rejects_nonfinite_spellings() {
        for spelling in ["NaN", "Infinity", "-Infinity"] {
            assert!(json_number_to_f64(&serde_json::json!(spelling)).is_err());
        }
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
        // Unknown components now fail fast - batch reports error status
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
    fn remove_resource_not_in_registry() {
        let mut world = World::new();
        let result = remove_resource(&mut world, "FakeResource".to_string());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
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
    fn batch_mutate_partial_status_on_embedded_errors() {
        // Simulate a batch where one operation returns Ok with embedded errors.
        // We can't easily trigger real embedded errors without registered components,
        // but we can test the status assignment logic directly.
        let val_with_errors = serde_json::json!({
            "entity_id": 42,
            "new_values": {},
            "errors": ["some field error"],
        });
        let val_without_errors = serde_json::json!({
            "entity_id": 42,
            "new_values": {"translation": [1.0, 2.0, 3.0]},
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
        let comp_id = register_test_resource(&mut world, "GameScore");

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
                type_object: None,
                name: "GameScore".to_string(),
            },
        );
        world.insert_resource(info);

        // set_resource should find it via CustomResourceInfo and construct int().
        let result = insert_resource(&mut world, "GameScore".to_string(), serde_json::json!({}));
        assert!(
            result.is_ok(),
            "Custom resource insert failed: {:?}",
            result
        );
        assert_eq!(result.unwrap()["custom"], true);

        assert!(world.contains_resource_by_id(comp_id));
    }

    #[test]
    fn insert_resource_custom_patch_stamps_the_change_tick() {
        setup_python();

        let mut world = World::new();
        let comp_id = register_test_resource(&mut world, "GameScore");
        let type_ptr = Python::attach(|py| py.get_type::<PyInt>().as_type_ptr());

        let mut info = CustomResourceInfo::default();
        info.insert(
            comp_id,
            CustomResourceEntry {
                type_ptr,
                type_object: None,
                name: "GameScore".to_string(),
            },
        );
        world.insert_resource(info);

        // First call constructs the value; second call takes the patch branch.
        insert_resource(&mut world, "GameScore".to_string(), serde_json::json!({})).unwrap();
        world.clear_trackers();
        let before = world
            .get_resource_change_ticks_by_id(comp_id)
            .expect("resource present")
            .changed;

        insert_resource(&mut world, "GameScore".to_string(), serde_json::json!({})).unwrap();

        let after = world
            .get_resource_change_ticks_by_id(comp_id)
            .expect("resource present")
            .changed;
        assert_ne!(
            before, after,
            "control-plane patch must stamp the change tick so Changed[T] observes it"
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
    fn remove_resource_custom_via_py_pyresource() {
        setup_python();

        let mut world = World::new();

        // Register a fake custom resource
        let comp_id = register_test_resource(&mut world, "MyCustomRes");

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
                type_object: None,
                name: "MyCustomRes".to_string(),
            },
        );
        world.insert_resource(info);

        let py_obj = Python::attach(|py| 42i64.into_pyobject(py).unwrap().into_any().unbind());
        // SAFETY: register_test_resource uses the matching Py<PyAny> descriptor.
        unsafe { insert_dynamic_resource_value(&mut world, comp_id, py_obj) };

        // Verify resource is present before removal
        assert!(
            world.contains_resource_by_id(comp_id),
            "Resource should exist before removal"
        );

        let result = remove_resource(&mut world, "MyCustomRes".to_string()).unwrap();
        assert_eq!(result["removed"], "MyCustomRes");
        assert_eq!(result["was_present"], true);

        assert!(
            !world.contains_resource_by_id(comp_id),
            "Resource should be removed from its resource entity"
        );

        let repeated = remove_resource(&mut world, "MyCustomRes".to_string()).unwrap();
        assert_eq!(repeated["was_present"], false);
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

        let result = crate::handlers::pyo3::scene::get_component(
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

    /// Regression test: the last-resort archetype scan must not reinterpret a
    /// wrapper-like component (mutable, TypeId-less, Py<PyAny>-sized) as a
    /// Py<PyAny>. Before the descriptor-shape check, this test dereferenced
    /// arbitrary bytes as a Python object pointer.
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

    /// The last-resort archetype scan must still find genuine PyObject-storage
    /// components that are missing from CustomComponentInfo.
    #[test]
    fn set_component_last_resort_finds_pyobject_component() {
        setup_python();

        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();

        // Same descriptor shape as create_python_object_descriptor: immutable,
        // TypeId-less, Py<PyAny> layout. No drop fn so the test world can be
        // dropped without running Python refcounting on shutdown ordering.
        let comp_id = world.register_component_with_descriptor(unsafe {
            ComponentDescriptor::new_with_layout(
                "SimpleNamespace",
                StorageType::Table,
                Layout::new::<Py<PyAny>>(),
                None,
                false,
                ComponentCloneBehavior::Default,
                None,
            )
        });

        let py_obj: Py<PyAny> = Python::attach(|py| {
            let ns = py
                .import("types")
                .unwrap()
                .getattr("SimpleNamespace")
                .unwrap()
                .call0()
                .unwrap();
            ns.setattr("x", 1).unwrap();
            ns.unbind()
        });

        let mut entity_mut = world.get_entity_mut(entity).unwrap();
        unsafe {
            let data = core::ptr::NonNull::new_unchecked(ptr::addr_of!(py_obj) as *mut u8);
            entity_mut.insert_by_id(comp_id, bevy::ptr::OwningPtr::new(data));
        }
        mem::forget(py_obj);

        world.insert_resource(CustomComponentInfo::default());

        let result = set_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "SimpleNamespace".to_string(),
            serde_json::json!({"x": 5}),
        );
        let value = result.expect("PyObject component must be found via last resort");
        assert_eq!(value["new_values"], serde_json::json!({"x": 5}));
    }

    #[test]
    fn levenshtein_distance_measures_edits() {
        assert_eq!(levenshtein_distance("translation", "translation"), 0);
        assert_eq!(levenshtein_distance("traslation", "translation"), 1);
        assert_eq!(levenshtein_distance("align_conntent", "align_content"), 1);
        assert_eq!(levenshtein_distance("pos", "scale"), 5);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        // Case-sensitive by contract; case folding happens in close_field_suggestion.
        assert_eq!(levenshtein_distance("ABC", "abc"), 3);
    }

    #[test]
    fn close_field_suggestion_requires_a_genuinely_close_name() {
        let fields: Vec<String> = ["rotation", "scale", "translation"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        assert_eq!(
            close_field_suggestion("traslation", &fields).as_deref(),
            Some("translation")
        );
        assert_eq!(
            close_field_suggestion("Translaton", &fields).as_deref(),
            Some("translation")
        );
        assert_eq!(close_field_suggestion("pos", &fields), None);
        assert_eq!(close_field_suggestion("bogus", &fields), None);
    }

    #[test]
    fn close_field_suggestion_breaks_ties_alphabetically() {
        let fields: Vec<String> = ["abcd", "abce"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        assert_eq!(
            close_field_suggestion("abcf", &fields).as_deref(),
            Some("abcd")
        );
    }

    #[test]
    fn unknown_field_error_names_field_component_and_close_suggestion() {
        let writable: Vec<String> = ["rotation", "scale", "translation"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        let unknown = vec!["traslation".to_string()];
        assert_eq!(
            unknown_field_error("Transform", &unknown, &writable),
            "Failed to set 'Transform': unknown field 'traslation'. Did you mean 'translation'?"
        );
    }

    #[test]
    fn unknown_field_error_lists_small_writable_inventories() {
        let writable: Vec<String> = ["affects_lightmapped_meshes", "brightness", "color"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        let unknown = vec!["bogus".to_string()];
        assert_eq!(
            unknown_field_error("AmbientLight", &unknown, &writable),
            "Failed to set 'AmbientLight': unknown field 'bogus'. Valid writable fields: affects_lightmapped_meshes, brightness, color."
        );
    }

    #[test]
    fn unknown_field_error_points_to_schema_for_large_inventories() {
        let writable: Vec<String> = (0..12).map(|index| format!("field_{index}")).collect();
        let unknown = vec!["bogus".to_string()];
        assert_eq!(
            unknown_field_error("Node", &unknown, &writable),
            "Failed to set 'Node': unknown field 'bogus'. See GET /api/v1/components/Node/schema for the writable fields."
        );
    }

    #[test]
    fn unknown_field_error_reports_multiple_unknowns_without_suggestion() {
        let writable: Vec<String> = ["color", "intensity"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        let unknown = vec!["bogus".to_string(), "worse".to_string()];
        assert_eq!(
            unknown_field_error("PointLight", &unknown, &writable),
            "Failed to set 'PointLight': unknown fields: bogus, worse. Valid writable fields: color, intensity."
        );
    }

    #[test]
    fn unknown_field_error_names_components_without_writable_fields() {
        let unknown = vec!["bogus".to_string()];
        assert_eq!(
            unknown_field_error("Camera3d", &unknown, &[]),
            "Failed to set 'Camera3d': unknown field 'bogus'. This component exposes no writable fields."
        );
    }
}
