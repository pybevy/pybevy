use std::collections::HashMap;

use bevy::{
    ecs::{entity::Entity, hierarchy::ChildOf, name::Name, reflect::AppTypeRegistry, world::World},
    reflect::TypeInfo,
};
use pyo3::prelude::*;

use crate::bridge::{ControlError, EntityRef};

/// Extract custom component names for an entity using the CustomComponentInfo registry.
/// Returns a list of component names that are NOT covered by bridge components.
fn get_custom_component_names(world: &World, entity: Entity) -> Vec<String> {
    let Some(info) = world.get_resource::<pybevy_core::CustomComponentInfo>() else {
        return Vec::new();
    };

    let Ok(entity_ref) = world.get_entity(entity) else {
        return Vec::new();
    };

    let mut names = Vec::new();
    for component_id in entity_ref.archetype().components() {
        if let Some(entry) = info.get(*component_id) {
            names.push(entry.name.clone());
        }
    }
    names
}

/// Extract field values from a custom component stored as PyObject.
/// Returns a JSON map of field_name → field_repr.
fn extract_custom_component_fields(
    py: Python<'_>,
    entity_ref: &bevy::ecs::world::EntityRef,
    component_id: bevy::ecs::component::ComponentId,
    is_pyobject_storage: bool,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    if !is_pyobject_storage {
        // Non-PyObject storage: raw data is a Rust wrapper, not a Py<PyAny>.
        // Cannot safely read fields via Python introspection.
        let mut map = serde_json::Map::new();
        map.insert(
            "_note".into(),
            serde_json::Value::String(
                "wrapper storage — fields not readable via MCP. Use storage=\"python\" for introspectable components.".into(),
            ),
        );
        return Some(map);
    }

    let ptr = entity_ref.get_by_id(component_id).ok()?;

    // For PyObject storage, the raw data is a Py<PyAny>
    let py_obj: &pyo3::Py<PyAny> = unsafe { &*(ptr.as_ptr() as *const pyo3::Py<PyAny>) };
    let bound = py_obj.bind(py);

    let mut map = serde_json::Map::new();

    // Try dataclass fields first
    let fields_result = py
        .import("dataclasses")
        .and_then(|dc| dc.getattr("fields"))
        .and_then(|f| f.call1((bound,)));
    if let Ok(fields) = fields_result
        && let Ok(iter) = fields.try_iter()
    {
        for field in iter.flatten() {
            let name_value = field
                .getattr("name")
                .and_then(|n| n.extract::<String>())
                .ok()
                .and_then(|name| bound.getattr(name.as_str()).ok().map(|value| (name, value)));
            if let Some((name, value)) = name_value {
                let repr = value
                    .repr()
                    .map(|r| r.to_string())
                    .unwrap_or_else(|_| "<opaque>".to_string());
                map.insert(name, serde_json::Value::String(repr));
            }
        }
        if !map.is_empty() {
            return Some(map);
        }
    }

    // Fallback: try __dict__
    if let Ok(dict) = bound.getattr("__dict__") {
        #[allow(clippy::collapsible_if)]
        if let Ok(py_dict) = dict.cast::<pyo3::types::PyDict>() {
            for (key, value) in py_dict.iter() {
                if let Ok(k) = key.extract::<String>() {
                    if !k.starts_with('_') {
                        let repr = value
                            .repr()
                            .map(|r| r.to_string())
                            .unwrap_or_else(|_| "<opaque>".to_string());
                        map.insert(k, serde_json::Value::String(repr));
                    }
                }
            }
        }
    }

    if map.is_empty() { None } else { Some(map) }
}

/// Convert a Python value to a JSON value, recursing into nested PyO3 structs.
///
/// - For int/float/bool/str/None → direct JSON conversion
/// - For objects with getset_descriptor attributes (nested PyO3 structs) → recurse
/// - For list/tuple → recurse on elements
/// - Fallback → repr() string
fn py_value_to_json(value: &Bound<'_, PyAny>) -> serde_json::Value {
    // None
    if value.is_none() {
        return serde_json::Value::Null;
    }
    // bool (must check before int, since bool is a subclass of int in Python)
    if let Ok(b) = value.extract::<bool>() {
        return serde_json::Value::Bool(b);
    }
    // int
    if let Ok(i) = value.extract::<i64>() {
        return serde_json::json!(i);
    }
    // float
    if let Ok(f) = value.extract::<f64>() {
        return serde_json::json!(f);
    }
    // str
    if let Ok(s) = value.extract::<String>() {
        return serde_json::Value::String(s);
    }
    // list/tuple → recurse on elements
    if let Ok(iter) = value.try_iter() {
        let elements: Vec<serde_json::Value> = iter
            .filter_map(|item| item.ok())
            .map(|item| py_value_to_json(&item))
            .collect();
        return serde_json::Value::Array(elements);
    }
    // Check if this is a nested PyO3 struct (has getset_descriptor attributes)
    let nested_fields = extract_bridge_fields_inner(value);
    if !nested_fields.is_empty() {
        return serde_json::Value::Object(nested_fields);
    }
    // Fallback: repr()
    value
        .repr()
        .map(|r| serde_json::Value::String(r.to_string()))
        .unwrap_or_else(|_| serde_json::Value::String("<opaque>".to_string()))
}

