use std::{any::TypeId, sync::Arc};

use bevy::{
    ecs::{
        entity::Entity,
        name::Name,
        reflect::{AppTypeRegistry, ReflectComponent},
        world::{EntityRef as BevyEntityRef, World},
    },
    reflect::{ReflectFromPtr, ReflectRef, TypeInfo},
};
use pybevy_core::{
    ComponentBridge, ValidityGuard,
    registry::global_registry::{all_component_bridges, all_resource_bridges},
    source_location::SourceLocation,
};
use pyo3::prelude::*;

use crate::{
    codec::reflect::reflect_to_json,
    engine::runtime::{ComponentCatalog, ComponentFilter, EntityComponentNames},
    protocol::{ControlError, EntityRef},
    pyo3::{
        asset_reference::reference_fields_for_value,
        names::{ComponentTarget, custom_component_name, resolve_component},
        to_json::{extract_bridge_fields, extract_custom_component_fields},
    },
    scene::{entity::resolve_entity, spatial::entity_label, summary::entity_component_breakdown},
};

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

/// Whether a reflected Bevy type is an enum, from the `AppTypeRegistry` TypeInfo.
pub(crate) fn reflected_type_is_enum(world: &World, type_id: TypeId) -> bool {
    world
        .get_resource::<AppTypeRegistry>()
        .is_some_and(|registry| {
            registry
                .read()
                .get(type_id)
                .is_some_and(|registration| matches!(registration.type_info(), TypeInfo::Enum(_)))
        })
}

fn reflected_enum_variant_name(world: &World, entity: Entity, type_id: TypeId) -> Option<String> {
    if !reflected_type_is_enum(world, type_id) {
        return None;
    }
    let registry = world.get_resource::<AppTypeRegistry>()?;
    let registry = registry.read();
    let reflect_component = registry.get(type_id)?.data::<ReflectComponent>()?;
    let entity_ref = world.get_entity(entity).ok()?;
    let value = reflect_component.reflect(entity_ref)?;
    let ReflectRef::Enum(value) = value.reflect_ref() else {
        return None;
    };
    Some(value.variant_name().to_string())
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
        && let Some(name) = eref.get::<Name>()
    {
        entity_name = Some(name.as_str().to_string());
    }

    // Extract component values via Python
    Python::attach(|py| {
        let (_read_window, validity) = ValidityGuard::read();

        for bridge in all_component_bridges() {
            let contains = world
                .get_entity(entity)
                .map(|entity_ref| bridge.entity_contains(&entity_ref))
                .unwrap_or(false);
            if contains {
                let name = bridge.name().to_string();
                let reflected_enum = reflected_type_is_enum(world, bridge.bevy_type_id());
                let reflected_variant = if reflected_enum {
                    reflected_enum_variant_name(world, entity, bridge.bevy_type_id())
                } else {
                    None
                };
                // SAFETY: `world` is a live &mut World; the pointer is valid for this call.
                let value = unsafe {
                    bridge.extract_from_entity_ref(
                        entity,
                        world as *mut World,
                        validity.clone(),
                        py,
                    )
                }
                .ok()
                .flatten()
                .map(|py_obj| {
                    serde_json::Value::Object(native_value_fields(
                        py,
                        world,
                        entity,
                        bridge.bevy_type_id(),
                        &name,
                        py_obj.bind(py),
                        (reflected_enum, reflected_variant),
                    ))
                })
                .unwrap_or_else(|| serde_json::Value::String("<opaque>".to_string()));

                components.insert(name, value);
            }
        }
    });

    // Extract custom Python component data
    let mut custom_components = serde_json::Map::new();
    if let Some(info) = world.get_resource::<pybevy_core::CustomComponentInfo>()
        && let Ok(eref) = world.get_entity(entity)
    {
        for component_id in eref.archetype().components() {
            if let Some(entry) = info.get(*component_id) {
                let descriptor_layout = world
                    .components()
                    .get_info(*component_id)
                    .map(|component| component.layout());
                Python::attach(|py| {
                    if let (Ok(eref2), Some(descriptor_layout)) =
                        (world.get_entity(entity), descriptor_layout)
                        && let Some(fields) = extract_custom_component_fields(
                            py,
                            &eref2,
                            *component_id,
                            entry,
                            descriptor_layout,
                        )
                    {
                        custom_components
                            .insert(entry.name.clone(), serde_json::Value::Object(fields));
                        return;
                    }
                    custom_components.insert(
                        entry.name.clone(),
                        serde_json::Value::String(if entry.is_pyobject_storage {
                            "<pyobject>".to_string()
                        } else {
                            "<wrapper>".to_string()
                        }),
                    );
                });
            }
        }
    }

    // Component breakdown: bridges + custom + Bevy-internal = archetype slots
    let bridge_count = components.len();
    let custom_count = custom_components.len();
    let breakdown = world
        .get_entity(entity)
        .ok()
        .map(|eref| entity_component_breakdown(&eref, bridge_count, custom_count));

    // Merge custom components into the components map
    for (k, v) in &custom_components {
        components.insert(k.clone(), v.clone());
    }

    let label = entity_label(world, entity);

    let mut result = serde_json::json!({
        "id": entity_id,
        "name": entity_name,
        "label": label,
        "components": components,
    });
    // Include source location if tracked
    if let Ok(eref) = world.get_entity(entity)
        && let Some(loc) = eref.get::<SourceLocation>()
    {
        result["source_location"] = serde_json::json!({
            "file": loc.file,
            "line": loc.line,
            "function": loc.function,
        });
    }
    if !custom_components.is_empty() {
        result["custom_components"] =
            serde_json::json!(custom_components.keys().collect::<Vec<_>>());
    }
    if let Some((archetype_slot_count, bevy_internal)) = breakdown {
        result["archetype_slot_count"] = serde_json::json!(archetype_slot_count);
        result["bridge_component_count"] = serde_json::json!(bridge_count);
        result["custom_component_count"] = serde_json::json!(custom_count);
        result["bevy_internal_component_count"] = serde_json::json!(bevy_internal);
    }
    Ok(result)
}

