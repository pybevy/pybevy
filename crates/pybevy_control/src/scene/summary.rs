use std::collections::HashMap;

use bevy::ecs::{
    entity::Entity,
    name::Name,
    world::{EntityRef as BevyEntityRef, World},
};
use pybevy_core::{public_error, source_location::SourceLocation};

use crate::{
    engine::runtime::ControlRuntime,
    protocol::{ControlError, MAX_QUERY_ENTITY_LIMIT, QueryEntitiesParams},
    scene::{
        entity_count::{scene_entities, scene_entities_including_disabled, scene_entity_count},
        spatial::{NameOccurrences, entity_label_with},
    },
};

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
pub(crate) fn entity_component_breakdown(
    entity_ref: &BevyEntityRef,
    bridge_count: usize,
    custom_count: usize,
) -> (usize, usize) {
    let archetype = entity_ref.archetype().components().len();
    let bevy_internal = archetype.saturating_sub(bridge_count + custom_count);
    (archetype, bevy_internal)
}

/// List all entities with their component types and Names
pub fn list_entities(
    world: &mut World,
    runtime: &mut dyn ControlRuntime,
) -> Result<serde_json::Value, ControlError> {
    let mut entities = Vec::new();
    let entity_list = scene_entities(world);
    let occurrences = NameOccurrences::collect(world);

    for entity in &entity_list {
        let entity_id = entity.to_bits();
        let names = runtime.component_names(world, *entity);
        let entity_name = world.get_entity(*entity).ok().and_then(|entity_ref| {
            entity_ref
                .get::<Name>()
                .map(|name| name.as_str().to_string())
        });
        let mut component_names = names.native.clone();
        component_names.extend(names.custom.iter().cloned());

        let label = entity_label_with(world, *entity, &occurrences);

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
        let bridge_count = names.native.len();
        let custom_count = names.custom.len();
        if let Ok(entity_ref) = world.get_entity(*entity) {
            let (archetype_slot_count, bevy_internal) =
                entity_component_breakdown(&entity_ref, bridge_count, custom_count);
            entry["archetype_slot_count"] = serde_json::json!(archetype_slot_count);
            entry["bridge_component_count"] = serde_json::json!(bridge_count);
            entry["custom_component_count"] = serde_json::json!(custom_count);
            entry["bevy_internal_component_count"] = serde_json::json!(bevy_internal);
        }
        if !names.custom.is_empty() {
            entry["custom_components"] = serde_json::json!(names.custom);
        }

        entities.push(entry);
    }

    Ok(serde_json::json!({
        "entity_count": entities.len(),
        "entities": entities,
    }))
}

/// Registry state, addressable resource names and sampled entity diagnostics.
pub fn debug_registry(
    world: &mut World,
    runtime: &mut dyn ControlRuntime,
) -> Result<serde_json::Value, ControlError> {
    let catalog = runtime.component_catalog(world);
    let mut resource_names = runtime.resource_names(world)?;
    resource_names.sort_unstable();
    resource_names.dedup();

    let entity_count = scene_entity_count(world) as usize;

    // Sample named entities (user's scene entities) for component detection
    let mut samples = Vec::new();
    let mut qs = world.query::<(Entity, &Name)>();
    let named_entities: Vec<(Entity, String)> = qs
        .iter(world)
        .take(10)
        .map(|(e, n)| (e, n.as_str().to_string()))
        .collect();
    let occurrences = NameOccurrences::collect(world);
    for (entity, name) in &named_entities {
        let Ok(archetype_components) = world
            .get_entity(*entity)
            .map(|entity_ref| entity_ref.archetype().components().len())
        else {
            continue;
        };
        let detected = runtime.component_names(world, *entity);
        let label = entity_label_with(world, *entity, &occurrences);
        samples.push(serde_json::json!({
            "id": entity.to_bits(),
            "name": name,
            "label": label,
            "detected_bridge_components": detected.native,
            "detected_custom_components": detected.custom,
            "archetype_component_count": archetype_components,
        }));
    }

    Ok(serde_json::json!({
        "component_bridge_count": catalog.native_names.len(),
        "component_bridge_names": catalog.native_names,
        "custom_component_count": catalog.custom_names.len(),
        "custom_component_names": catalog.custom_names,
        "custom_component_qualified_names": catalog.custom_qualified_names,
        "resource_bridge_count": catalog.resource_type_count,
        "total_entities": entity_count,
        "entity_samples": samples,
        "resource_names": resource_names,
    }))
}

