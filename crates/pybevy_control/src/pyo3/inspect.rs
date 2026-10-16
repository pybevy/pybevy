use std::{any::TypeId, collections::HashMap};

use bevy::{
    ecs::{
        entity::Entity,
        name::Name,
        reflect::{AppTypeRegistry, ReflectComponent},
        schedule::{IntoSystemSet, Schedules, SystemSet},
        world::{EntityRef as BevyEntityRef, World},
    },
    reflect::{ReflectRef, TypeInfo},
};
use pybevy_core::{
    public_error,
    registry::global_registry::{all_component_bridges, all_resource_bridges},
    source_location::SourceLocation,
};
use pybevy_ecs::shared::{
    schedule::ScheduleDisplayNameRegistry,
    system_runtime::{HotReloadGeneration, ReloadGenerationSet},
};
use pyo3::prelude::*;

use crate::{
    protocol::{ControlError, EntityRef},
    pyo3::{
        names::{ComponentTarget, custom_component_name, resolve_component},
        to_json::{extract_bridge_fields, extract_custom_component_fields},
    },
    scene::entity::resolve_entity,
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

/// Pick the least prevalent custom component, then the lexical first on ties.
fn custom_component_group_label<'a>(
    names: &'a [String],
    prevalence: &HashMap<String, usize>,
) -> Option<&'a str> {
    names
        .iter()
        .min_by(|left, right| {
            prevalence
                .get(*left)
                .cmp(&prevalence.get(*right))
                .then_with(|| left.cmp(right))
        })
        .map(String::as_str)
}