/// Inner helper: extract getset_descriptor fields from a PyO3 object.
/// Returns an empty map if the object has no such fields.
fn extract_bridge_fields_inner(
    bound: &Bound<'_, PyAny>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    let py_type = bound.get_type();

    if let Ok(dir_list) = py_type.dir() {
        for attr_obj in dir_list.iter() {
            let Ok(name) = attr_obj.extract::<String>() else {
                continue;
            };
            if name.starts_with('_') {
                continue;
            }
            // Check if the class attribute is a getset_descriptor (PyO3 property)
            let Ok(class_attr) = py_type.as_any().getattr(name.as_str()) else {
                continue;
            };
            let type_name = class_attr
                .get_type()
                .name()
                .map(|n| n.to_string())
                .unwrap_or_default();
            if type_name != "getset_descriptor" {
                continue;
            }
            // Get the actual value from the instance and convert recursively
            if let Ok(value) = bound.getattr(name.as_str()) {
                map.insert(name, py_value_to_json(&value));
            }
        }
    }

    map
}

/// Check if a Python type has any writable getset_descriptor properties (non-underscore).
/// This detects types like Text2d that have settable PyO3 properties even when
/// Bevy's reflection says they are not editable (e.g. TupleStruct types).
fn has_writable_properties(py: Python<'_>, py_type: &Bound<'_, pyo3::types::PyType>) -> bool {
    let Ok(dir_list) = py_type.dir() else {
        return false;
    };
    for attr_obj in dir_list.iter() {
        let Ok(name) = attr_obj.extract::<String>() else {
            continue;
        };
        if name.starts_with('_') {
            continue;
        }
        let Ok(class_attr) = py_type.as_any().getattr(name.as_str()) else {
            continue;
        };
        let type_name = class_attr
            .get_type()
            .name()
            .map(|n| n.to_string())
            .unwrap_or_default();
        if type_name == "getset_descriptor" {
            // Check if the descriptor has fset (is writable)
            // PyO3 getset_descriptors with setters are writable
            if class_attr.getattr("fset").is_ok_and(|v| !v.is_none()) {
                return true;
            }
            // If fset check fails (some PyO3 versions), just having a getset_descriptor
            // with a non-underscore name is a good enough signal
            return true;
        }
    }
    let _ = py; // suppress unused warning
    false
}

/// Extract field values from a bridge (PyO3) component by iterating its `getset_descriptor` properties.
/// Returns a JSON map of field_name → value, recursing into nested structs.
fn extract_bridge_fields(
    _py: Python<'_>,
    bound: &Bound<'_, PyAny>,
) -> serde_json::Map<String, serde_json::Value> {
    extract_bridge_fields_inner(bound)
}

/// List all entities with their component types and Names
pub fn list_entities(world: &mut World) -> Result<serde_json::Value, ControlError> {
    let mut entities = Vec::new();
    let bridges = pybevy_core::registry::global_registry::all_component_bridges();

    let mut query_state = world.query::<Entity>();
    let entity_list: Vec<Entity> = query_state.iter(world).collect();

    for entity in &entity_list {
        let entity_id = entity.to_bits();
        let mut component_names = Vec::new();
        let mut entity_name: Option<String> = None;

        // Get Name component if present
        if let Ok(entity_ref) = world.get_entity(*entity) {
            if let Some(name) = entity_ref.get::<Name>() {
                entity_name = Some(name.as_str().to_string());
            }

            // Get all component types via bridge registry
            for bridge in &bridges {
                if bridge.entity_contains(&entity_ref) {
                    component_names.push(bridge.name().to_string());
                }
            }
        }

        // Add custom Python component names
        let custom_names = get_custom_component_names(world, *entity);
        component_names.extend(custom_names.iter().cloned());

        let label = super::spatial::entity_label(world, *entity);

        let mut entry = serde_json::json!({
            "id": entity_id,
            "name": entity_name,
            "label": label,
            "components": component_names,
        });

        // Report any remaining unknown components
        if let Ok(entity_ref) = world.get_entity(*entity) {
            let archetype_count = entity_ref.archetype().components().len();
            let known_count = component_names.len();
            let unknown_count = archetype_count.saturating_sub(known_count);
            if unknown_count > 0 {
                entry["unknown_component_count"] = serde_json::json!(unknown_count);
            }
        }
        if !custom_names.is_empty() {
            entry["custom_components"] = serde_json::json!(custom_names);
        }

        entities.push(entry);
    }

    Ok(serde_json::json!({
        "entity_count": entities.len(),
        "entities": entities,
    }))
}