/// The reflected fields of a native value whose wrapper exposes no readable
/// properties, from `entity`'s component or, without one, the resource.
pub(crate) fn opaque_value_fields(
    world: &World,
    type_id: TypeId,
    entity: Option<Entity>,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    if fields.len() != 1 || !fields.contains_key("repr") {
        return None;
    }
    let component_id = world.components().get_id(type_id)?;
    let ptr = match entity {
        Some(entity) => world
            .get_entity(entity)
            .ok()?
            .get_by_id(component_id)
            .ok()?,
        None => world.get_resource_by_id(component_id)?,
    };
    let registry = world.get_resource::<AppTypeRegistry>()?.read();
    let from_ptr = registry.get_type_data::<ReflectFromPtr>(type_id)?;
    // SAFETY: `ptr` holds the value registered under `type_id`, the type `from_ptr` reads.
    let value = unsafe { from_ptr.ptr_as_reflect(ptr) };
    match reflect_to_json(value.as_partial_reflect()) {
        serde_json::Value::Object(map) => Some(map),
        _ => None,
    }
}

/// A native component value's fields: its asset reference when it has one,
/// else its getter fields, or its reflected fields when it has no getters,
/// plus `variant` for a reflected enum.
fn native_value_fields(
    py: Python<'_>,
    world: &mut World,
    entity: Entity,
    type_id: TypeId,
    component: &str,
    value: &Bound<'_, PyAny>,
    (reflected_enum, reflected_variant): (bool, Option<String>),
) -> serde_json::Map<String, serde_json::Value> {
    if let Some(serde_json::Value::Object(fields)) =
        reference_fields_for_value(world, entity, component, value)
    {
        return fields;
    }
    let mut fields = extract_bridge_fields(py, value, reflected_enum);
    if let Some(reflected) = opaque_value_fields(world, type_id, Some(entity), &fields) {
        fields = reflected;
    }
    if let Some(variant) = reflected_variant {
        if value.call_method0("__copy__").is_err() {
            fields.clear();
        }
        fields.insert("variant".to_string(), serde_json::Value::String(variant));
    }
    fields
}

/// The fields `get_component` reports for a native component value.
pub(crate) fn native_component_fields(
    py: Python<'_>,
    world: &mut World,
    entity: Entity,
    bridge: &Arc<dyn ComponentBridge>,
    value: &Bound<'_, PyAny>,
) -> serde_json::Map<String, serde_json::Value> {
    let reflected_enum = reflected_type_is_enum(world, bridge.bevy_type_id());
    let reflected_variant = if reflected_enum {
        reflected_enum_variant_name(world, entity, bridge.bevy_type_id())
    } else {
        None
    };
    native_value_fields(
        py,
        world,
        entity,
        bridge.bevy_type_id(),
        bridge.name(),
        value,
        (reflected_enum, reflected_variant),
    )
}