/// Compute archetype/Bevy-internal component counts for an entity.
/// Returns (archetype_slot_count, bevy_internal_component_count).
fn entity_component_breakdown(
    entity_ref: &BevyEntityRef,
    bridge_count: usize,
    custom_count: usize,
) -> (usize, usize) {
    let archetype = entity_ref.archetype().components().len();
    let bevy_internal = archetype.saturating_sub(bridge_count + custom_count);
    (archetype, bevy_internal)
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

/// List all entities with their component types and Names
pub fn list_entities(world: &mut World) -> Result<serde_json::Value, ControlError> {
    let mut entities = Vec::new();
    let bridges = all_component_bridges();

    let entity_list = crate::scene::entity_count::scene_entities(world);
    let occurrences = crate::scene::spatial::NameOccurrences::collect(world);

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

        let label = crate::scene::spatial::entity_label_with(world, *entity, &occurrences);

        // Include source location if tracked
        let source_location = world
            .get_entity(*entity)
            .ok()
            .and_then(|eref| eref.get::<SourceLocation>())
            .map(|loc| {
                serde_json::json!({
                    "file": loc.file,
                    "line": loc.line,
                    "function": loc.function,
                })
            });

        let mut entry = serde_json::json!({
            "id": entity_id,
            "name": entity_name,
            "label": label,
            "components": component_names,
        });
        if let Some(loc) = source_location {
            entry["source_location"] = loc;
        }

        // Component breakdown: bridges + custom + Bevy-internal = archetype slots
        let bridge_count = component_names.len().saturating_sub(custom_names.len());
        let custom_count = custom_names.len();
        if let Ok(entity_ref) = world.get_entity(*entity) {
            let (archetype_slot_count, bevy_internal) =
                entity_component_breakdown(&entity_ref, bridge_count, custom_count);
            entry["archetype_slot_count"] = serde_json::json!(archetype_slot_count);
            entry["bridge_component_count"] = serde_json::json!(bridge_count);
            entry["custom_component_count"] = serde_json::json!(custom_count);
            entry["bevy_internal_component_count"] = serde_json::json!(bevy_internal);
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
    let bridges = all_component_bridges();
    let resource_bridges = all_resource_bridges();

    let bridge_names: Vec<String> = bridges.iter().map(|b| b.name().to_string()).collect();

    // Custom @component types registered via CustomComponentInfo. Duplicates preserved.
    let custom_names: Vec<String> = world
        .get_resource::<pybevy_core::CustomComponentInfo>()
        .map(|info| info.iter().map(|(_, entry)| entry.name.clone()).collect())
        .unwrap_or_default();
    let custom_qualified_names: Vec<String> = world
        .get_resource::<pybevy_core::CustomComponentInfo>()
        .map(|info| {
            info.iter()
                .map(|(id, entry)| {
                    custom_component_name(world, id).unwrap_or_else(|| entry.name.clone())
                })
                .collect()
        })
        .unwrap_or_default();

    let entity_count = crate::scene::entity_count::scene_entity_count(world) as usize;

    // Sample named entities (user's scene entities) for component detection
    let mut samples = Vec::new();
    let mut qs = world.query::<(Entity, &Name)>();
    let named_entities: Vec<(Entity, String)> = qs
        .iter(world)
        .take(10)
        .map(|(e, n)| (e, n.as_str().to_string()))
        .collect();
    let occurrences = crate::scene::spatial::NameOccurrences::collect(world);
    for (entity, name) in &named_entities {
        if let Ok(entity_ref) = world.get_entity(*entity) {
            let detected: Vec<String> = bridges
                .iter()
                .filter(|b| b.entity_contains(&entity_ref))
                .map(|b| b.name().to_string())
                .collect();
            let archetype_components = entity_ref.archetype().components().len();

            let detected_custom = get_custom_component_names(world, *entity);
            let label = crate::scene::spatial::entity_label_with(world, *entity, &occurrences);
            samples.push(serde_json::json!({
                "id": entity.to_bits(),
                "name": name,
                "label": label,
                "detected_bridge_components": detected,
                "detected_custom_components": detected_custom,
                "archetype_component_count": archetype_components,
            }));
        }
    }

    Ok(serde_json::json!({
        "component_bridge_count": bridges.len(),
        "component_bridge_names": bridge_names,
        "custom_component_count": custom_names.len(),
        "custom_component_names": custom_names,
        "custom_component_qualified_names": custom_qualified_names,
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
        && let Some(name) = eref.get::<Name>()
    {
        entity_name = Some(name.as_str().to_string());
    }

    // Extract component values via Python
    Python::attach(|py| {
        let validity_flag = pybevy_core::ValidityFlag::new_read();
        let validity = validity_flag.with_access_mode(pybevy_core::AccessMode::Read);

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
                    let bound = py_obj.bind(py);
                    if let Some(fields) = super::asset_reference::reference_fields_for_value(
                        world, entity, &name, bound,
                    ) {
                        return fields;
                    }
                    let mut fields = extract_bridge_fields(py, bound, reflected_enum);
                    if let Some(variant) = reflected_variant {
                        if bound.call_method0("__copy__").is_err() {
                            fields.clear();
                        }
                        fields.insert("variant".to_string(), serde_json::Value::String(variant));
                    }
                    serde_json::Value::Object(fields)
                })
                .unwrap_or_else(|| serde_json::Value::String("<opaque>".to_string()));

                components.insert(name, value);
            }
        }

        validity_flag.set_invalid();
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

    let label = crate::scene::spatial::entity_label(world, entity);

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

/// Get a single component's field values from an entity
pub fn get_component(
    world: &mut World,
    entity_ref: EntityRef,
    component: String,
) -> Result<serde_json::Value, ControlError> {
    let entity = resolve_entity(world, &entity_ref)?;
    let entity_id = entity.to_bits();
    let label = crate::scene::spatial::entity_label(world, entity);
    let target = resolve_component(world, &component)?;

    // Try bridge components first
    let bridge_result = Python::attach(|py| -> Option<serde_json::Value> {
        let validity_flag = pybevy_core::ValidityFlag::new_read();
        let validity = validity_flag.with_access_mode(pybevy_core::AccessMode::Read);

        if let Some(ComponentTarget::Native(bridge)) = &target {
            let contains = match world.get_entity(entity) {
                Ok(eref) => bridge.entity_contains(&eref),
                Err(_) => return None,
            };
            if !contains {
                validity_flag.set_invalid();
                return None;
            }
            let reflected_enum = reflected_type_is_enum(world, bridge.bevy_type_id());
            let reflected_variant = if reflected_enum {
                reflected_enum_variant_name(world, entity, bridge.bevy_type_id())
            } else {
                None
            };
            // SAFETY: `world` is a live &mut World; the pointer is valid for this call.
            let fields = unsafe {
                bridge.extract_from_entity_ref(entity, world as *mut World, validity.clone(), py)
            }
            .ok()
            .flatten()
            .map(|py_obj| {
                let bound = py_obj.bind(py);
                if let Some(fields) = super::asset_reference::reference_fields_for_value(
                    world,
                    entity,
                    bridge.name(),
                    bound,
                ) {
                    return fields.as_object().cloned().unwrap_or_default();
                }
                let mut fields = extract_bridge_fields(py, bound, reflected_enum);
                if let Some(variant) = reflected_variant {
                    if bound.call_method0("__copy__").is_err() {
                        fields.clear();
                    }
                    fields.insert("variant".to_string(), serde_json::Value::String(variant));
                }
                fields
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

/// Query entities by With/Without component filters
pub fn query_entities(
    world: &mut World,
    with_filters: Vec<String>,
    without_filters: Vec<String>,
    limit: usize,
) -> Result<serde_json::Value, ControlError> {
    if limit > crate::protocol::MAX_QUERY_ENTITY_LIMIT {
        return Err(ControlError::invalid_params(format!(
            "limit must be at most {}",
            crate::protocol::MAX_QUERY_ENTITY_LIMIT
        )));
    }

    let bridges = all_component_bridges();
    let mut resolved_with = Vec::new();
    let mut resolved_without = Vec::new();
    let mut unknown_filters = Vec::new();
    for (names, targets) in [
        (&with_filters, &mut resolved_with),
        (&without_filters, &mut resolved_without),
    ] {
        for name in names {
            match resolve_component(world, name)? {
                Some(target) => targets.push(target),
                None => unknown_filters.push(name.as_str()),
            }
        }
    }
    unknown_filters.sort_unstable();
    unknown_filters.dedup();
    if !unknown_filters.is_empty() {
        return Err(ControlError::invalid_params(format!(
            "Unknown component filters: {}. {}",
            unknown_filters.join(", "),
            public_error::MCP_CUSTOM_COMPONENT_BOOTSTRAP
        )));
    }

    let all_entities = if resolved_with.iter().any(
        |target| matches!(target, ComponentTarget::Native(bridge) if bridge.name() == "Disabled"),
    ) {
        crate::scene::entity_count::scene_entities_including_disabled(world)
    } else {
        crate::scene::entity_count::scene_entities(world)
    };

    let occurrences = crate::scene::spatial::NameOccurrences::collect(world);
    let mut matching = Vec::new();
    let mut total_count = 0usize;

    // Debug: log counts on first call or when empty results seem wrong
    let entity_count = all_entities.len();
    let bridge_count = bridges.len();

    if entity_count > 0 && bridge_count == 0 {
        bevy::log::warn!(
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
        let has_all_with = resolved_with.iter().all(|target| match target {
            ComponentTarget::Native(bridge) => bridge.entity_contains(&entity_ref),
            ComponentTarget::Custom(id) => entity_ref.contains_id(*id),
        });

        // Check Without filters
        let has_none_without = resolved_without.iter().all(|target| match target {
            ComponentTarget::Native(bridge) => !bridge.entity_contains(&entity_ref),
            ComponentTarget::Custom(id) => !entity_ref.contains_id(*id),
        });

        if has_all_with && has_none_without {
            total_count += 1;
            if matching.len() >= limit {
                continue;
            }
            let entity_name = entity_ref.get::<Name>().map(|n| n.as_str().to_string());
            let label = crate::scene::spatial::entity_label_with(world, *entity, &occurrences);

            // Component breakdown: bridges + custom + Bevy-internal = archetype slots
            let custom_count = custom_names.len();
            let bridge_count = has_components.len().saturating_sub(custom_count);
            let (archetype_slot_count, bevy_internal) =
                entity_component_breakdown(&entity_ref, bridge_count, custom_count);

            let mut entry = serde_json::json!({
                "id": entity.to_bits(),
                "name": entity_name,
                "label": label,
                "components": has_components,
                "archetype_slot_count": archetype_slot_count,
                "bridge_component_count": bridge_count,
                "custom_component_count": custom_count,
                "bevy_internal_component_count": bevy_internal,
            });
            if !custom_names.is_empty() {
                entry["custom_components"] = serde_json::json!(custom_names);
            }
            matching.push(entry);
        }
    }

    // Debug: log when entities exist but nothing matched the filters
    if matching.is_empty() && entity_count > 0 && !with_filters.is_empty() {
        // Sample first entity to debug
        if let Some(first_entity) = all_entities.first()
            && let Ok(entity_ref) = world.get_entity(*first_entity)
        {
            let sample_components: Vec<String> = bridges
                .iter()
                .filter(|b| b.entity_contains(&entity_ref))
                .map(|b| b.name().to_string())
                .collect();

            // Also check with Bevy's native Name
            let has_name = entity_ref.get::<Name>().is_some();

            bevy::log::debug!(
                "[MCP] query_entities: 0 matches from {entity_count} entities (filters: with={with_filters:?}, without={without_filters:?}). \
                     Bridges: {bridge_count}. First entity ({}) has {} bridge components: {sample_components:?}. Has Bevy Name: {has_name}",
                first_entity.to_bits(),
                sample_components.len(),
            );
        }
    }

    Ok(serde_json::json!({
        "count": matching.len(),
        "total_count": total_count,
        "truncated": total_count > matching.len(),
        "entities": matching,
    }))
}

/// List scene-owned systems by stage, with optional engine internals.
pub fn list_systems(
    world: &mut World,
    include_internal: bool,
) -> Result<serde_json::Value, ControlError> {
    let mut stages = serde_json::Map::new();
    let mut visible_system_count = 0;
    let mut total_system_count = 0;
    let mut scene_system_count = 0;
    let mut internal_system_count = 0;
    let current_generation = world
        .get_resource::<HotReloadGeneration>()
        .map(|generation| generation.current);
    let effective_current_generation = current_generation.unwrap_or(0);
    let display_names = world.get_resource::<ScheduleDisplayNameRegistry>();

    if let Some(schedules) = world.get_resource::<Schedules>() {
        for (label, schedule) in schedules.iter() {
            let stage_total_system_count = schedule.systems_len();
            total_system_count += stage_total_system_count;
            let current_generation_systems = schedule
                .graph()
                .systems_in_set(
                    ReloadGenerationSet(effective_current_generation)
                        .into_system_set()
                        .intern(),
                )
                .ok();
            let retained_generation = current_generation.and_then(|generation| {
                generation
                    .checked_sub(1)
                    .map(|previous| (generation, previous))
            });
            let retained_generation_systems = retained_generation.and_then(|(_, previous)| {
                schedule
                    .graph()
                    .systems_in_set(ReloadGenerationSet(previous).into_system_set().intern())
                    .ok()
            });
            let retired_generation_systems = current_generation
                .map(|generation| {
                    (0..generation.saturating_sub(1))
                        .flat_map(|retired_generation| {
                            schedule
                                .graph()
                                .systems_in_set(
                                    ReloadGenerationSet(retired_generation)
                                        .into_system_set()
                                        .intern(),
                                )
                                .ok()
                                .into_iter()
                                .flat_map(move |systems| {
                                    systems.iter().map(move |key| (*key, retired_generation))
                                })
                        })
                        .collect::<HashMap<_, _>>()
                })
                .unwrap_or_default();
            let mut active_system_count = 0;
            let mut retained_system_count = 0;
            let mut retired_system_count = 0;
            let mut stage_scene_system_count = 0;
            let mut stage_internal_system_count = 0;
            // Graph may be uninitialized; degrade to empty list rather than failing.
            let schedule_systems = schedule.systems().map(|iter| {
                iter.map(|(key, system)| (key, system.name().to_string()))
                    .collect::<Vec<_>>()
            });
            let systems: Vec<serde_json::Value> = match schedule_systems {
                Ok(schedule_systems) => {
                    let mut current_name_counts = HashMap::<String, usize>::new();
                    for (key, name) in &schedule_systems {
                        if current_generation_systems.is_some_and(|systems| systems.contains(key)) {
                            *current_name_counts.entry(name.clone()).or_default() += 1;
                        }
                    }

                    schedule_systems
                        .into_iter()
                        .filter_map(|(key, system_name)| {
                            let in_current_generation = current_generation_systems
                                .is_some_and(|systems| systems.contains(&key));
                            let in_retained_generation = retained_generation_systems
                                .is_some_and(|systems| systems.contains(&key));
                            let retired_generation = retired_generation_systems.get(&key).copied();
                            let scene_owned = in_current_generation
                                || in_retained_generation
                                || retired_generation.is_some();
                            if scene_owned {
                                stage_scene_system_count += 1;
                            } else {
                                stage_internal_system_count += 1;
                            }

                            if !include_internal && !scene_owned {
                                return None;
                            }

                            let (active, reload_generation, reload_state) = if in_current_generation
                            {
                                active_system_count += 1;
                                (true, Some(effective_current_generation), "active")
                            } else if in_retained_generation {
                                let has_current_successor = current_name_counts
                                    .get_mut(&system_name)
                                    .is_some_and(|remaining| {
                                        if *remaining == 0 {
                                            false
                                        } else {
                                            *remaining -= 1;
                                            true
                                        }
                                    });
                                if has_current_successor {
                                    retained_system_count += 1;
                                    (
                                        false,
                                        retained_generation.map(|(_, previous)| previous),
                                        "rollback_retained",
                                    )
                                } else {
                                    retired_system_count += 1;
                                    (
                                        false,
                                        retained_generation.map(|(_, previous)| previous),
                                        "retired",
                                    )
                                }
                            } else if let Some(retired_generation) = retired_generation {
                                retired_system_count += 1;
                                (false, Some(retired_generation), "retired")
                            } else {
                                active_system_count += 1;
                                (true, None, "active")
                            };
                            Some(serde_json::json!({
                                "name": system_name,
                                "origin": if scene_owned { "scene" } else { "internal" },
                                "active": active,
                                "reload_generation": reload_generation,
                                "reload_state": reload_state,
                            }))
                        })
                        .collect()
                }
                Err(_) => Vec::new(),
            };

            let stage_visible_system_count = systems.len();
            visible_system_count += stage_visible_system_count;
            scene_system_count += stage_scene_system_count;
            internal_system_count += stage_internal_system_count;

            if !include_internal && systems.is_empty() {
                continue;
            }

            stages.insert(
                display_names
                    .and_then(|registry| registry.get_by_label(label))
                    .map_or_else(|| format!("{label:?}"), str::to_owned),
                serde_json::json!({
                    "system_count": stage_visible_system_count,
                    "total_system_count": stage_total_system_count,
                    "scene_system_count": stage_scene_system_count,
                    "internal_system_count": stage_internal_system_count,
                    "active_system_count": active_system_count,
                    "rollback_retained_system_count": retained_system_count,
                    "retired_system_count": retired_system_count,
                    "systems": systems,
                }),
            );
        }
    }

    Ok(serde_json::json!({
        "include_internal": include_internal,
        "system_count": visible_system_count,
        "total_system_count": total_system_count,
        "scene_system_count": scene_system_count,
        "internal_system_count": internal_system_count,
        "stages": stages,
    }))
}

/// Resolve an EntityRef to a Bevy Entity
/// Get the bounding box (AABB) of an entity, both local and world-space.
/// Falls back to merging descendant AABBs for WorldAssetRoot/GLB hierarchy entities.
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

        let r = crate::scene::spatial::round6;
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

    // Fallback: merge descendant AABBs (WorldAssetRoot/GLB entities)
    let merged = crate::scene::spatial::compute_world_aabb(world, entity)
        .map_err(|_| ControlError::not_found("Entity has no Aabb (no mesh?)"))?;

    let world_center = (merged.min + merged.max) * 0.5;
    let world_size = merged.max - merged.min;
    let r = crate::scene::spatial::round6;

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
/// 1. Custom Python component name (least prevalent in the scene, then lexical)
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

    let bridges = all_component_bridges();

    let entity_list = crate::scene::entity_count::scene_entities(world);
    let total = entity_list.len();

    // Per-group state for grouping pass.
    struct GroupInfo {
        count: u64,
        source: &'static str,
        representative_id: u64,
        // True only if every entity in the group has a Name equal to the label.
        all_names_match: bool,
        // The first member's real Name, for the singleton case below.
        first_name: Option<String>,
    }

    let mut groups: HashMap<String, GroupInfo> = HashMap::new();

    // Insert or update a group entry.
    fn add(
        groups: &mut HashMap<String, GroupInfo>,
        label: String,
        source: &'static str,
        entity: Entity,
        name_matches_label: bool,
        first_name: Option<String>,
    ) {
        let entry = groups.entry(label).or_insert(GroupInfo {
            count: 0,
            source,
            representative_id: entity.to_bits(),
            all_names_match: name_matches_label,
            first_name,
        });
        entry.count += 1;
        entry.representative_id = entry.representative_id.min(entity.to_bits());
        if !name_matches_label {
            entry.all_names_match = false;
        }
    }

    // Cache sorted, deduplicated names and scene prevalence before assigning a
    // label. The canonical scene list excludes resource backing entities.
    let custom_names_by_entity: HashMap<Entity, Vec<String>> = entity_list
        .iter()
        .map(|entity| {
            let mut names = get_custom_component_names(world, *entity);
            names.sort_unstable();
            names.dedup();
            (*entity, names)
        })
        .collect();
    let mut custom_prevalence: HashMap<String, usize> = HashMap::new();
    for names in custom_names_by_entity.values() {
        for name in names {
            *custom_prevalence.entry(name.clone()).or_default() += 1;
        }
    }

    for entity in &entity_list {
        let Ok(entity_ref) = world.get_entity(*entity) else {
            add(
                &mut groups,
                "other".to_string(),
                "fallback",
                *entity,
                false,
                None,
            );
            continue;
        };

        // Priority 1: Custom Python component name
        let custom_names = custom_names_by_entity
            .get(entity)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if let Some(custom_label) = custom_component_group_label(custom_names, &custom_prevalence) {
            add(
                &mut groups,
                custom_label.to_string(),
                "custom_component",
                *entity,
                false,
                None,
            );
            continue;
        }

        // Priority 2: Name component
        if let Some(name) = entity_ref.get::<Name>() {
            let name_str = name.as_str();
            let label = strip_numeric_suffix(name_str);
            let name_matches_label = label == name_str;
            add(
                &mut groups,
                label,
                "name",
                *entity,
                name_matches_label,
                Some(name_str.to_string()),
            );
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
                add(
                    &mut groups,
                    char_name.to_string(),
                    "component",
                    *entity,
                    false,
                    None,
                );
                found = true;
                break;
            }
        }

        if !found {
            add(
                &mut groups,
                "other".to_string(),
                "fallback",
                *entity,
                false,
                None,
            );
        }
    }

    // A group of one is not a prefix: the stripped label resolves nowhere.
    fn reported_label(label: String, info: &GroupInfo) -> String {
        match (&info.first_name, info.count, info.all_names_match) {
            (Some(name), 1, false) => name.clone(),
            _ => label,
        }
    }

    // Sort by count descending, then reported label ascending.
    let mut group_list: Vec<(String, GroupInfo)> = groups
        .into_iter()
        .map(|(label, info)| (reported_label(label, &info), info))
        .collect();
    group_list.sort_by(|a, b| b.1.count.cmp(&a.1.count).then_with(|| a.0.cmp(&b.0)));

    let groups_json: Vec<serde_json::Value> = group_list
        .iter()
        .map(|(label, info)| {
            // Promote name -> name_prefix when stripping merged distinct Names.
            let source = if info.source == "name" && info.count > 1 && !info.all_names_match {
                "name_prefix"
            } else {
                info.source
            };
            let mut obj = serde_json::json!({
                "label": label,
                "count": info.count,
                "source": source,
            });
            obj["representative_id"] = serde_json::json!(info.representative_id);
            obj
        })
        .collect();

    let summary_parts: Vec<String> = group_list
        .iter()
        .map(|(label, info)| format!("{} {}", info.count, label))
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

#[cfg(test)]
mod tests {
    use std::{
        ptr,
        sync::{Arc, Once, atomic::AtomicU32},
    };

    use bevy::{
        camera::primitives::Aabb,
        ecs::{
            component::{Component, ComponentId},
            name::Name,
            schedule::{IntoScheduleConfigs, Schedule, ScheduleLabel},
        },
        math::Vec3,
        prelude::{Camera3d, GlobalTransform, Transform},
    };
    use pybevy_ecs::shared::schedule::{
        StateMachineId, StateScheduleLabel, TransitionScheduleLabel,
    };

    // Force linker to include inventory entries needed by these handler tests.
    extern crate pybevy_camera;
    extern crate pybevy_transform;

    use super::*;
    use crate::protocol::ErrorCode;

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            Python::initialize();
            pybevy_core::bridge_inventory::collect_all();
        });
    }

    #[derive(Component)]
    struct CustomRole;

    #[derive(Component)]
    struct Chaser;

    #[derive(Component)]
    struct Prey;

    #[derive(Component)]
    struct GridMover;

    #[derive(Component)]
    struct AlphaTag;

    #[derive(Component)]
    struct ZetaTag;

    fn register_custom_component<T: Component>(
        world: &mut World,
        info: &mut pybevy_core::CustomComponentInfo,
        name: &str,
    ) {
        let component_id = world.register_component::<T>();
        info.insert(
            component_id,
            pybevy_core::CustomComponentEntry {
                type_ptr: ptr::null(),
                retained_type: None,
                name: name.to_string(),
                is_pyobject_storage: false,
                wrapper_layout: None,
            },
        );
    }

    fn custom_specificity_groups(reverse_registration: bool) -> serde_json::Value {
        let mut world = World::new();
        let mut info = pybevy_core::CustomComponentInfo::default();
        if reverse_registration {
            register_custom_component::<ZetaTag>(&mut world, &mut info, "ZetaTag");
            register_custom_component::<AlphaTag>(&mut world, &mut info, "AlphaTag");
            register_custom_component::<Prey>(&mut world, &mut info, "Prey");
            register_custom_component::<Chaser>(&mut world, &mut info, "Chaser");
            register_custom_component::<GridMover>(&mut world, &mut info, "GridMover");
        } else {
            register_custom_component::<GridMover>(&mut world, &mut info, "GridMover");
            register_custom_component::<Chaser>(&mut world, &mut info, "Chaser");
            register_custom_component::<Prey>(&mut world, &mut info, "Prey");
            register_custom_component::<AlphaTag>(&mut world, &mut info, "AlphaTag");
            register_custom_component::<ZetaTag>(&mut world, &mut info, "ZetaTag");
        }
        world.insert_resource(info);

        if reverse_registration {
            world.spawn((GridMover, Chaser));
            world.spawn((Prey, GridMover));
            world.spawn((GridMover, Prey));
            world.spawn((ZetaTag, AlphaTag));
        } else {
            world.spawn((Chaser, GridMover));
            world.spawn((GridMover, Prey));
            world.spawn((Prey, GridMover));
            world.spawn((AlphaTag, ZetaTag));
        }
        scene_summary(&mut world).unwrap()
    }

    // Test-only labels for list_systems coverage.
    #[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct TestStage;

    #[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct EmptyStage;

    fn sys_alpha() {}
    fn sys_beta() {}
    fn sys_gamma() {}
    fn sys_delta() {}

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
    fn strip_numeric_suffix_underscore_digits() {
        assert_eq!(strip_numeric_suffix("Cube_01"), "Cube");
    }

    #[test]
    fn strip_numeric_suffix_trailing_digits() {
        assert_eq!(strip_numeric_suffix("Fish3"), "Fish");
    }

    #[test]
    fn strip_numeric_suffix_no_suffix() {
        assert_eq!(strip_numeric_suffix("MyThing"), "MyThing");
    }

    #[test]
    fn strip_numeric_suffix_all_digits() {
        assert_eq!(strip_numeric_suffix("123"), "123");
    }

    #[test]
    fn strip_numeric_suffix_dash_digits() {
        assert_eq!(strip_numeric_suffix("Obj-42"), "Obj");
    }

    #[test]
    fn strip_numeric_suffix_space_digits() {
        assert_eq!(strip_numeric_suffix("Thing 7"), "Thing");
    }

    #[test]
    fn strip_numeric_suffix_decimal_coordinate() {
        assert_eq!(strip_numeric_suffix("pos_3.2"), "pos_3.2");
    }

    #[test]
    fn strip_numeric_suffix_with_decimal_in_name() {
        assert_eq!(strip_numeric_suffix("tile_1.5_offset"), "tile_1.5_offset");
    }

    #[test]
    fn list_entities_empty_world() {
        let mut world = World::new();
        let result = list_entities(&mut world).unwrap();
        assert_eq!(result["entity_count"], 0);
        assert!(result["entities"].as_array().unwrap().is_empty());
    }

    #[test]
    fn list_entities_with_named_entity() {
        let mut world = World::new();
        world.spawn(Name::new("Player"));
        let result = list_entities(&mut world).unwrap();
        assert_eq!(result["entity_count"], 1);
        let entities = result["entities"].as_array().unwrap();
        assert_eq!(entities[0]["name"], "Player");
    }

    #[test]
    fn list_entities_unnamed_entity_has_null_name() {
        let mut world = World::new();
        world.spawn_empty();
        let result = list_entities(&mut world).unwrap();
        assert_eq!(result["entity_count"], 1);
        let entities = result["entities"].as_array().unwrap();
        assert!(entities[0]["name"].is_null());
    }

    #[test]
    fn list_systems_empty_world() {
        let mut world = World::new();
        let result = list_systems(&mut world, false).unwrap();
        assert!(result["stages"].as_object().unwrap().is_empty());
        assert_eq!(result["system_count"], 0);
        assert_eq!(result["internal_system_count"], 0);
    }

    #[test]
    fn scene_summary_empty_world() {
        let mut world = World::new();
        let result = scene_summary(&mut world).unwrap();
        assert_eq!(result["total_entities"], 0);
        assert!(result["groups"].as_array().unwrap().is_empty());
        assert!(
            result["summary"]
                .as_str()
                .unwrap()
                .starts_with("0 entities")
        );
    }

    #[test]
    fn scene_summary_groups_by_name_prefix() {
        let mut world = World::new();
        world.spawn(Name::new("Cube_01"));
        world.spawn(Name::new("Cube_02"));
        world.spawn(Name::new("Sphere"));
        let result = scene_summary(&mut world).unwrap();
        assert_eq!(result["total_entities"], 3);
        let groups = result["groups"].as_array().unwrap();
        // "Cube" group should have count 2
        let cube_group = groups.iter().find(|g| g["label"] == "Cube").unwrap();
        assert_eq!(cube_group["count"], 2);
        let sphere_group = groups.iter().find(|g| g["label"] == "Sphere").unwrap();
        assert_eq!(sphere_group["count"], 1);
    }

    /// A group of one is not a prefix: report the entity's real Name.
    #[test]
    fn scene_summary_reports_the_real_name_for_a_group_of_one() {
        let mut world = World::new();
        for name in ["cube_rot45", "gamma42", "a_12", "n_1_2", "x9", "aZ"] {
            world.spawn(Name::new(name));
        }
        let result = scene_summary(&mut world).unwrap();
        let labels: Vec<&str> = result["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| {
                assert_eq!(group["count"], 1);
                assert_eq!(group["source"], "name");
                group["label"].as_str().unwrap()
            })
            .collect();
        // Ascending by the reported name, not by the stripped grouping key.
        assert_eq!(
            labels,
            ["aZ", "a_12", "cube_rot45", "gamma42", "n_1_2", "x9"]
        );
        assert_eq!(
            result["summary"],
            "6 entities: 1 aZ, 1 a_12, 1 cube_rot45, 1 gamma42, 1 n_1_2, 1 x9"
        );
    }

    /// Multiple numbered Names form a prefix group.
    #[test]
    fn scene_summary_still_groups_several_numbered_names() {
        let mut world = World::new();
        world.spawn(Name::new("enemy1"));
        world.spawn(Name::new("enemy2"));
        world.spawn(Name::new("enemy3"));
        let result = scene_summary(&mut world).unwrap();
        let groups = result["groups"].as_array().unwrap();
        let group = groups.iter().find(|g| g["label"] == "enemy").unwrap();
        assert_eq!(group["count"], 3);
        assert_eq!(group["source"], "name_prefix");
        assert!(!group["representative_id"].is_null());
    }

    #[test]
    fn get_scene_summary_single_name_uses_name_source() {
        let mut world = World::new();
        let hero = world.spawn(Name::new("hero")).id();
        let result = scene_summary(&mut world).unwrap();
        let groups = result["groups"].as_array().unwrap();
        let g = groups.iter().find(|g| g["label"] == "hero").unwrap();
        assert_eq!(g["count"], 1);
        assert_eq!(g["source"], "name");
        assert_eq!(g["representative_id"], hero.to_bits());
    }

    #[test]
    fn get_scene_summary_multiple_same_name_uses_name_source() {
        let mut world = World::new();
        let a = world.spawn(Name::new("goblin")).id();
        let b = world.spawn(Name::new("goblin")).id();
        let result = scene_summary(&mut world).unwrap();
        let groups = result["groups"].as_array().unwrap();
        let g = groups.iter().find(|g| g["label"] == "goblin").unwrap();
        assert_eq!(g["count"], 2);
        assert_eq!(g["source"], "name");
        assert_eq!(g["representative_id"], a.to_bits().min(b.to_bits()));
    }

    #[test]
    fn get_scene_summary_prefix_collision_uses_name_prefix_source() {
        let mut world = World::new();
        let a = world.spawn(Name::new("rep_p1")).id();
        let b = world.spawn(Name::new("rep_p2")).id();
        let result = scene_summary(&mut world).unwrap();
        let groups = result["groups"].as_array().unwrap();
        let g = groups.iter().find(|g| g["label"] == "rep_p").unwrap();
        assert_eq!(g["count"], 2);
        assert_eq!(g["source"], "name_prefix");
        assert_eq!(g["representative_id"], a.to_bits().min(b.to_bits()));
    }

    #[test]
    fn get_scene_summary_representative_id_present_for_component_groups() {
        let mut world = World::new();
        // Three unnamed entities with no recognised characteristic component
        // collapse into the "other" fallback group; representative_id still set.
        world.spawn_empty();
        world.spawn_empty();
        world.spawn_empty();
        let result = scene_summary(&mut world).unwrap();
        let groups = result["groups"].as_array().unwrap();
        let g = groups.iter().find(|g| g["label"] == "other").unwrap();
        assert_eq!(g["count"], 3);
        assert_eq!(g["source"], "fallback");
        assert!(g["representative_id"].is_u64());
    }

    #[test]
    fn get_scene_summary_representative_id_present_for_singletons() {
        let mut world = World::new();
        let solo = world.spawn(Name::new("solo")).id();
        let result = scene_summary(&mut world).unwrap();
        let groups = result["groups"].as_array().unwrap();
        let g = groups.iter().find(|g| g["label"] == "solo").unwrap();
        assert_eq!(g["count"], 1);
        assert_eq!(g["representative_id"], solo.to_bits());
    }

    #[test]
    fn scene_summary_singleton_representatives_cover_every_source() {
        setup();
        let mut world = World::new();
        let mut info = pybevy_core::CustomComponentInfo::default();
        register_custom_component::<CustomRole>(&mut world, &mut info, "CustomRole");
        world.insert_resource(info);

        let named = world.spawn(Name::new("named")).id();
        let custom = world.spawn(CustomRole).id();
        let builtin = world.spawn(Camera3d::default()).id();
        let fallback = world.spawn_empty().id();

        let result = scene_summary(&mut world).unwrap();
        let groups = result["groups"].as_array().unwrap();
        for (label, source, entity) in [
            ("named", "name", named),
            ("CustomRole", "custom_component", custom),
            ("Camera3d", "component", builtin),
            ("other", "fallback", fallback),
        ] {
            let group = groups.iter().find(|group| group["label"] == label).unwrap();
            assert_eq!(group["count"], 1);
            assert_eq!(group["source"], source);
            assert_eq!(group["representative_id"], entity.to_bits());
            assert!(world.get_entity(entity).is_ok());
        }
    }

    #[test]
    fn scene_summary_custom_labels_use_specificity_then_lexical_order() {
        for reverse_registration in [false, true] {
            let result = custom_specificity_groups(reverse_registration);
            let groups = result["groups"].as_array().unwrap();
            let labels_and_counts: Vec<(&str, u64)> = groups
                .iter()
                .map(|group| {
                    (
                        group["label"].as_str().unwrap(),
                        group["count"].as_u64().unwrap(),
                    )
                })
                .collect();
            assert_eq!(
                labels_and_counts,
                [("Prey", 2), ("AlphaTag", 1), ("Chaser", 1)]
            );
            assert!(
                groups
                    .iter()
                    .all(|group| group["source"] == "custom_component")
            );
        }
    }
    #[test]
    fn debug_registry_returns_valid_shape() {
        let mut world = World::new();
        let result = debug_registry(&mut world).unwrap();
        assert!(result["component_bridge_count"].is_number());
        assert!(result["resource_bridge_count"].is_number());
        assert!(result["total_entities"].is_number());
        assert!(result["entity_samples"].is_array());
    }

    #[test]
    fn debug_registry_includes_custom_component_names() {
        let mut world = World::new();
        let mut info = pybevy_core::CustomComponentInfo::default();
        info.insert(
            ComponentId::new(10001),
            pybevy_core::CustomComponentEntry {
                type_ptr: ptr::null(),
                retained_type: None,
                name: "Bouncy".to_string(),
                is_pyobject_storage: true,
                wrapper_layout: None,
            },
        );
        info.insert(
            ComponentId::new(10002),
            pybevy_core::CustomComponentEntry {
                type_ptr: ptr::null(),
                retained_type: None,
                name: "BounceCounter".to_string(),
                is_pyobject_storage: true,
                wrapper_layout: None,
            },
        );
        world.insert_resource(info);

        let result = debug_registry(&mut world).unwrap();
        assert_eq!(result["custom_component_count"], 2);
        let names: Vec<String> = result["custom_component_names"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"Bouncy".to_string()));
        assert!(names.contains(&"BounceCounter".to_string()));
    }

    #[test]
    fn debug_registry_no_custom_info_resource_emits_zero() {
        let mut world = World::new();
        let result = debug_registry(&mut world).unwrap();
        assert_eq!(result["custom_component_count"], 0);
        assert!(
            result["custom_component_names"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn debug_registry_preserves_duplicate_custom_entries() {
        let mut world = World::new();
        let mut info = pybevy_core::CustomComponentInfo::default();
        // Two distinct ComponentIds with the same name mimic the leak scenario.
        info.insert(
            ComponentId::new(20001),
            pybevy_core::CustomComponentEntry {
                type_ptr: ptr::null(),
                retained_type: None,
                name: "Bouncy".to_string(),
                is_pyobject_storage: true,
                wrapper_layout: None,
            },
        );
        info.insert(
            ComponentId::new(20002),
            pybevy_core::CustomComponentEntry {
                type_ptr: ptr::null(),
                retained_type: None,
                name: "Bouncy".to_string(),
                is_pyobject_storage: true,
                wrapper_layout: None,
            },
        );
        world.insert_resource(info);

        let result = debug_registry(&mut world).unwrap();
        assert_eq!(result["custom_component_count"], 2);
        let names: Vec<String> = result["custom_component_names"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let bouncy_count = names.iter().filter(|n| n.as_str() == "Bouncy").count();
        assert_eq!(
            bouncy_count, 2,
            "duplicate Bouncy entries should be visible"
        );
    }
    #[test]
    fn get_bounding_box_no_aabb() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("NoMesh")).id();
        let result = get_bounding_box(&mut world, EntityRef::Id(entity.to_bits()));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("no Aabb"));
    }

    #[test]
    fn get_bounding_box_with_aabb_no_transform() {
        let mut world = World::new();
        let entity = world
            .spawn(Aabb::from_min_max(
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(1.0, 1.0, 1.0),
            ))
            .id();
        let result = get_bounding_box(&mut world, EntityRef::Id(entity.to_bits())).unwrap();
        assert!(result["local"]["center"].is_array());
        assert!(result["world"].is_null());
    }

    #[test]
    fn get_bounding_box_with_aabb_and_transform() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
                GlobalTransform::from_xyz(10.0, 0.0, 0.0),
            ))
            .id();
        let result = get_bounding_box(&mut world, EntityRef::Id(entity.to_bits())).unwrap();
        assert!(result["local"]["center"].is_array());
        // World bounds should be shifted by +10 on X
        let world_center = result["world"]["center"].as_array().unwrap();
        let x = world_center[0].as_f64().unwrap();
        assert!(
            (x - 10.0).abs() < 0.01,
            "Expected world center X ~10.0, got {x}"
        );
    }
    #[test]
    fn query_entities_empty_world() {
        let mut world = World::new();
        let result = query_entities(&mut world, vec![], vec![], 100).unwrap();
        assert_eq!(result["count"], 0);
    }

    #[test]
    fn query_entities_no_filters_returns_all() {
        let mut world = World::new();
        world.spawn(Name::new("A"));
        world.spawn(Name::new("B"));
        let result = query_entities(&mut world, vec![], vec![], 100).unwrap();
        assert_eq!(result["count"], 2);
        assert_eq!(result["total_count"], 2);
        assert_eq!(result["truncated"], false);
    }

    #[test]
    fn query_entities_limits_records_and_reports_total() {
        let mut world = World::new();
        world.spawn(Name::new("A"));
        world.spawn(Name::new("B"));
        world.spawn(Name::new("C"));

        let result = query_entities(&mut world, vec![], vec![], 2).unwrap();

        assert_eq!(result["count"], 2);
        assert_eq!(result["total_count"], 3);
        assert_eq!(result["truncated"], true);
        assert_eq!(result["entities"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn query_entities_rejects_excessive_limit() {
        let mut world = World::new();
        let error = query_entities(
            &mut world,
            vec![],
            vec![],
            crate::protocol::MAX_QUERY_ENTITY_LIMIT + 1,
        )
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(error.message.contains("at most 1000"));
    }

    #[test]
    fn query_entities_rejects_unknown_component_filters() {
        setup();
        let mut world = World::new();
        let error = query_entities(
            &mut world,
            vec!["MissingWith".to_string(), "Transform".to_string()],
            vec!["MissingWithout".to_string(), "MissingWith".to_string()],
            100,
        )
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert_eq!(
            error.message,
            format!(
                "Unknown component filters: MissingWith, MissingWithout. {}",
                public_error::MCP_CUSTOM_COMPONENT_BOOTSTRAP
            )
        );
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
    fn get_entity_not_found() {
        let mut world = World::new();
        let result = get_entity(&mut world, EntityRef::Id(999999));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }
    #[test]
    fn query_entities_empty_filters() {
        let mut world = World::new();
        world.spawn(Name::new("A"));
        world.spawn(Name::new("B"));
        let result = query_entities(&mut world, vec![], vec![], 100).unwrap();
        // With no filters, should return entities that have at least some components
        assert!(result["count"].as_u64().unwrap() >= 2);
    }

    #[test]
    fn list_entities_includes_component_breakdown_fields() {
        setup();
        let mut world = World::new();
        world.spawn((Name::new("Sample"), Transform::default()));
        let result = list_entities(&mut world).unwrap();
        let entry = result["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == "Sample")
            .expect("Sample entity should be present");
        assert!(entry["archetype_slot_count"].is_number());
        assert!(entry["bridge_component_count"].is_number());
        assert!(entry["custom_component_count"].is_number());
        assert!(entry["bevy_internal_component_count"].is_number());
        assert!(entry.get("unknown_component_count").is_none());
        let arch = entry["archetype_slot_count"].as_u64().unwrap();
        let bridge = entry["bridge_component_count"].as_u64().unwrap();
        let custom = entry["custom_component_count"].as_u64().unwrap();
        let internal = entry["bevy_internal_component_count"].as_u64().unwrap();
        assert_eq!(arch, bridge + custom + internal);
        assert!(bridge >= 1, "Transform bridge should be detected");
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
    fn query_entities_includes_component_breakdown_fields() {
        setup();
        let mut world = World::new();
        world.spawn((Name::new("Q"), Transform::default()));
        let result = query_entities(&mut world, vec![], vec![], 100).unwrap();
        let entry = result["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == "Q")
            .expect("Q entity should be present");
        assert!(entry["archetype_slot_count"].is_number());
        assert!(entry["bridge_component_count"].is_number());
        assert!(entry["custom_component_count"].is_number());
        assert!(entry["bevy_internal_component_count"].is_number());
        assert!(entry.get("unknown_component_count").is_none());
        let arch = entry["archetype_slot_count"].as_u64().unwrap();
        let bridge = entry["bridge_component_count"].as_u64().unwrap();
        let custom = entry["custom_component_count"].as_u64().unwrap();
        let internal = entry["bevy_internal_component_count"].as_u64().unwrap();
        assert_eq!(arch, bridge + custom + internal);
    }
    #[test]
    fn scene_summary_with_entities() {
        let mut world = World::new();
        world.spawn(Name::new("A"));
        world.spawn(Name::new("B"));
        world.spawn(Name::new("C"));
        let result = scene_summary(&mut world).unwrap();
        assert!(result["total_entities"].as_u64().unwrap() >= 3);
    }
    #[test]
    fn list_entities_with_names() {
        let mut world = World::new();
        world.spawn(Name::new("Alpha"));
        world.spawn(Name::new("Beta"));
        let result = list_entities(&mut world).unwrap();
        assert!(result["entity_count"].as_u64().unwrap() >= 2);
        let entities = result["entities"].as_array().unwrap();
        let names: Vec<&str> = entities.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(names.contains(&"Alpha"));
        assert!(names.contains(&"Beta"));
    }
    #[test]
    fn get_bounding_box_with_aabb_checks_fields() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Name::new("Box"),
                Aabb::from_min_max(Vec3::new(-1.0, -2.0, -3.0), Vec3::new(1.0, 2.0, 3.0)),
                GlobalTransform::default(),
            ))
            .id();
        let result = get_bounding_box(&mut world, EntityRef::Id(entity.to_bits())).unwrap();
        assert!(result["local"]["min"].is_array());
        assert!(result["local"]["max"].is_array());
        assert!(result["world"]["size"].is_array());
    }

    #[test]
    fn get_bounding_box_resolves_children() {
        let mut world = World::new();
        let parent = world
            .spawn((
                Name::new("WorldAssetRoot"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        let child = world
            .spawn((
                Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
                GlobalTransform::default(),
            ))
            .id();
        world.entity_mut(parent).add_children(&[child]);
        let result =
            get_bounding_box(&mut world, EntityRef::Name("WorldAssetRoot".into())).unwrap();
        assert!(result["resolved_from_children"].as_bool().unwrap());
        assert!(result["local"].is_null());
        assert!(result["world"]["size"].is_array());
    }
    #[test]
    fn list_systems_returns_names_per_stage() {
        let mut world = World::new();
        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestStage);
        schedule.add_systems((sys_alpha, sys_beta, sys_gamma));
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, true).unwrap();
        let stage = &val["stages"]["TestStage"];
        assert_eq!(stage["system_count"], 3);
        assert_eq!(stage["scene_system_count"], 0);
        assert_eq!(stage["internal_system_count"], 3);
        let systems = stage["systems"].as_array().expect("systems array");
        assert_eq!(systems.len(), 3);
        let names: Vec<String> = systems
            .iter()
            .map(|s| s["name"].as_str().unwrap().to_string())
            .collect();
        assert!(
            names.iter().any(|n| n.contains("sys_alpha")),
            "expected sys_alpha in {names:?}"
        );
        assert!(
            names.iter().any(|n| n.contains("sys_beta")),
            "expected sys_beta in {names:?}"
        );
        assert!(
            names.iter().any(|n| n.contains("sys_gamma")),
            "expected sys_gamma in {names:?}"
        );

        let filtered = list_systems(&mut world, false).unwrap();
        assert!(filtered["stages"]["TestStage"].is_null());
        assert_eq!(filtered["system_count"], 0);
        assert_eq!(filtered["total_system_count"], 3);
        assert_eq!(filtered["internal_system_count"], 3);
    }

    #[test]
    fn list_systems_empty_schedule() {
        let mut world = World::new();
        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(EmptyStage);
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, true).unwrap();
        let stage = &val["stages"]["EmptyStage"];
        assert_eq!(stage["system_count"], 0);
        let systems = stage["systems"].as_array().expect("systems array");
        assert!(
            systems.is_empty(),
            "expected empty systems, got {systems:?}"
        );
    }

    #[test]
    fn list_systems_count_matches_names_len() {
        let mut world = World::new();
        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestStage);
        schedule.add_systems((sys_alpha, sys_beta));
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, true).unwrap();
        let stage = &val["stages"]["TestStage"];
        let count = stage["system_count"].as_u64().unwrap() as usize;
        let systems_len = stage["systems"].as_array().unwrap().len();
        assert_eq!(count, systems_len);
    }

    #[test]
    fn list_systems_uses_registered_state_schedule_names() {
        let mut world = World::new();
        let first_enter = StateScheduleLabel::on_enter(StateMachineId::new(1), 7);
        let second_exit = StateScheduleLabel::on_exit(StateMachineId::new(2), 7);
        let transition = TransitionScheduleLabel::new(StateMachineId::new(1), 7, 8);
        let mut schedules = Schedules::default();
        for label in [
            first_enter.intern(),
            second_exit.intern(),
            transition.intern(),
        ] {
            let mut schedule = Schedule::new(label);
            schedule.add_systems(sys_alpha);
            schedule.initialize(&mut world).unwrap();
            schedules.insert(schedule);
        }
        let mut ordinary = Schedule::new(TestStage);
        ordinary.add_systems(sys_beta);
        ordinary.initialize(&mut world).unwrap();
        schedules.insert(ordinary);
        world.insert_resource(schedules);

        let mut names = ScheduleDisplayNameRegistry::default();
        names.insert(first_enter.intern(), "OnEnter(game.First.READY)");
        names.insert(second_exit.intern(), "OnExit(game.Second.READY)");
        names.insert(
            transition.intern(),
            "OnTransition(game.First.READY -> game.First.RUNNING)",
        );
        world.insert_resource(names);

        let value = list_systems(&mut world, true).unwrap();
        let stages = value["stages"].as_object().unwrap();
        assert!(stages.contains_key("OnEnter(game.First.READY)"));
        assert!(stages.contains_key("OnExit(game.Second.READY)"));
        assert!(stages.contains_key("OnTransition(game.First.READY -> game.First.RUNNING)"));
        assert!(stages.contains_key("TestStage"));
        assert!(
            stages
                .keys()
                .all(|name| !name.contains("StateMachineId") && !name.contains("state_hash"))
        );
    }

    #[test]
    fn list_systems_recognizes_generation_zero_without_reload_resource() {
        let mut world = World::new();
        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestStage);
        schedule.add_systems(sys_alpha.in_set(ReloadGenerationSet(0)));
        schedule.add_systems(sys_beta);
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, false).unwrap();
        let stage = &val["stages"]["TestStage"];
        assert_eq!(stage["system_count"], 1);
        assert_eq!(stage["scene_system_count"], 1);
        assert_eq!(stage["internal_system_count"], 1);
        let system = &stage["systems"][0];
        assert!(system["name"].as_str().unwrap().contains("sys_alpha"));
        assert_eq!(system["origin"], "scene");
        assert_eq!(system["reload_generation"], 0);
    }

    #[test]
    fn list_systems_distinguishes_active_retained_and_retired_systems() {
        let mut world = World::new();
        world.insert_resource(HotReloadGeneration::new(Arc::new(AtomicU32::new(2))));

        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestStage);
        schedule.add_systems(sys_alpha);
        schedule.add_systems(sys_delta.in_set(ReloadGenerationSet(0)));
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(1)));
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(2)));
        schedule.add_systems(sys_gamma.in_set(ReloadGenerationSet(2)));
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, false).unwrap();
        let stage = &val["stages"]["TestStage"];
        assert_eq!(stage["system_count"], 4);
        assert_eq!(stage["total_system_count"], 5);
        assert_eq!(stage["scene_system_count"], 4);
        assert_eq!(stage["internal_system_count"], 1);
        assert_eq!(stage["active_system_count"], 2);
        assert_eq!(stage["rollback_retained_system_count"], 1);
        assert_eq!(stage["retired_system_count"], 1);
        assert_eq!(val["system_count"], 4);
        assert_eq!(val["total_system_count"], 5);
        assert_eq!(val["scene_system_count"], 4);
        assert_eq!(val["internal_system_count"], 1);

        let systems = stage["systems"].as_array().unwrap();
        assert!(systems.iter().all(|system| system["origin"] == "scene"));
        assert!(
            systems
                .iter()
                .all(|system| !system["name"].as_str().unwrap().contains("sys_alpha"))
        );
        let retained = systems
            .iter()
            .find(|system| {
                system["name"].as_str().unwrap().contains("sys_beta")
                    && system["reload_state"] == "rollback_retained"
            })
            .unwrap();
        assert_eq!(retained["active"], false);
        assert_eq!(retained["reload_generation"], 1);
        assert_eq!(retained["reload_state"], "rollback_retained");

        let current = systems
            .iter()
            .find(|system| system["name"].as_str().unwrap().contains("sys_gamma"))
            .unwrap();
        assert_eq!(current["active"], true);
        assert_eq!(current["reload_generation"], 2);
        assert_eq!(current["reload_state"], "active");

        let retired = systems
            .iter()
            .find(|system| system["name"].as_str().unwrap().contains("sys_delta"))
            .unwrap();
        assert_eq!(retired["active"], false);
        assert_eq!(retired["reload_generation"], 0);
        assert_eq!(retired["reload_state"], "retired");

        let with_internal = list_systems(&mut world, true).unwrap();
        let all_systems = with_internal["stages"]["TestStage"]["systems"]
            .as_array()
            .unwrap();
        let internal = all_systems
            .iter()
            .find(|system| system["name"].as_str().unwrap().contains("sys_alpha"))
            .unwrap();
        assert_eq!(internal["origin"], "internal");
        assert_eq!(with_internal["system_count"], 5);
    }

    #[test]
    fn list_systems_matches_retained_occurrences_one_for_one() {
        let mut world = World::new();
        world.insert_resource(HotReloadGeneration::new(Arc::new(AtomicU32::new(1))));

        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestStage);
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(0)));
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(0)));
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(1)));
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, false).unwrap();
        let stage = &val["stages"]["TestStage"];
        assert_eq!(stage["active_system_count"], 1);
        assert_eq!(stage["rollback_retained_system_count"], 1);
        assert_eq!(stage["retired_system_count"], 1);
        assert_eq!(stage["scene_system_count"], 3);
    }

    #[test]
    fn list_systems_classifies_nonzero_older_generation_as_retired() {
        let mut world = World::new();
        world.insert_resource(HotReloadGeneration::new(Arc::new(AtomicU32::new(3))));

        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestStage);
        schedule.add_systems(sys_delta.in_set(ReloadGenerationSet(1)));
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(2)));
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(3)));
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, false).unwrap();
        let stage = &val["stages"]["TestStage"];
        assert_eq!(stage["active_system_count"], 1);
        assert_eq!(stage["rollback_retained_system_count"], 1);
        assert_eq!(stage["retired_system_count"], 1);
        let retired = stage["systems"]
            .as_array()
            .unwrap()
            .iter()
            .find(|system| system["reload_state"] == "retired")
            .unwrap();
        assert!(retired["name"].as_str().unwrap().contains("sys_delta"));
        assert_eq!(retired["reload_generation"], 1);
    }
}