/// Debug tool: get registry state and entity diagnostics
pub fn debug_registry(world: &mut World) -> Result<serde_json::Value, ControlError> {
    let bridges = pybevy_core::registry::global_registry::all_component_bridges();
    let resource_bridges = pybevy_core::registry::global_registry::all_resource_bridges();

    let bridge_names: Vec<String> = bridges.iter().map(|b| b.name().to_string()).collect();

    let mut query_state = world.query::<Entity>();
    let entity_count = query_state.iter(world).count();

    // Sample named entities (user's scene entities) for component detection
    let mut samples = Vec::new();
    let mut qs = world.query::<(Entity, &Name)>();
    let named_entities: Vec<(Entity, String)> = qs
        .iter(world)
        .take(10)
        .map(|(e, n)| (e, n.as_str().to_string()))
        .collect();
    for (entity, name) in &named_entities {
        if let Ok(entity_ref) = world.get_entity(*entity) {
            let detected: Vec<String> = bridges
                .iter()
                .filter(|b| b.entity_contains(&entity_ref))
                .map(|b| b.name().to_string())
                .collect();
            let archetype_components = entity_ref.archetype().components().len();

            let label = super::spatial::entity_label(world, *entity);
            samples.push(serde_json::json!({
                "id": entity.to_bits(),
                "name": name,
                "label": label,
                "detected_bridge_components": detected,
                "archetype_component_count": archetype_components,
            }));
        }
    }

    Ok(serde_json::json!({
        "component_bridge_count": bridges.len(),
        "component_bridge_names": bridge_names,
        "resource_bridge_count": resource_bridges.len(),
        "total_entities": entity_count,
        "entity_samples": samples,
    }))
}

/// Get detailed component values for a single entity
pub fn get_entity(
    world: &mut World,
    entity_ref: EntityRef,
) -> Result<serde_json::Value, ControlError> {
    let entity = resolve_entity(world, &entity_ref)?;
    let entity_id = entity.to_bits();

    let mut components = serde_json::Map::new();
    let mut entity_name: Option<String> = None;

    if let Ok(eref) = world.get_entity(entity)
        && let Some(name) = eref.get::<Name>() {
            entity_name = Some(name.as_str().to_string());
        }

    // Extract component values via Python
    Python::attach(|py| {
        let validity_flag = pybevy_core::ValidityFlag::new_read();
        let validity = validity_flag.with_access_mode(pybevy_core::AccessMode::Read);

        for bridge in pybevy_core::registry::global_registry::all_component_bridges() {
            if let Ok(entity_ref) = world.get_entity(entity)
                && bridge.entity_contains(&entity_ref) {
                    let name = bridge.name().to_string();
                    let value = bridge
                        .extract_from_entity_ref(&entity_ref, validity.clone(), py)
                        .ok()
                        .flatten()
                        .and_then(|py_obj| {
                            let bound = py_obj.bind(py);
                            bound.repr().ok().map(|r| r.to_string())
                        })
                        .unwrap_or_else(|| "<opaque>".to_string());

                    components.insert(name, serde_json::Value::String(value));
                }
        }

        validity_flag.set_invalid();
    });

    // Extract custom Python component data
    let mut custom_components = serde_json::Map::new();
    if let Some(info) = world.get_resource::<pybevy_core::CustomComponentInfo>()
        && let Ok(eref) = world.get_entity(entity) {
            for component_id in eref.archetype().components() {
                if let Some(entry) = info.get(*component_id) {
                    if entry.is_pyobject_storage {
                        // Extract field values for PyObject storage components
                        Python::attach(|py| {
                            if let Ok(eref2) = world.get_entity(entity) {
                                if let Some(fields) =
                                    extract_custom_component_fields(py, &eref2, *component_id, true)
                                {
                                    custom_components.insert(
                                        entry.name.clone(),
                                        serde_json::Value::Object(fields),
                                    );
                                } else {
                                    custom_components.insert(
                                        entry.name.clone(),
                                        serde_json::Value::String("<pyobject>".to_string()),
                                    );
                                }
                            }
                        });
                    } else {
                        // Wrapper storage - can report name but not fields
                        custom_components.insert(
                            entry.name.clone(),
                            serde_json::Value::String("<wrapper>".to_string()),
                        );
                    }
                }
            }
        }

    // Count remaining unknown components
    let mut unknown_count = 0;
    if let Ok(eref) = world.get_entity(entity) {
        let archetype_count = eref.archetype().components().len();
        let known_count = components.len() + custom_components.len();
        unknown_count = archetype_count.saturating_sub(known_count);
    }

    // Merge custom components into the components map
    for (k, v) in &custom_components {
        components.insert(k.clone(), v.clone());
    }

    let label = super::spatial::entity_label(world, entity);

    let mut result = serde_json::json!({
        "id": entity_id,
        "name": entity_name,
        "label": label,
        "components": components,
    });
    if !custom_components.is_empty() {
        result["custom_components"] =
            serde_json::json!(custom_components.keys().collect::<Vec<_>>());
    }
    if unknown_count > 0 {
        result["unknown_component_count"] = serde_json::json!(unknown_count);
    }
    Ok(result)
}