pub fn get_component(
    world: &mut World,
    entity_ref: EntityRef,
    component: String,
) -> Result<serde_json::Value, ControlError> {
    let entity = resolve_entity(world, &entity_ref)?;
    let entity_id = entity.to_bits();
    let label = entity_label(world, entity);
    let target = resolve_component(world, &component)?;

    // Try bridge components first
    let bridge_result = Python::attach(|py| -> Option<serde_json::Value> {
        let (_read_window, validity) = ValidityGuard::read();

        if let Some(ComponentTarget::Native(bridge)) = &target {
            let contains = match world.get_entity(entity) {
                Ok(eref) => bridge.entity_contains(&eref),
                Err(_) => return None,
            };
            if !contains {
                return None;
            }
            // SAFETY: `world` is a live &mut World; the pointer is valid for this call.
            let fields = unsafe {
                bridge.extract_from_entity_ref(entity, world as *mut World, validity.clone(), py)
            }
            .ok()
            .flatten()
            .map(|py_obj| native_component_fields(py, world, entity, bridge, py_obj.bind(py)));

            return Some(serde_json::json!({
                "entity_id": entity_id,
                "entity_label": &label,
                "component": component,
                "fields": fields,
            }));
        }

        None
    });

    if let Some(result) = bridge_result {
        return Ok(result);
    }

    // Try custom Python components
    if let Some(info) = world.get_resource::<pybevy_core::CustomComponentInfo>()
        && let Ok(eref) = world.get_entity(entity)
    {
        for component_id in eref.archetype().components() {
            if let Some(entry) = info.get(*component_id) {
                if !matches!(&target, Some(ComponentTarget::Custom(id)) if id == component_id) {
                    continue;
                }
                let descriptor_layout = world
                    .components()
                    .get_info(*component_id)
                    .map(|component| component.layout());
                let fields = Python::attach(|py| {
                    if let (Ok(eref2), Some(descriptor_layout)) =
                        (world.get_entity(entity), descriptor_layout)
                    {
                        extract_custom_component_fields(
                            py,
                            &eref2,
                            *component_id,
                            entry,
                            descriptor_layout,
                        )
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

/// Registered bridge and custom component names present on `entity`.
pub(crate) fn component_names(world: &World, entity: Entity) -> EntityComponentNames {
    let native = world
        .get_entity(entity)
        .map(|entity_ref| {
            all_component_bridges()
                .iter()
                .filter(|bridge| bridge.entity_contains(&entity_ref))
                .map(|bridge| bridge.name().to_string())
                .collect()
        })
        .unwrap_or_default();
    EntityComponentNames {
        native,
        custom: get_custom_component_names(world, entity),
    }
}

pub(crate) fn resolve_component_filter(
    world: &World,
    name: &str,
) -> Result<Option<ComponentFilter>, ControlError> {
    Ok(resolve_component(world, name)?.map(|target| match target {
        ComponentTarget::Native(bridge) => ComponentFilter::new(
            bridge.name() == "Disabled",
            move |entity_ref: &BevyEntityRef| bridge.entity_contains(entity_ref),
        ),
        ComponentTarget::Custom(id) => {
            ComponentFilter::new(false, move |entity_ref: &BevyEntityRef| {
                entity_ref.contains_id(id)
            })
        }
    }))
}

pub(crate) fn component_catalog(world: &World) -> ComponentCatalog {
    let custom = world.get_resource::<pybevy_core::CustomComponentInfo>();
    ComponentCatalog {
        native_names: all_component_bridges()
            .iter()
            .map(|bridge| bridge.name().to_string())
            .collect(),
        custom_names: custom
            .map(|info| info.iter().map(|(_, entry)| entry.name.clone()).collect())
            .unwrap_or_default(),
        custom_qualified_names: custom
            .map(|info| {
                info.iter()
                    .map(|(id, entry)| {
                        custom_component_name(world, id).unwrap_or_else(|| entry.name.clone())
                    })
                    .collect()
            })
            .unwrap_or_default(),
        resource_type_count: all_resource_bridges().len(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use bevy::{ecs::name::Name, prelude::Transform};

    // Force linker to include inventory entries needed by these handler tests.
    extern crate pybevy_camera;
    extern crate pybevy_transform;

    use pybevy_core::bridge_inventory::collect_all;

    use super::*;
    use crate::{protocol::ErrorCode, test_python::initialize_python};

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            initialize_python();
            collect_all();
        });
    }

    #[test]
    fn get_component_entity_not_found() {
        let mut world = World::new();
        let result = get_component(&mut world, EntityRef::Id(999999), "Transform".into());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn get_component_not_present_on_entity() {
        setup();
        let mut world = World::new();
        let entity = world.spawn(Name::new("TestEntity")).id();
        let result = get_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "Transform".into(),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert!(err.message.contains("not found on entity"));
    }

    #[test]
    fn get_component_bridge_by_name() {
        setup();
        let mut world = World::new();
        let entity = world
            .spawn((Name::new("Player"), Transform::from_xyz(1.0, 2.0, 3.0)))
            .id();
        let result = get_component(
            &mut world,
            EntityRef::Name("Player".into()),
            "Transform".into(),
        );
        let val = result.unwrap();
        assert_eq!(val["entity_id"], entity.to_bits());
        assert_eq!(val["component"], "Transform");
        let fields = val["fields"].as_object().expect("Expected fields object");
        assert!(
            fields.contains_key("translation"),
            "Expected translation field, got: {fields:?}"
        );
    }

    #[test]
    fn get_component_bridge_by_id() {
        setup();
        let mut world = World::new();
        let entity = world
            .spawn((Name::new("Light"), Transform::from_xyz(5.0, 10.0, 0.0)))
            .id();
        let result = get_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "Transform".into(),
        );
        let val = result.unwrap();
        assert_eq!(val["entity_id"], entity.to_bits());
        assert_eq!(val["component"], "Transform");
        let fields = val["fields"].as_object().expect("Expected fields object");
        assert!(
            fields.contains_key("translation"),
            "Expected translation field, got: {fields:?}"
        );
    }

    #[test]
    fn get_component_wrong_component_name() {
        setup();
        let mut world = World::new();
        let entity = world.spawn(Transform::default()).id();
        let result = get_component(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "NonexistentComponent".into(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("NonexistentComponent"));
    }

    #[test]
    fn get_entity_by_name() {
        setup();
        let mut world = World::new();
        world.spawn(Name::new("FindMe"));
        let result = get_entity(&mut world, EntityRef::Name("FindMe".into())).unwrap();
        assert_eq!(result["name"], "FindMe");
    }

    #[test]
    fn get_entity_returns_structured_native_component_values() {
        setup();
        let mut world = World::new();
        let entity = world
            .spawn((Name::new("Structured"), Transform::from_xyz(1.0, 2.0, 3.0)))
            .id();

        let result = get_entity(&mut world, EntityRef::Id(entity.to_bits())).unwrap();
        let transform = &result["components"]["Transform"];

        assert!(
            transform.is_object(),
            "expected JSON fields, got {transform}"
        );
        assert_eq!(transform["translation"], serde_json::json!([1.0, 2.0, 3.0]));
        assert_eq!(transform["scale"], serde_json::json!([1.0, 1.0, 1.0]));
        assert!(transform["rotation"].is_array());
    }

    #[test]
    fn get_entity_includes_component_breakdown_fields() {
        setup();
        let mut world = World::new();
        let entity = world.spawn((Name::new("Probe"), Transform::default())).id();
        let result = get_entity(&mut world, EntityRef::Id(entity.to_bits())).unwrap();
        assert!(result["archetype_slot_count"].is_number());
        assert!(result["bridge_component_count"].is_number());
        assert!(result["custom_component_count"].is_number());
        assert!(result["bevy_internal_component_count"].is_number());
        assert!(result.get("unknown_component_count").is_none());
        let arch = result["archetype_slot_count"].as_u64().unwrap();
        let bridge = result["bridge_component_count"].as_u64().unwrap();
        let custom = result["custom_component_count"].as_u64().unwrap();
        let internal = result["bevy_internal_component_count"].as_u64().unwrap();
        assert_eq!(arch, bridge + custom + internal);
    }

    #[test]
    fn get_entity_by_id() {
        setup();
        let mut world = World::new();
        let entity = world.spawn(Name::new("TestEntity")).id();
        let result = get_entity(&mut world, EntityRef::Id(entity.to_bits())).unwrap();
        assert!(result["id"].is_number());
        assert_eq!(result["name"], "TestEntity");
        assert!(result["components"].is_object());
    }

    #[test]
    fn get_entity_not_found() {
        let mut world = World::new();
        let result = get_entity(&mut world, EntityRef::Id(999999));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }
}