/// Query entities by With/Without component filters
pub fn query_entities(
    world: &mut World,
    runtime: &mut dyn ControlRuntime,
    params: QueryEntitiesParams,
) -> Result<serde_json::Value, ControlError> {
    let QueryEntitiesParams {
        with: with_filters,
        without: without_filters,
        limit,
    } = params;
    if limit > MAX_QUERY_ENTITY_LIMIT {
        return Err(ControlError::invalid_params(format!(
            "limit must be at most {MAX_QUERY_ENTITY_LIMIT}"
        )));
    }

    let mut resolved_with = Vec::new();
    let mut resolved_without = Vec::new();
    let mut unknown_filters = Vec::new();
    for (names, filters) in [
        (&with_filters, &mut resolved_with),
        (&without_filters, &mut resolved_without),
    ] {
        for name in names {
            match runtime.resolve_component_filter(world, name)? {
                Some(filter) => filters.push(filter),
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

    let all_entities = if resolved_with.iter().any(|filter| filter.selects_disabled) {
        scene_entities_including_disabled(world)
    } else {
        scene_entities(world)
    };

    let occurrences = NameOccurrences::collect(world);
    let mut matching = Vec::new();
    let mut total_count = 0usize;

    let entity_count = all_entities.len();
    let bridge_count = runtime.component_catalog(world).native_names.len();
    if entity_count > 0 && bridge_count == 0 {
        bevy::log::warn!(
            "[MCP] query_entities: {entity_count} entities but 0 bridges registered! Bridge registry may not be populated."
        );
    }

    for entity in &all_entities {
        let Ok(entity_ref) = world.get_entity(*entity) else {
            continue;
        };
        let has_all_with = resolved_with
            .iter()
            .all(|filter| filter.matches(&entity_ref));
        let has_none_without = resolved_without
            .iter()
            .all(|filter| !filter.matches(&entity_ref));
        if !(has_all_with && has_none_without) {
            continue;
        }
        total_count += 1;
        if matching.len() >= limit {
            continue;
        }

        let names = runtime.component_names(world, *entity);
        let Ok(entity_ref) = world.get_entity(*entity) else {
            continue;
        };
        let entity_name = entity_ref.get::<Name>().map(|n| n.as_str().to_string());
        let label = entity_label_with(world, *entity, &occurrences);

        let custom_count = names.custom.len();
        let bridge_count = names.native.len();
        let (archetype_slot_count, bevy_internal) =
            entity_component_breakdown(&entity_ref, bridge_count, custom_count);
        let mut components = names.native.clone();
        components.extend(names.custom.iter().cloned());

        let mut entry = serde_json::json!({
            "id": entity.to_bits(),
            "name": entity_name,
            "label": label,
            "components": components,
            "archetype_slot_count": archetype_slot_count,
            "bridge_component_count": bridge_count,
            "custom_component_count": custom_count,
            "bevy_internal_component_count": bevy_internal,
        });
        if !names.custom.is_empty() {
            entry["custom_components"] = serde_json::json!(names.custom);
        }
        matching.push(entry);
    }

    if matching.is_empty()
        && entity_count > 0
        && !with_filters.is_empty()
        && let Some(first_entity) = all_entities.first()
    {
        let sample_components = runtime.component_names(world, *first_entity).native;
        let has_name = world.get::<Name>(*first_entity).is_some();
        bevy::log::debug!(
            "[MCP] query_entities: 0 matches from {entity_count} entities (filters: with={with_filters:?}, without={without_filters:?}). \
                     Bridges: {bridge_count}. First entity ({}) has {} bridge components: {sample_components:?}. Has Bevy Name: {has_name}",
            first_entity.to_bits(),
            sample_components.len(),
        );
    }

    Ok(serde_json::json!({
        "count": matching.len(),
        "total_count": total_count,
        "truncated": total_count > matching.len(),
        "entities": matching,
    }))
}

/// Scene summary: group entities by type for a quick inventory.
///
/// Grouping priority per entity:
/// 1. Custom Python component name (least prevalent in the scene, then lexical)
/// 2. Name component text (grouped by identical/prefix)
/// 3. Characteristic built-in component (Camera3d > PointLight > Mesh3d etc.)
/// 4. Fallback: "other"
pub fn scene_summary(
    world: &mut World,
    runtime: &mut dyn ControlRuntime,
) -> Result<serde_json::Value, ControlError> {
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

    let entity_list = scene_entities(world);
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
    let names_by_entity: HashMap<Entity, (Vec<String>, Vec<String>)> = entity_list
        .iter()
        .map(|entity| {
            let names = runtime.component_names(world, *entity);
            let mut custom = names.custom;
            custom.sort_unstable();
            custom.dedup();
            (*entity, (custom, names.native))
        })
        .collect();
    let mut custom_prevalence: HashMap<String, usize> = HashMap::new();
    for (custom, _) in names_by_entity.values() {
        for name in custom {
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
        let (custom_names, native_names) = names_by_entity
            .get(entity)
            .map(|(custom, native)| (custom.as_slice(), native.as_slice()))
            .unwrap_or_default();

        // Priority 1: Custom Python component name
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
        let characteristic = CHARACTERISTIC_COMPONENTS
            .iter()
            .find(|characteristic| native_names.iter().any(|name| name == *characteristic));
        match characteristic {
            Some(characteristic) => add(
                &mut groups,
                characteristic.to_string(),
                "component",
                *entity,
                false,
                None,
            ),
            None => add(
                &mut groups,
                "other".to_string(),
                "fallback",
                *entity,
                false,
                None,
            ),
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
    let summary = if summary_parts.is_empty() {
        format!("{total} entities")
    } else {
        format!("{total} entities: {}", summary_parts.join(", "))
    };

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
    use bevy::ecs::entity_disabling::Disabled;
    use serde_json::{Value, json};

    use super::*;
    use crate::{
        engine::runtime::fake::{Crate, Enemy, FakeRuntime},
        protocol::ErrorCode,
    };

    struct Scene {
        world: World,
        goblin: Entity,
        crate_box: Entity,
        hidden: Entity,
    }

    fn scene() -> Scene {
        let mut world = World::new();
        world.register_disabling_component::<Disabled>();
        let goblin = world.spawn((Name::new("Goblin_1"), Crate, Enemy)).id();
        let crate_box = world.spawn((Name::new("Box"), Crate)).id();
        let hidden = world.spawn((Crate, Disabled)).id();
        Scene {
            world,
            goblin,
            crate_box,
            hidden,
        }
    }

    fn entry(entities: &Value, entity: Entity) -> &Value {
        entities
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == json!(entity.to_bits()))
            .unwrap()
    }

    fn ids(result: &Value) -> Vec<u64> {
        result["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["id"].as_u64().unwrap())
            .collect()
    }

    fn query(world: &mut World, with: &[&str], without: &[&str]) -> Result<Value, ControlError> {
        let params = QueryEntitiesParams {
            with: with.iter().map(|name| name.to_string()).collect(),
            without: without.iter().map(|name| name.to_string()).collect(),
            limit: 100,
        };
        query_entities(world, &mut FakeRuntime, params)
    }

    #[test]
    fn list_entities_reports_runtime_component_names() {
        let mut scene = scene();
        let result = list_entities(&mut scene.world, &mut FakeRuntime).unwrap();

        assert_eq!(result["entity_count"], 2);
        let goblin = entry(&result["entities"], scene.goblin);
        assert_eq!(goblin["components"], json!(["Crate", "Enemy"]));
        assert_eq!(goblin["custom_components"], json!(["Enemy"]));
        assert_eq!(goblin["bridge_component_count"], 1);
        assert_eq!(goblin["custom_component_count"], 1);
        let crate_box = entry(&result["entities"], scene.crate_box);
        assert_eq!(crate_box["components"], json!(["Crate"]));
        assert!(crate_box.get("custom_components").is_none());
    }

    #[test]
    fn query_entities_applies_runtime_filters() {
        let mut scene = scene();

        let result = query(&mut scene.world, &["Crate"], &["Enemy"]).unwrap();
        assert_eq!(ids(&result), vec![scene.crate_box.to_bits()]);
        assert_eq!(result["total_count"], 1);

        let result = query(&mut scene.world, &["Crate", "Disabled"], &[]).unwrap();
        assert_eq!(ids(&result), vec![scene.hidden.to_bits()]);
    }

    #[test]
    fn query_entities_reports_each_unknown_filter_once() {
        let mut scene = scene();
        let error = query(&mut scene.world, &["Ghost", "Crate"], &["Ghost"]).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(
            error
                .message
                .starts_with("Unknown component filters: Ghost. "),
            "{}",
            error.message
        );
    }

    #[test]
    fn debug_registry_reports_catalog_and_sorted_resource_names() {
        let mut scene = scene();
        let result = debug_registry(&mut scene.world, &mut FakeRuntime).unwrap();

        assert_eq!(result["resource_names"], json!(["Clock", "Score"]));
        assert_eq!(result["component_bridge_names"], json!(["Crate"]));
        assert_eq!(result["custom_component_names"], json!(["Enemy"]));
        assert_eq!(
            result["custom_component_qualified_names"],
            json!(["game.Enemy"])
        );
        assert_eq!(result["resource_bridge_count"], 3);
        assert_eq!(result["total_entities"], 2);
        let goblin = entry(&result["entity_samples"], scene.goblin);
        assert_eq!(goblin["detected_bridge_components"], json!(["Crate"]));
        assert_eq!(goblin["detected_custom_components"], json!(["Enemy"]));
    }

    #[test]
    fn scene_summary_groups_custom_components_before_names() {
        let mut scene = scene();
        let result = scene_summary(&mut scene.world, &mut FakeRuntime).unwrap();

        assert_eq!(result["summary"], "2 entities: 1 Box, 1 Enemy");
        assert_eq!(
            result["groups"],
            json!([
                {"label": "Box", "count": 1, "source": "name", "representative_id": scene.crate_box.to_bits()},
                {"label": "Enemy", "count": 1, "source": "custom_component", "representative_id": scene.goblin.to_bits()},
            ])
        );
    }

    #[test]
    fn scene_summary_of_an_empty_scene_has_no_group_list() {
        let mut world = World::new();
        let result = scene_summary(&mut world, &mut FakeRuntime).unwrap();
        assert_eq!(
            result,
            json!({"total_entities": 0, "summary": "0 entities", "groups": []})
        );
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
}

#[cfg(all(test, feature = "pyo3-runtime"))]
mod runtime_tests {
    use std::{ptr, sync::Once};

    use bevy::{
        ecs::{
            component::{Component, ComponentId},
            name::Name,
        },
        prelude::{Camera3d, Transform},
    };
    use pyo3::Python;

    // Force linker to include inventory entries needed by these handler tests.
    extern crate pybevy_camera;
    extern crate pybevy_transform;

    use super::*;
    use crate::{
        protocol::{ErrorCode, QueryEntitiesParams},
        pyo3::runtime::Pyo3ControlRuntime,
    };

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            Python::initialize();
            pybevy_core::bridge_inventory::collect_all();
        });
    }

    fn list_entities(world: &mut World) -> Result<serde_json::Value, ControlError> {
        super::list_entities(world, &mut Pyo3ControlRuntime)
    }

    fn scene_summary(world: &mut World) -> Result<serde_json::Value, ControlError> {
        super::scene_summary(world, &mut Pyo3ControlRuntime)
    }

    fn debug_registry(world: &mut World) -> Result<serde_json::Value, ControlError> {
        super::debug_registry(world, &mut Pyo3ControlRuntime)
    }

    fn query_entities(
        world: &mut World,
        with: Vec<String>,
        without: Vec<String>,
        limit: usize,
    ) -> Result<serde_json::Value, ControlError> {
        super::query_entities(
            world,
            &mut Pyo3ControlRuntime,
            QueryEntitiesParams {
                with,
                without,
                limit,
            },
        )
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
}