/// Get a single component's field values from an entity
pub fn get_component(
    world: &mut World,
    entity_ref: EntityRef,
    component: String,
) -> Result<serde_json::Value, ControlError> {
    let entity = resolve_entity(world, &entity_ref)?;
    let entity_id = entity.to_bits();
    let label = super::spatial::entity_label(world, entity);

    // Try bridge components first
    let bridge_result = Python::attach(|py| -> Option<serde_json::Value> {
        let validity_flag = pybevy_core::ValidityFlag::new_read();
        let validity = validity_flag.with_access_mode(pybevy_core::AccessMode::Read);

        for bridge in pybevy_core::registry::global_registry::all_component_bridges() {
            if bridge.name() != component {
                continue;
            }
            let Ok(eref) = world.get_entity(entity) else {
                return None;
            };
            if !bridge.entity_contains(&eref) {
                validity_flag.set_invalid();
                return None;
            }
            let fields = bridge
                .extract_from_entity_ref(&eref, validity.clone(), py)
                .ok()
                .flatten()
                .map(|py_obj| {
                    let bound = py_obj.bind(py);
                    extract_bridge_fields(py, bound)
                });

            validity_flag.set_invalid();
            return Some(serde_json::json!({
                "entity_id": entity_id,
                "entity_label": &label,
                "component": component,
                "fields": fields,
            }));
        }

        validity_flag.set_invalid();
        None
    });

    if let Some(result) = bridge_result {
        return Ok(result);
    }

    // Try custom Python components
    if let Some(info) = world.get_resource::<pybevy_core::CustomComponentInfo>()
        && let Ok(eref) = world.get_entity(entity) {
            for component_id in eref.archetype().components() {
                if let Some(entry) = info.get(*component_id) {
                    if entry.name != component {
                        continue;
                    }
                    let is_pyobj = entry.is_pyobject_storage;
                    let fields = Python::attach(|py| {
                        if let Ok(eref2) = world.get_entity(entity) {
                            extract_custom_component_fields(py, &eref2, *component_id, is_pyobj)
                        } else {
                            None
                        }
                    });
                    return Ok(serde_json::json!({
                        "entity_id": entity_id,
                        "entity_label": &label,
                        "component": component,
                        "fields": fields,
                    }));
                }
            }
        }

    Err(ControlError::not_found(format!(
        "Component '{component}' not found on entity {entity_id}"
    )))
}

/// Query entities by With/Without component filters
pub fn query_entities(
    world: &mut World,
    with_filters: Vec<String>,
    without_filters: Vec<String>,
) -> Result<serde_json::Value, ControlError> {
    let bridges = pybevy_core::registry::global_registry::all_component_bridges();

    let mut query_state = world.query::<Entity>();
    let all_entities: Vec<Entity> = query_state.iter(world).collect();

    let mut matching = Vec::new();

    // Debug: log counts on first call or when empty results seem wrong
    let entity_count = all_entities.len();
    let bridge_count = bridges.len();

    if entity_count > 0 && bridge_count == 0 {
        eprintln!(
            "[MCP] query_entities: {entity_count} entities but 0 bridges registered! Bridge registry may not be populated."
        );
    }

    for entity in &all_entities {
        let Ok(entity_ref) = world.get_entity(*entity) else {
            continue;
        };

        // Collect component names for this entity (bridges + custom)
        let mut has_components: Vec<String> = Vec::new();
        for bridge in &bridges {
            if bridge.entity_contains(&entity_ref) {
                has_components.push(bridge.name().to_string());
            }
        }

        // Add custom Python component names
        let custom_names = get_custom_component_names(world, *entity);
        has_components.extend(custom_names.iter().cloned());

        // Check With filters
        let has_all_with = with_filters
            .iter()
            .all(|f| has_components.iter().any(|c| c == f));

        // Check Without filters
        let has_none_without = without_filters
            .iter()
            .all(|f| !has_components.iter().any(|c| c == f));

        if has_all_with && has_none_without {
            let entity_name = entity_ref.get::<Name>().map(|n| n.as_str().to_string());
            let label = super::spatial::entity_label(world, *entity);

            // Detect remaining unknown components
            let archetype_count = entity_ref.archetype().components().len();
            let unknown_count = archetype_count.saturating_sub(has_components.len());

            let mut entry = serde_json::json!({
                "id": entity.to_bits(),
                "name": entity_name,
                "label": label,
                "components": has_components,
            });
            if !custom_names.is_empty() {
                entry["custom_components"] = serde_json::json!(custom_names);
            }
            if unknown_count > 0 {
                entry["unknown_component_count"] = serde_json::json!(unknown_count);
            }
            matching.push(entry);
        }
    }

    // Debug: log when entities exist but nothing matched the filters
    if matching.is_empty() && entity_count > 0 && !with_filters.is_empty() {
        // Sample first entity to debug
        if let Some(first_entity) = all_entities.first()
            && let Ok(entity_ref) = world.get_entity(*first_entity) {
                let sample_components: Vec<String> = bridges
                    .iter()
                    .filter(|b| b.entity_contains(&entity_ref))
                    .map(|b| b.name().to_string())
                    .collect();

                // Also check with Bevy's native Name
                let has_name = entity_ref.get::<Name>().is_some();

                eprintln!(
                    "[MCP] query_entities: 0 matches from {entity_count} entities (filters: with={with_filters:?}, without={without_filters:?}). \
                     Bridges: {bridge_count}. First entity ({}) has {} bridge components: {sample_components:?}. Has Bevy Name: {has_name}",
                    first_entity.to_bits(),
                    sample_components.len(),
                );
            }
    }

    Ok(serde_json::json!({
        "count": matching.len(),
        "entities": matching,
    }))
}

/// Extract field values from a custom Python resource (dataclass or plain object).
/// Returns a JSON map of field_name -> repr(value), or None if no fields found.
fn extract_custom_resource_fields(
    py: Python<'_>,
    bound: &Bound<'_, PyAny>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let mut map = serde_json::Map::new();

    // Try dataclass fields first
    let fields_result = py
        .import("dataclasses")
        .and_then(|dc| dc.getattr("fields"))
        .and_then(|f| f.call1((bound,)));
    if let Ok(fields) = fields_result
        && let Ok(iter) = fields.try_iter()
    {
        for field in iter.flatten() {
            let name_value = field
                .getattr("name")
                .and_then(|n| n.extract::<String>())
                .ok()
                .and_then(|name| bound.getattr(name.as_str()).ok().map(|value| (name, value)));
            if let Some((name, value)) = name_value {
                let repr = value
                    .repr()
                    .map(|r| r.to_string())
                    .unwrap_or_else(|_| "<opaque>".to_string());
                map.insert(name, serde_json::Value::String(repr));
            }
        }
        if !map.is_empty() {
            return Some(map);
        }
    }

    // Fallback: try __dict__
    if let Ok(dict) = bound.getattr("__dict__") {
        #[allow(clippy::collapsible_if)]
        if let Ok(py_dict) = dict.cast::<pyo3::types::PyDict>() {
            for (key, value) in py_dict.iter() {
                if let Ok(k) = key.extract::<String>() {
                    if !k.starts_with('_') {
                        let repr = value
                            .repr()
                            .map(|r| r.to_string())
                            .unwrap_or_else(|_| "<opaque>".to_string());
                        map.insert(k, serde_json::Value::String(repr));
                    }
                }
            }
        }
    }

    if map.is_empty() { None } else { Some(map) }
}

/// List all registered resources
pub fn list_resources(world: &mut World) -> Result<serde_json::Value, ControlError> {
    let mut resources = Vec::new();

    // Built-in resources (from bridge registry)
    for bridge in pybevy_core::registry::global_registry::all_resource_bridges() {
        let name = bridge.name().to_string();
        let present = bridge.contains_in_world(world);

        let mut entry = serde_json::json!({
            "name": name,
            "present": present,
        });

        // Extract field values for present resources
        if present {
            let fields = Python::attach(|py| {
                let validity_flag = pybevy_core::ValidityFlag::new_read();
                let validity = validity_flag.with_access_mode(pybevy_core::AccessMode::Read);
                let result = bridge.get(world, validity, py).ok().map(|py_obj| {
                    let bound = py_obj.bind(py);
                    extract_bridge_fields(py, bound)
                });
                validity_flag.set_invalid();
                result
            });
            if let Some(fields) = fields {
                entry
                    .as_object_mut()
                    .unwrap()
                    .insert("fields".into(), serde_json::json!(fields));
            }
        }

        resources.push(entry);
    }

    // Custom Python resources (from CustomResourceInfo)
    let bridge_names: std::collections::HashSet<String> = resources
        .iter()
        .filter_map(|r| r["name"].as_str().map(String::from))
        .collect();

    // Collect custom resource entries before accessing PyResourceStorage
    let custom_entries: Vec<(bevy::ecs::component::ComponentId, String)> = world
        .get_resource::<pybevy_core::CustomResourceInfo>()
        .map(|custom_info| {
            custom_info
                .iter()
                .filter(|(_, entry)| !bridge_names.contains(&entry.name))
                .map(|(comp_id, entry)| (comp_id, entry.name.clone()))
                .collect()
        })
        .unwrap_or_default();

    for (comp_id, name) in custom_entries {
        let mut res_entry = serde_json::json!({
            "name": name,
            "present": true,
            "custom": true,
        });

        // Extract values from PyResourceStorage
        if let Some(storage) = world.get_resource::<pybevy_core::PyResourceStorage>()
            && let Some(py_obj) = storage.resources.get(&comp_id) {
                let fields = Python::attach(|py| {
                    let bound = py_obj.bind(py);
                    extract_custom_resource_fields(py, bound)
                });
                if let Some(fields) = fields {
                    res_entry
                        .as_object_mut()
                        .unwrap()
                        .insert("fields".into(), serde_json::json!(fields));
                }
            }

        resources.push(res_entry);
    }

    Ok(serde_json::json!({
        "resource_count": resources.len(),
        "resources": resources,
    }))
}

/// List registered systems by stage
pub fn list_systems(world: &mut World) -> Result<serde_json::Value, ControlError> {
    let mut stages = serde_json::Map::new();

    if let Some(schedules) = world.get_resource::<bevy::ecs::schedule::Schedules>() {
        for (label, schedule) in schedules.iter() {
            let system_count = schedule.systems_len();
            stages.insert(
                format!("{label:?}"),
                serde_json::json!({ "system_count": system_count }),
            );
        }
    }

    Ok(serde_json::json!({ "stages": stages }))
}

/// Get component schema (field names, types, defaults, spawn example)
pub fn get_component_schema(
    world: &mut World,
    name: String,
) -> Result<serde_json::Value, ControlError> {
    // Handle special prefixed queries from grep_api and get_type_definition
    if let Some(query) = name.strip_prefix("__search:") {
        return Ok(serde_json::json!({
            "note": "grep_api should be handled by the server's ApiIndex",
            "query": query,
        }));
    }
    if let Some(type_name) = name.strip_prefix("__typedef:") {
        return Ok(serde_json::json!({
            "note": "get_type_definition should be handled by the server's ApiIndex",
            "type_name": type_name,
        }));
    }

    // Find the component bridge by name
    for bridge in pybevy_core::registry::global_registry::all_component_bridges() {
        if bridge.name() == name {
            // Try to get Bevy reflection type info for richer schema
            let reflection_info = world
                .get_resource::<AppTypeRegistry>()
                .and_then(|reg| {
                    let type_registry = reg.read();
                    let type_id = bridge.bevy_type_id();
                    type_registry.get(type_id).map(|registration| {
                        let type_info = registration.type_info();
                        let has_reflect_component = registration
                            .data::<bevy::ecs::reflect::ReflectComponent>()
                            .is_some();
                        let is_struct = matches!(type_info, TypeInfo::Struct(_));
                        // NOTE: This misses components that have settable Python properties
                        // (e.g. Text2d). A more complete check would inspect for @property
                        // setters on the Python type, but that requires further investigation.
                        let editable = has_reflect_component && is_struct;

                        let mut field_types = serde_json::Map::new();
                        if let TypeInfo::Struct(info) = type_info {
                            for i in 0..info.field_len() {
                                if let Some(field) = info.field_at(i) {
                                    field_types.insert(
                                        field.name().to_string(),
                                        serde_json::json!(field.type_path_table().short_path()),
                                    );
                                }
                            }
                        }

                        (editable, field_types, type_info_kind_name(type_info))
                    })
                });

            let schema = Python::attach(|py| {
                let py_type = bridge.py_type(py);
                let type_name = py_type
                    .name()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|_| name.clone());
                let fields = get_class_fields(py, &py_type);

                let mut result = serde_json::json!({
                    "name": type_name,
                    "fields": fields,
                    "registered": true,
                });

                if let Some((editable, field_types, type_kind)) = &reflection_info {
                    // If reflection says not editable, check for writable Python properties
                    let effective_editable = if !editable {
                        has_writable_properties(py, &py_type)
                    } else {
                        *editable
                    };
                    let obj = result.as_object_mut().unwrap();
                    obj.insert("editable".into(), serde_json::json!(effective_editable));
                    obj.insert("type_kind".into(), serde_json::json!(type_kind));
                    if !field_types.is_empty() {
                        obj.insert(
                            "field_types".into(),
                            serde_json::Value::Object(field_types.clone()),
                        );
                    }
                }

                result
            });

            return Ok(schema);
        }
    }

    // Fallback: check custom Python components via CustomComponentInfo
    if let Some(custom_info) = world.get_resource::<pybevy_core::CustomComponentInfo>() {
        for (_, entry) in custom_info.iter() {
            if entry.name == name {
                let schema = Python::attach(|py| {
                    let py_type = unsafe {
                        pyo3::Bound::from_borrowed_ptr(
                            py,
                            entry.type_ptr as *mut pyo3::ffi::PyObject,
                        )
                    };
                    let fields = if let Ok(cls) = py_type.cast::<pyo3::types::PyType>() {
                        get_class_fields(py, cls)
                    } else {
                        serde_json::json!({})
                    };

                    serde_json::json!({
                        "name": entry.name,
                        "fields": fields,
                        "registered": true,
                        "custom": true,
                        "editable": entry.is_pyobject_storage,
                        "type_kind": "CustomPython",
                        "storage": if entry.is_pyobject_storage { "python" } else { "wrapper" },
                    })
                });

                return Ok(schema);
            }
        }
    }

    Err(ControlError::not_found(format!(
        "Component '{name}' not found in registry. It may be an Asset or Resource. \
         Use get_type_definition(type_name=\"{name}\") to see its API."
    )))
}

fn type_info_kind_name(info: &TypeInfo) -> &'static str {
    match info {
        TypeInfo::Struct(_) => "Struct",
        TypeInfo::TupleStruct(_) => "TupleStruct",
        TypeInfo::Tuple(_) => "Tuple",
        TypeInfo::List(_) => "List",
        TypeInfo::Array(_) => "Array",
        TypeInfo::Map(_) => "Map",
        TypeInfo::Set(_) => "Set",
        TypeInfo::Enum(_) => "Enum",
        TypeInfo::Opaque(_) => "Opaque",
    }
}

/// Normalize a Python type repr string.
/// Strips `<class '...'>` wrapping and surrounding single quotes from type names.
/// e.g. `"<class 'float'>"` -> `"float"`, `"'Vec3'"` -> `"Vec3"`
fn normalize_type_repr(s: &str) -> String {
    // Strip <class '...'> wrapping
    if s.starts_with("<class '") && s.ends_with("'>") {
        return s[8..s.len() - 2].to_string();
    }
    // Strip surrounding single quotes
    if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 {
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

/// Extract field information from a Python class
fn get_class_fields(py: Python<'_>, py_type: &Bound<'_, pyo3::types::PyType>) -> serde_json::Value {
    let mut fields = serde_json::Map::new();

    // Try __annotations__
    if let Ok(annotations) = py_type.getattr("__annotations__")
        && let Ok(dict) = annotations.cast::<pyo3::types::PyDict>() {
            for (key, value) in dict.iter() {
                if let Ok(k) = key.extract::<String>() {
                    let v = value
                        .repr()
                        .map(|r| normalize_type_repr(&r.to_string()))
                        .unwrap_or_else(|_| "unknown".to_string());
                    fields.insert(k, serde_json::Value::String(v));
                }
            }
        }

    // Try PyO3 getset_descriptor detection via dir()
    if fields.is_empty()
        && let Ok(dir_list) = py_type.dir() {
            for attr_obj in dir_list.iter() {
                let Ok(name) = attr_obj.extract::<String>() else {
                    continue;
                };
                if name.starts_with('_') {
                    continue;
                }
                if let Ok(attr) = py_type.as_any().getattr(name.as_str()) {
                    let type_name = attr
                        .get_type()
                        .name()
                        .map(|n| n.to_string())
                        .unwrap_or_default();
                    if type_name == "getset_descriptor" {
                        fields.insert(name, serde_json::Value::String("property".into()));
                    }
                }
            }
        }

    // Fallback: try __init__ signature via inspect module
    if fields.is_empty()
        && let Ok(inspect) = py.import("inspect")
            && let Ok(sig) = inspect.call_method1("signature", (py_type,))
                && let Ok(params) = sig.getattr("parameters")
                    && let Ok(items) = params.call_method0("items")
                        && let Ok(iter) = items.try_iter() {
                            for item in iter {
                                if let Ok(tuple) = item
                                    && let Ok(k) =
                                        tuple.get_item(0).and_then(|v| v.extract::<String>())
                                        && k != "self" && !k.starts_with('_') {
                                            fields.insert(
                                                k,
                                                serde_json::Value::String("unknown".into()),
                                            );
                                        }
                            }
                        }

    serde_json::Value::Object(fields)
}

/// Resolve an EntityRef to a Bevy Entity
/// Get the bounding box (AABB) of an entity, both local and world-space.
/// Falls back to merging descendant AABBs for SceneRoot/GLB hierarchy entities.
pub fn get_bounding_box(
    world: &mut World,
    entity_ref: EntityRef,
) -> Result<serde_json::Value, ControlError> {
    let entity = resolve_entity(world, &entity_ref)?;

    // Try entity's own Aabb first
    if let Some(aabb) = world.get::<bevy::camera::primitives::Aabb>(entity) {
        let center = aabb.center;
        let half = aabb.half_extents;
        let local_min = center - half;
        let local_max = center + half;

        let r = super::spatial::round6;
        let local = serde_json::json!({
            "center": [r(center.x), r(center.y), r(center.z)],
            "half_extents": [r(half.x), r(half.y), r(half.z)],
            "min": [r(local_min.x), r(local_min.y), r(local_min.z)],
            "max": [r(local_max.x), r(local_max.y), r(local_max.z)],
        });

        // Compute world-space bounds by transforming AABB corners
        let world_bounds = if let Some(gt) = world.get::<bevy::prelude::GlobalTransform>(entity) {
            let transform = gt.affine();
            let corners = [
                bevy::math::Vec3A::new(local_min.x, local_min.y, local_min.z),
                bevy::math::Vec3A::new(local_max.x, local_min.y, local_min.z),
                bevy::math::Vec3A::new(local_min.x, local_max.y, local_min.z),
                bevy::math::Vec3A::new(local_max.x, local_max.y, local_min.z),
                bevy::math::Vec3A::new(local_min.x, local_min.y, local_max.z),
                bevy::math::Vec3A::new(local_max.x, local_min.y, local_max.z),
                bevy::math::Vec3A::new(local_min.x, local_max.y, local_max.z),
                bevy::math::Vec3A::new(local_max.x, local_max.y, local_max.z),
            ];

            let mut world_min = bevy::math::Vec3A::splat(f32::MAX);
            let mut world_max = bevy::math::Vec3A::splat(f32::MIN);
            for corner in &corners {
                let transformed = transform.transform_point3a(*corner);
                world_min = world_min.min(transformed);
                world_max = world_max.max(transformed);
            }

            let world_center = (world_min + world_max) * 0.5;
            let world_size = world_max - world_min;

            serde_json::json!({
                "center": [r(world_center.x), r(world_center.y), r(world_center.z)],
                "min": [r(world_min.x), r(world_min.y), r(world_min.z)],
                "max": [r(world_max.x), r(world_max.y), r(world_max.z)],
                "size": [r(world_size.x), r(world_size.y), r(world_size.z)],
            })
        } else {
            serde_json::json!(null)
        };

        return Ok(serde_json::json!({
            "local": local,
            "world": world_bounds,
        }));
    }

    // Fallback: merge descendant AABBs (SceneRoot/GLB entities)
    let merged = super::spatial::compute_world_aabb(world, entity)
        .map_err(|_| ControlError::not_found("Entity has no Aabb (no mesh?)"))?;

    let world_center = (merged.min + merged.max) * 0.5;
    let world_size = merged.max - merged.min;
    let r = super::spatial::round6;

    Ok(serde_json::json!({
        "local": null,
        "world": {
            "center": [r(world_center.x), r(world_center.y), r(world_center.z)],
            "min": [r(merged.min.x), r(merged.min.y), r(merged.min.z)],
            "max": [r(merged.max.x), r(merged.max.y), r(merged.max.z)],
            "size": [r(world_size.x), r(world_size.y), r(world_size.z)],
        },
        "resolved_from_children": true,
    }))
}

/// Scene summary: group entities by type for a quick inventory.
///
/// Grouping priority per entity:
/// 1. Custom Python component name (most descriptive)
/// 2. Name component text (grouped by identical/prefix)
/// 3. Characteristic built-in component (Camera3d > PointLight > Mesh3d etc.)
/// 4. Fallback: "other"
pub fn scene_summary(world: &mut World) -> Result<serde_json::Value, ControlError> {
    /// Characteristic built-in components in priority order.
    const CHARACTERISTIC_COMPONENTS: &[&str] = &[
        "Camera3d",
        "Camera2d",
        "PointLight",
        "DirectionalLight",
        "SpotLight",
        "Mesh3d",
        "Text",
        "AudioPlayer",
    ];

    let bridges = pybevy_core::registry::global_registry::all_component_bridges();

    let mut query_state = world.query::<Entity>();
    let entity_list: Vec<Entity> = query_state.iter(world).collect();
    let total = entity_list.len();

    // Map: label -> (count, source)
    let mut groups: HashMap<String, (usize, &'static str)> = HashMap::new();

    for entity in &entity_list {
        let Ok(entity_ref) = world.get_entity(*entity) else {
            groups
                .entry("other".to_string())
                .or_insert((0, "fallback"))
                .0 += 1;
            continue;
        };

        // Priority 1: Custom Python component name
        let custom_names = get_custom_component_names(world, *entity);
        if let Some(first_custom) = custom_names.first() {
            groups
                .entry(first_custom.clone())
                .or_insert((0, "custom_component"))
                .0 += 1;
            continue;
        }

        // Priority 2: Name component
        if let Some(name) = entity_ref.get::<Name>() {
            let name_str = name.as_str();
            let label = strip_numeric_suffix(name_str);
            groups.entry(label).or_insert((0, "name")).0 += 1;
            continue;
        }

        // Priority 3: Characteristic built-in component
        let bridge_names: Vec<String> = bridges
            .iter()
            .filter(|b| b.entity_contains(&entity_ref))
            .map(|b| b.name().to_string())
            .collect();

        let mut found = false;
        for &char_name in CHARACTERISTIC_COMPONENTS {
            if bridge_names.iter().any(|n| n == char_name) {
                groups
                    .entry(char_name.to_string())
                    .or_insert((0, "component"))
                    .0 += 1;
                found = true;
                break;
            }
        }

        if !found {
            groups
                .entry("other".to_string())
                .or_insert((0, "fallback"))
                .0 += 1;
        }
    }

    // Build sorted output (by count descending, then name)
    let mut group_list: Vec<_> = groups.into_iter().collect();
    group_list.sort_by(|a, b| b.1.0.cmp(&a.1.0).then_with(|| a.0.cmp(&b.0)));

    let groups_json: Vec<serde_json::Value> = group_list
        .iter()
        .map(|(label, (count, source))| {
            serde_json::json!({
                "label": label,
                "count": count,
                "source": source,
            })
        })
        .collect();

    // Build summary string
    let summary_parts: Vec<String> = group_list
        .iter()
        .map(|(label, (count, _))| format!("{count} {label}"))
        .collect();
    let summary = format!("{total} entities: {}", summary_parts.join(", "));

    Ok(serde_json::json!({
        "total_entities": total,
        "summary": summary,
        "groups": groups_json,
    }))
}

/// Strip trailing numeric suffix for Name grouping.
/// "Cube_01" → "Cube", "Fish3" → "Fish", "MyThing" → "MyThing"
fn strip_numeric_suffix(name: &str) -> String {
    let trailing_digits = name.len() - name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if trailing_digits == 0 {
        return name.to_string();
    }
    // Check if there's a decimal point just before the trailing digits
    let before_digits = &name[..name.len() - trailing_digits];
    if before_digits.ends_with('.') {
        return name.to_string(); // Part of a decimal number, don't strip
    }
    let trimmed = before_digits.trim_end_matches(['_', '-', ' ']);
    if trimmed.is_empty() {
        name.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn resolve_entity(world: &mut World, entity_ref: &EntityRef) -> Result<Entity, ControlError> {
    match entity_ref {
        EntityRef::Id(id) => {
            let entity = Entity::from_bits(*id);
            world
                .get_entity(entity)
                .map(|_| entity)
                .map_err(|_| ControlError::not_found(format!("Entity {id} not found")))
        }
        EntityRef::Name(name) => {
            let mut query_state = world.query::<(Entity, &Name)>();
            let mut first_child_match: Option<Entity> = None;
            for (entity, entity_name) in query_state.iter(world) {
                if entity_name.as_str() == name {
                    if world.get::<ChildOf>(entity).is_none() {
                        return Ok(entity); // Root entity — return immediately
                    }
                    if first_child_match.is_none() {
                        first_child_match = Some(entity); // Remember first child as fallback
                    }
                }
            }
            first_child_match.ok_or_else(|| {
                ControlError::not_found(format!("Entity with name '{name}' not found"))
            })
        }
    }
}
