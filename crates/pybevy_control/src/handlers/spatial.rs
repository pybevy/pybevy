use std::collections::{HashMap, HashSet};

use bevy::{
    ecs::{entity::Entity, name::Name, world::World},
    math::Vec3A,
    prelude::*,
};

use super::entity::resolve_entity;
use crate::bridge::{
    CheckAllOverlapsParams, CheckOverlapsParams, ControlError, QuerySpatialNeighborhoodParams,
    QuerySpatialParams,
};

/// Round an f32 to 6 decimal places for cleaner JSON output.
pub fn round6(v: f32) -> f32 {
    (v * 1_000_000.0).round() / 1_000_000.0
}

/// Round an f64 to 6 decimal places for cleaner JSON output.
#[allow(dead_code)]
pub fn round6_f64(v: f64) -> f64 {
    (v * 1_000_000.0).round() / 1_000_000.0
}

/// World-space axis-aligned bounding box.
#[derive(Debug)]
pub struct WorldAabb {
    pub min: Vec3A,
    pub max: Vec3A,
    pub entity: Entity,
}

/// Compute world-space AABB for a single entity that has its own Aabb component.
fn compute_entity_aabb(world: &World, entity: Entity) -> Result<Option<WorldAabb>, ControlError> {
    let Some(aabb) = world.get::<bevy::camera::primitives::Aabb>(entity) else {
        return Ok(None);
    };
    let Some(gt) = world.get::<GlobalTransform>(entity) else {
        return Ok(None);
    };
    if !gt.affine().is_finite() || !aabb.center.is_finite() || !aabb.half_extents.is_finite() {
        return Err(ControlError::invalid_params(format!(
            "Entity {} has a non-finite GlobalTransform or Aabb",
            entity.to_bits()
        )));
    }

    let center = aabb.center;
    let half = aabb.half_extents;
    let local_min = center - half;
    let local_max = center + half;

    let transform = gt.affine();
    let corners = [
        Vec3A::new(local_min.x, local_min.y, local_min.z),
        Vec3A::new(local_max.x, local_min.y, local_min.z),
        Vec3A::new(local_min.x, local_max.y, local_min.z),
        Vec3A::new(local_max.x, local_max.y, local_min.z),
        Vec3A::new(local_min.x, local_min.y, local_max.z),
        Vec3A::new(local_max.x, local_min.y, local_max.z),
        Vec3A::new(local_min.x, local_max.y, local_max.z),
        Vec3A::new(local_max.x, local_max.y, local_max.z),
    ];

    let mut world_min = Vec3A::splat(f32::MAX);
    let mut world_max = Vec3A::splat(f32::MIN);
    for corner in &corners {
        let transformed = transform.transform_point3a(*corner);
        world_min = world_min.min(transformed);
        world_max = world_max.max(transformed);
    }

    Ok(Some(WorldAabb {
        min: world_min,
        max: world_max,
        entity,
    }))
}

/// Recursively collect world AABBs from all descendants that have Aabb.
fn collect_descendant_aabbs(world: &World, entity: Entity) -> Result<Vec<WorldAabb>, ControlError> {
    let mut result = Vec::new();
    let Some(children) = world.get::<Children>(entity) else {
        return Ok(result);
    };
    for child in children.iter() {
        if let Some(aabb) = compute_entity_aabb(world, child)? {
            result.push(aabb);
        }
        // Recurse into grandchildren
        result.extend(collect_descendant_aabbs(world, child)?);
    }
    Ok(result)
}

fn collect_descendants(world: &World, root: Entity) -> HashSet<Entity> {
    let mut descendants = HashSet::new();
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        let Some(children) = world.get::<Children>(entity) else {
            continue;
        };
        for child in children.iter() {
            if descendants.insert(child) {
                pending.push(child);
            }
        }
    }
    descendants
}

/// Compute world-space AABB by transforming local Aabb corners via GlobalTransform.
/// Falls back to merging descendant AABBs for WorldAssetRoot/hierarchy entities.
pub fn compute_world_aabb(world: &World, entity: Entity) -> Result<WorldAabb, ControlError> {
    // Fast path: entity has its own Aabb
    if let Some(aabb) = compute_entity_aabb(world, entity)? {
        return Ok(aabb);
    }

    // Fallback: merge AABBs from descendants (handles WorldAssetRoot/GLB hierarchies)
    let descendant_aabbs = collect_descendant_aabbs(world, entity)?;
    if descendant_aabbs.is_empty() {
        return Err(ControlError::not_found(
            "Entity has no Aabb and no descendants with Aabb (no mesh in hierarchy?)",
        ));
    }

    let mut merged_min = Vec3A::splat(f32::MAX);
    let mut merged_max = Vec3A::splat(f32::MIN);
    for aabb in &descendant_aabbs {
        merged_min = merged_min.min(aabb.min);
        merged_max = merged_max.max(aabb.max);
    }

    Ok(WorldAabb {
        min: merged_min,
        max: merged_max,
        entity,
    })
}

/// Check if two AABBs overlap on all three axes.
pub fn aabbs_overlap(a: &WorldAabb, b: &WorldAabb) -> bool {
    a.min.x <= b.max.x
        && a.max.x >= b.min.x
        && a.min.y <= b.max.y
        && a.max.y >= b.min.y
        && a.min.z <= b.max.z
        && a.max.z >= b.min.z
}

/// Minimum distance between two non-overlapping AABBs (0 if overlapping).
pub fn aabb_min_distance(a: &WorldAabb, b: &WorldAabb) -> f32 {
    let dx = (a.min.x - b.max.x).max(b.min.x - a.max.x).max(0.0);
    let dy = (a.min.y - b.max.y).max(b.min.y - a.max.y).max(0.0);
    let dz = (a.min.z - b.max.z).max(b.min.z - a.max.z).max(0.0);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Penetration depth and dominant axis for overlapping AABBs.
/// Returns (depth, axis_name) where axis_name is "X", "Y", or "Z".
pub fn compute_penetration(a: &WorldAabb, b: &WorldAabb) -> (f32, &'static str) {
    let axis_depth = |a_min: f32, a_max: f32, b_min: f32, b_max: f32| {
        (a_max - b_min).min(b_max - a_min).max(0.0)
    };

    let depth_x = axis_depth(a.min.x, a.max.x, b.min.x, b.max.x);
    let depth_y = axis_depth(a.min.y, a.max.y, b.min.y, b.max.y);
    let depth_z = axis_depth(a.min.z, a.max.z, b.min.z, b.max.z);

    if depth_x <= depth_y && depth_x <= depth_z {
        (depth_x, "X")
    } else if depth_y <= depth_z {
        (depth_y, "Y")
    } else {
        (depth_z, "Z")
    }
}

/// Human-readable direction description.
/// Threshold: ignore axes < 15% of dominant axis magnitude.
pub fn describe_direction(dir: Vec3) -> String {
    if !dir.is_finite() {
        return "invalid direction (non-finite coordinates)".to_string();
    }
    let abs_x = dir.x.abs();
    let abs_y = dir.y.abs();
    let abs_z = dir.z.abs();
    let dominant = abs_x.max(abs_y).max(abs_z);

    if dominant < 0.001 {
        return "same position".to_string();
    }

    let threshold = dominant * 0.15;
    let mut parts = Vec::new();

    if abs_x > threshold {
        if dir.x > 0.0 {
            parts.push("+X (right)");
        } else {
            parts.push("-X (left)");
        }
    }
    if abs_y > threshold {
        if dir.y > 0.0 {
            parts.push("+Y (above)");
        } else {
            parts.push("-Y (below)");
        }
    }
    if abs_z > threshold {
        if dir.z > 0.0 {
            parts.push("+Z (forward)");
        } else {
            parts.push("-Z (behind)");
        }
    }

    if parts.is_empty() {
        "same position".to_string()
    } else {
        parts.join(" and ")
    }
}

/// Check if a name is generic (GLB mesh children share names like "geometry_0.PBRMaterial").
pub fn is_generic_name(name: Option<&str>) -> bool {
    match name {
        None => true,
        Some(n) => {
            n.contains("geometry_")
                || n.contains("Mesh/Primitive")
                || n.contains(".material")
                || n.contains(".mesh")
        }
    }
}

/// Per-request snapshot of how many entities carry each `Name`.
///
/// Label rendering needs name-uniqueness; computing it per entity makes
/// entity-listing endpoints quadratic, so loops build this once and pass it
/// to [`entity_label_with`].
pub(crate) struct NameOccurrences(HashMap<String, usize>);

impl NameOccurrences {
    pub(crate) fn collect(world: &World) -> Self {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for entity in world.iter_entities() {
            if let Some(name) = entity.get::<Name>() {
                *counts.entry(name.as_str().to_string()).or_insert(0) += 1;
            }
        }
        Self(counts)
    }

    fn is_unique(&self, candidate: &str) -> bool {
        self.0.get(candidate).copied().unwrap_or(0) == 1
    }
}

/// Walk up ChildOf chain to find the root ancestor (entity with no parent).
/// Returns the entity itself if it has no parent.
pub fn find_root_ancestor(world: &World, entity: Entity) -> Entity {
    let mut current = entity;
    for _ in 0..20 {
        let Some(child_of) = world.get::<ChildOf>(current) else {
            return current;
        };
        current = child_of.parent();
    }
    current
}

fn share_hierarchy_root(world: &World, a: Entity, b: Entity) -> bool {
    (world.get::<ChildOf>(a).is_some() || world.get::<ChildOf>(b).is_some())
        && find_root_ancestor(world, a) == find_root_ancestor(world, b)
}

/// Walk up ChildOf chain (max 10 levels) to find first non-generic named ancestor.
fn find_ancestor_name(
    world: &World,
    entity: Entity,
    occurrences: &NameOccurrences,
) -> Option<String> {
    let mut current = entity;
    let mut fallback = None;
    for _ in 0..10 {
        let Some(child_of) = world.get::<ChildOf>(current) else {
            break;
        };
        let parent = child_of.parent();
        if let Some(name) = world.get::<Name>(parent)
            && !is_generic_name(Some(name.as_str()))
        {
            let candidate = name.as_str().to_string();
            if occurrences.is_unique(&candidate) {
                return Some(candidate);
            }
            fallback.get_or_insert(candidate);
        }
        current = parent;
    }
    fallback
}

/// Get entity name or ID label for display.
/// For generic GLB mesh children, appends the parent's name for context.
///
/// Builds a fresh name snapshot per call; loops over many entities should
/// use [`entity_label_with`] with one [`NameOccurrences::collect`] instead.
pub fn entity_label(world: &World, entity: Entity) -> String {
    entity_label_with(world, entity, &NameOccurrences::collect(world))
}

pub(crate) fn entity_label_with(
    world: &World,
    entity: Entity,
    occurrences: &NameOccurrences,
) -> String {
    let name = world.get::<Name>(entity);
    let name_str = name.as_ref().map(|n| n.as_str());

    let base = match name_str {
        Some(n) => format!("\"{}\" ({})", n, entity.to_bits()),
        None => format!("{}", entity.to_bits()),
    };

    // Append parent context for generic, unnamed, or repeated imported names.
    if (is_generic_name(name_str) || name_str.is_some_and(|name| !occurrences.is_unique(name)))
        && let Some(ancestor) = find_ancestor_name(world, entity, occurrences)
    {
        return format!("{} [parent: {}]", base, ancestor);
    }

    base
}

/// Pairwise spatial query: distance, direction, AABB overlap between two entities.
pub fn query_spatial(
    world: &mut World,
    params: QuerySpatialParams,
) -> Result<serde_json::Value, ControlError> {
    let QuerySpatialParams { entity_a, entity_b } = params;
    let ea = resolve_entity(world, &entity_a)?;
    let eb = resolve_entity(world, &entity_b)?;

    let gt_a = world
        .get::<GlobalTransform>(ea)
        .ok_or_else(|| ControlError::not_found("Entity A has no GlobalTransform"))?;
    let gt_b = world
        .get::<GlobalTransform>(eb)
        .ok_or_else(|| ControlError::not_found("Entity B has no GlobalTransform"))?;

    let pos_a = gt_a.translation();
    let pos_b = gt_b.translation();
    let invalid_entities = [(ea, gt_a), (eb, gt_b)]
        .into_iter()
        .filter(|(_, transform)| !transform.affine().is_finite())
        .map(|(entity, _)| entity_label(world, entity))
        .collect::<Vec<_>>();
    if !invalid_entities.is_empty() {
        return Err(ControlError::invalid_params(format!(
            "Spatial query requires finite GlobalTransform values; invalid entities: {}",
            invalid_entities.join(", ")
        )));
    }
    let delta = pos_b - pos_a;
    let distance = delta.length();
    let direction = describe_direction(delta);

    let mut result = serde_json::json!({
        "entity_a": entity_label(world, ea),
        "entity_b": entity_label(world, eb),
        "position_a": [round6(pos_a.x), round6(pos_a.y), round6(pos_a.z)],
        "position_b": [round6(pos_b.x), round6(pos_b.y), round6(pos_b.z)],
        "center_distance": round6(distance),
        "direction_a_to_b": direction,
    });

    // Try AABB overlap analysis (entities may not have Aabb)
    let aabb_a = compute_world_aabb(world, ea);
    let aabb_b = compute_world_aabb(world, eb);

    if let (Ok(aa), Ok(ab)) = (&aabb_a, &aabb_b) {
        let overlaps = aabbs_overlap(aa, ab);
        result["aabb_overlap"] = serde_json::json!(overlaps);

        if overlaps {
            let (depth, axis) = compute_penetration(aa, ab);
            result["penetration_depth"] = serde_json::json!(round6(depth));
            result["penetration_axis"] = serde_json::json!(axis);
        } else {
            let gap = aabb_min_distance(aa, ab);
            result["surface_gap"] = serde_json::json!(round6(gap));
        }

        // AABB sizes for context
        let size_a = aa.max - aa.min;
        let size_b = ab.max - ab.min;
        result["aabb_size_a"] =
            serde_json::json!([round6(size_a.x), round6(size_a.y), round6(size_a.z)]);
        result["aabb_size_b"] =
            serde_json::json!([round6(size_b.x), round6(size_b.y), round6(size_b.z)]);
    } else {
        result["aabb_note"] = serde_json::json!("One or both entities lack Aabb (no mesh)");
    }

    Ok(result)
}

/// Neighborhood query: find entities within radius of a center entity.
pub fn query_spatial_neighborhood(
    world: &mut World,
    params: QuerySpatialNeighborhoodParams,
) -> Result<serde_json::Value, ControlError> {
    let QuerySpatialNeighborhoodParams {
        entity: entity_ref,
        radius,
        max_results,
    } = params;
    if !radius.is_finite() || radius < 0.0 {
        return Err(ControlError::invalid_params(
            "radius must be >= 0 and finite",
        ));
    }
    let center_entity = resolve_entity(world, &entity_ref)?;

    // Verify the entity actually has a Transform (not just a stale GlobalTransform)
    if world.get::<Transform>(center_entity).is_none() {
        return Err(ControlError::not_found(
            "Center entity has no Transform component (GlobalTransform may be stale)",
        ));
    }
    let gt_center = world
        .get::<GlobalTransform>(center_entity)
        .ok_or_else(|| ControlError::not_found("Center entity has no GlobalTransform"))?;
    let center_pos = gt_center.translation();
    if !gt_center.affine().is_finite() {
        return Err(ControlError::invalid_params(format!(
            "Center entity {} has a non-finite GlobalTransform",
            entity_label(world, center_entity)
        )));
    }

    // Collect all entities with GlobalTransform
    let mut query_state = world.query::<(Entity, &GlobalTransform)>();
    let mut neighbors: Vec<(Entity, f32, Vec3)> = Vec::new();
    let mut invalid_entities = Vec::new();

    for (entity, gt) in query_state.iter(world) {
        if entity == center_entity {
            continue;
        }
        if !gt.affine().is_finite() {
            invalid_entities.push(entity_label(world, entity));
            continue;
        }
        let pos = gt.translation();
        let dist = (pos - center_pos).length();
        if dist <= radius {
            neighbors.push((entity, dist, pos));
        }
    }

    // Sort by distance
    neighbors.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let limit = max_results.unwrap_or(50);
    let total_count = neighbors.len();
    let truncated = total_count > limit;
    let neighbors: Vec<_> = neighbors.into_iter().take(limit).collect();

    let occurrences = NameOccurrences::collect(world);
    let results: Vec<serde_json::Value> = neighbors
        .iter()
        .map(|(entity, dist, pos)| {
            let dir = *pos - center_pos;
            serde_json::json!({
                "entity": entity_label_with(world, *entity, &occurrences),
                "distance": round6(*dist),
                "position": [round6(pos.x), round6(pos.y), round6(pos.z)],
                "direction": describe_direction(dir),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "center": entity_label_with(world, center_entity, &occurrences),
        "center_position": [round6(center_pos.x), round6(center_pos.y), round6(center_pos.z)],
        "radius": radius,
        "count": results.len(),
        "total_count": total_count,
        "truncated": truncated,
        "invalid_count": invalid_entities.len(),
        "invalid_entities": invalid_entities,
        "neighbors": results,
    }))
}

/// Check overlaps for a single entity against all others.
pub fn check_overlaps(
    world: &mut World,
    params: CheckOverlapsParams,
) -> Result<serde_json::Value, ControlError> {
    let CheckOverlapsParams {
        entity: entity_ref,
        include_siblings,
        max_float_gap,
        ground_y,
    } = params;
    let target = resolve_entity(world, &entity_ref)?;
    if world
        .get::<GlobalTransform>(target)
        .is_some_and(|transform| !transform.affine().is_finite())
    {
        return Err(ControlError::invalid_params(format!(
            "Entity {} has a non-finite GlobalTransform",
            entity_label(world, target)
        )));
    }
    let target_aabb = compute_world_aabb(world, target)?;
    let target_descendants = collect_descendants(world, target);

    // Collect all entities with Aabb + GlobalTransform
    let mut query_state =
        world.query::<(Entity, &bevy::camera::primitives::Aabb, &GlobalTransform)>();
    let all_entities: Vec<Entity> = query_state.iter(world).map(|(e, _, _)| e).collect();

    let occurrences = NameOccurrences::collect(world);
    let mut overlaps = Vec::new();
    let mut nearest_below: Option<(Entity, f32)> = None;
    let mut invalid_entities = Vec::new();

    for entity in &all_entities {
        if *entity == target || target_descendants.contains(entity) {
            continue;
        }

        // Skip entities sharing the same root ancestor (parented parts overlap by design)
        if !include_siblings && share_hierarchy_root(world, target, *entity) {
            continue;
        }

        let other_aabb = match compute_world_aabb(world, *entity) {
            Ok(aabb) => aabb,
            Err(error) if error.code == crate::bridge::ErrorCode::InvalidParams => {
                invalid_entities.push(entity_label(world, *entity));
                continue;
            }
            Err(_) => continue,
        };

        if aabbs_overlap(&target_aabb, &other_aabb) {
            let (depth, axis) = compute_penetration(&target_aabb, &other_aabb);
            overlaps.push(serde_json::json!({
                "entity": entity_label_with(world, *entity, &occurrences),
                "penetration_depth": round6(depth),
                "penetration_axis": axis,
            }));
        }

        // Track nearest surface below for grounded detection
        // Check if other_aabb has a surface below our AABB min-Y
        if other_aabb.max.y <= target_aabb.min.y + max_float_gap
            && other_aabb.max.y >= target_aabb.min.y - 1.0
        {
            // Check X/Z overlap (must be somewhat underneath)
            if target_aabb.min.x <= other_aabb.max.x
                && target_aabb.max.x >= other_aabb.min.x
                && target_aabb.min.z <= other_aabb.max.z
                && target_aabb.max.z >= other_aabb.min.z
            {
                let gap = target_aabb.min.y - other_aabb.max.y;
                if nearest_below.is_none() || gap < nearest_below.unwrap().1 {
                    nearest_below = Some((*entity, gap));
                }
            }
        }
    }

    let sunken_penetration = ground_y
        .map(|gy| gy - target_aabb.min.y)
        .filter(|penetration| *penetration > 0.001);
    let on_ground_plane = ground_y.is_some_and(|gy| {
        let gap = target_aabb.min.y - gy;
        gap >= -0.001 && gap <= max_float_gap
    });
    let grounded = on_ground_plane
        || nearest_below
            .as_ref()
            .is_some_and(|(_, gap)| *gap <= max_float_gap);

    let mut result = serde_json::json!({
        "entity": entity_label_with(world, target, &occurrences),
        "overlap_count": overlaps.len(),
        "overlaps": overlaps,
        "grounded": grounded,
        "invalid_count": invalid_entities.len(),
        "invalid_entities": invalid_entities,
    });

    if let Some((below_entity, gap)) = nearest_below {
        result["nearest_surface_below"] = serde_json::json!({
            "entity": entity_label_with(world, below_entity, &occurrences),
            "gap": round6(gap),
        });
    } else {
        result["nearest_surface_below"] = serde_json::json!(null);
    }
    if !grounded && sunken_penetration.is_none() {
        result["floating"] = serde_json::json!(true);
    }

    // Ground penetration detection
    if ground_y.is_some() {
        if let Some(penetration) = sunken_penetration {
            result["sunken"] = serde_json::json!({
                "penetration_depth": round6(penetration),
                "world_aabb_min_y": round6(target_aabb.min.y),
            });
        } else {
            result["sunken"] = serde_json::json!(null);
        }
    }

    Ok(result)
}

/// Scene-wide overlap detection using sweep-and-prune on Y axis.
pub fn check_all_overlaps(
    world: &mut World,
    params: CheckAllOverlapsParams,
) -> Result<serde_json::Value, ControlError> {
    let CheckAllOverlapsParams {
        min_penetration,
        max_results,
        max_float_gap,
        ground_y,
        include_siblings,
    } = params;
    let min_pen = min_penetration.unwrap_or(0.001);
    let max_res = max_results.unwrap_or(100);

    // Collect all world AABBs
    let mut query_state =
        world.query::<(Entity, &bevy::camera::primitives::Aabb, &GlobalTransform)>();
    let entities: Vec<Entity> = query_state.iter(world).map(|(e, _, _)| e).collect();

    let mut aabbs: Vec<WorldAabb> = Vec::new();
    let mut invalid_entities = Vec::new();
    for entity in &entities {
        match compute_world_aabb(world, *entity) {
            Ok(aabb) => aabbs.push(aabb),
            Err(error) if error.code == crate::bridge::ErrorCode::InvalidParams => {
                invalid_entities.push(entity_label(world, *entity));
            }
            Err(_) => {}
        }
    }

    // Sort by AABB min-Y for sweep-and-prune
    aabbs.sort_by(|a, b| {
        a.min
            .y
            .partial_cmp(&b.min.y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let occurrences = NameOccurrences::collect(world);
    let mut overlaps = Vec::new();
    let mut floating_entities = Vec::new();

    // Sweep-and-prune: O(n log n) average case
    for i in 0..aabbs.len() {
        let mut has_ground_contact = false;

        for j in (i + 1)..aabbs.len() {
            // Prune: if min-Y of j is beyond max-Y of i, no more overlaps for i on this axis
            if aabbs[j].min.y > aabbs[i].max.y {
                break;
            }

            if aabbs_overlap(&aabbs[i], &aabbs[j]) {
                if !include_siblings {
                    // Skip overlaps between entities that share a common root ancestor
                    // (e.g., mesh children within the same GLB model hierarchy)
                    if share_hierarchy_root(world, aabbs[i].entity, aabbs[j].entity) {
                        has_ground_contact = true;
                        continue;
                    }
                }
                let (depth, axis) = compute_penetration(&aabbs[i], &aabbs[j]);
                if depth >= min_pen && overlaps.len() < max_res {
                    overlaps.push(serde_json::json!({
                        "entity_a": entity_label_with(world, aabbs[i].entity, &occurrences),
                        "entity_b": entity_label_with(world, aabbs[j].entity, &occurrences),
                        "penetration_depth": round6(depth),
                        "penetration_axis": axis,
                    }));
                }
                has_ground_contact = true;
            }
        }

        // Also check backward for ground contact
        if !has_ground_contact {
            for j in 0..i {
                if aabbs[j].max.y >= aabbs[i].min.y - max_float_gap
                    && aabbs[j].min.x <= aabbs[i].max.x
                    && aabbs[j].max.x >= aabbs[i].min.x
                    && aabbs[j].min.z <= aabbs[i].max.z
                    && aabbs[j].max.z >= aabbs[i].min.z
                {
                    has_ground_contact = true;
                    break;
                }
            }
        }

        let sunken = ground_y.is_some_and(|gy| gy - aabbs[i].min.y > 0.001);
        let on_ground_plane = ground_y.is_some_and(|gy| {
            let gap = aabbs[i].min.y - gy;
            gap >= -0.001 && gap <= max_float_gap
        });
        if !has_ground_contact && !on_ground_plane && !sunken && floating_entities.len() < 20 {
            floating_entities.push(entity_label_with(world, aabbs[i].entity, &occurrences));
        }
    }

    let mut result = serde_json::json!({
        "total_entities_with_aabb": aabbs.len(),
        "overlap_count": overlaps.len(),
        "overlaps": overlaps,
        "floating_count": floating_entities.len(),
        "floating_entities": floating_entities,
        "invalid_count": invalid_entities.len(),
        "invalid_entities": invalid_entities,
    });

    // Ground penetration detection
    if let Some(gy) = ground_y {
        let mut sunken_entities = Vec::new();
        for aabb in &aabbs {
            let penetration = gy - aabb.min.y;
            if penetration > 0.001 {
                sunken_entities.push(serde_json::json!({
                    "entity": entity_label_with(world, aabb.entity, &occurrences),
                    "penetration_depth": round6(penetration),
                    "world_aabb_min_y": round6(aabb.min.y),
                }));
            }
        }
        result["sunken_count"] = serde_json::json!(sunken_entities.len());
        result["sunken_entities"] = serde_json::json!(sunken_entities);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use bevy::{camera::primitives::Aabb, ecs::entity::Entity, math::Vec3A};

    use super::*;
    use crate::bridge::{EntityRef, ErrorCode};

    fn make_aabb(entity_bits: u64, min: [f32; 3], max: [f32; 3]) -> WorldAabb {
        WorldAabb {
            min: Vec3A::from_array(min),
            max: Vec3A::from_array(max),
            entity: Entity::from_bits(entity_bits),
        }
    }

    #[test]
    fn aabbs_overlap_overlapping() {
        let a = make_aabb(1, [0.0, 0.0, 0.0], [2.0, 2.0, 2.0]);
        let b = make_aabb(2, [1.0, 1.0, 1.0], [3.0, 3.0, 3.0]);
        assert!(aabbs_overlap(&a, &b));
    }

    #[test]
    fn aabbs_overlap_separated() {
        let a = make_aabb(1, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let b = make_aabb(2, [5.0, 5.0, 5.0], [6.0, 6.0, 6.0]);
        assert!(!aabbs_overlap(&a, &b));
    }

    #[test]
    fn aabbs_overlap_touching_edges() {
        let a = make_aabb(1, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let b = make_aabb(2, [1.0, 0.0, 0.0], [2.0, 1.0, 1.0]);
        assert!(aabbs_overlap(&a, &b));
    }

    #[test]
    fn aabbs_overlap_partial_two_axes() {
        // Overlap on X and Y but not Z
        let a = make_aabb(1, [0.0, 0.0, 0.0], [2.0, 2.0, 1.0]);
        let b = make_aabb(2, [1.0, 1.0, 3.0], [3.0, 3.0, 4.0]);
        assert!(!aabbs_overlap(&a, &b));
    }

    #[test]
    fn aabb_min_distance_overlapping_is_zero() {
        let a = make_aabb(1, [0.0, 0.0, 0.0], [2.0, 2.0, 2.0]);
        let b = make_aabb(2, [1.0, 1.0, 1.0], [3.0, 3.0, 3.0]);
        assert_eq!(aabb_min_distance(&a, &b), 0.0);
    }

    #[test]
    fn aabb_min_distance_gap_single_axis() {
        let a = make_aabb(1, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let b = make_aabb(2, [4.0, 0.0, 0.0], [5.0, 1.0, 1.0]);
        // Gap is 3.0 on X axis only
        assert!((aabb_min_distance(&a, &b) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn aabb_min_distance_gap_multiple_axes() {
        let a = make_aabb(1, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let b = make_aabb(2, [4.0, 5.0, 0.0], [5.0, 6.0, 1.0]);
        // Gap: dx=3, dy=4, dz=0 → sqrt(9+16) = 5
        assert!((aabb_min_distance(&a, &b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn compute_penetration_x_dominant() {
        // Overlap: X=0.5, Y=1.0, Z=1.0 → X is minimum
        let a = make_aabb(1, [0.0, 0.0, 0.0], [2.0, 2.0, 2.0]);
        let b = make_aabb(2, [1.5, 1.0, 1.0], [3.5, 3.0, 3.0]);
        let (depth, axis) = compute_penetration(&a, &b);
        assert_eq!(axis, "X");
        assert!((depth - 0.5).abs() < 1e-6);
    }

    #[test]
    fn compute_penetration_y_dominant() {
        // Overlap: X=1.0, Y=0.3, Z=1.0 → Y is minimum
        let a = make_aabb(1, [0.0, 0.0, 0.0], [2.0, 2.0, 2.0]);
        let b = make_aabb(2, [1.0, 1.7, 1.0], [3.0, 3.7, 3.0]);
        let (depth, axis) = compute_penetration(&a, &b);
        assert_eq!(axis, "Y");
        assert!((depth - 0.3).abs() < 1e-5);
    }

    #[test]
    fn compute_penetration_z_dominant() {
        // Overlap: X=1.0, Y=1.0, Z=0.2 → Z is minimum
        let a = make_aabb(1, [0.0, 0.0, 0.0], [2.0, 2.0, 2.0]);
        let b = make_aabb(2, [1.0, 1.0, 1.8], [3.0, 3.0, 3.8]);
        let (depth, axis) = compute_penetration(&a, &b);
        assert_eq!(axis, "Z");
        assert!((depth - 0.2).abs() < 1e-5);
    }

    #[test]
    fn describe_direction_pure_x() {
        assert_eq!(describe_direction(Vec3::new(5.0, 0.0, 0.0)), "+X (right)");
    }

    #[test]
    fn describe_direction_negative_y() {
        assert_eq!(describe_direction(Vec3::new(0.0, -3.0, 0.0)), "-Y (below)");
    }

    #[test]
    fn describe_direction_mixed() {
        let result = describe_direction(Vec3::new(5.0, 5.0, 0.0));
        assert!(result.contains("+X (right)"));
        assert!(result.contains("+Y (above)"));
    }

    #[test]
    fn describe_direction_zero_vector() {
        assert_eq!(
            describe_direction(Vec3::new(0.0, 0.0, 0.0)),
            "same position"
        );
    }

    #[test]
    fn describe_direction_rejects_non_finite_coordinates() {
        assert_eq!(
            describe_direction(Vec3::NAN),
            "invalid direction (non-finite coordinates)"
        );
    }

    #[test]
    fn describe_direction_small_below_threshold() {
        // dominant = 10.0, threshold = 1.5. x=0.5 is below threshold
        let result = describe_direction(Vec3::new(0.5, 10.0, 0.0));
        assert!(!result.contains("+X"));
        assert!(result.contains("+Y (above)"));
    }

    #[test]
    fn resolve_entity_by_id_found() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let entity_ref = EntityRef::Id(entity.to_bits());
        let result = resolve_entity(&mut world, &entity_ref);
        assert_eq!(result.unwrap(), entity);
    }

    #[test]
    fn resolve_entity_by_id_not_found() {
        let mut world = World::new();
        let entity_ref = EntityRef::Id(999999);
        let result = resolve_entity(&mut world, &entity_ref);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn resolve_entity_by_name_found() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("TestEntity")).id();
        let entity_ref = EntityRef::Name("TestEntity".into());
        let result = resolve_entity(&mut world, &entity_ref);
        assert_eq!(result.unwrap(), entity);
    }

    #[test]
    fn resolve_entity_by_name_not_found() {
        let mut world = World::new();
        let entity_ref = EntityRef::Name("NonExistent".into());
        let result = resolve_entity(&mut world, &entity_ref);
        assert!(result.is_err());
    }

    #[test]
    fn query_spatial_two_entities() {
        let mut world = World::new();
        let ea = world
            .spawn((
                Name::new("A"),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0)),
            ))
            .id();
        let eb = world
            .spawn((
                Name::new("B"),
                GlobalTransform::from(Transform::from_xyz(3.0, 4.0, 0.0)),
            ))
            .id();

        let result = super::query_spatial(
            &mut world,
            QuerySpatialParams {
                entity_a: EntityRef::Id(ea.to_bits()),
                entity_b: EntityRef::Id(eb.to_bits()),
            },
        )
        .unwrap();

        let dist = result["center_distance"].as_f64().unwrap();
        assert!((dist - 5.0).abs() < 1e-4); // 3-4-5 triangle

        let dir = result["direction_a_to_b"].as_str().unwrap();
        assert!(dir.contains("+X (right)"));
        assert!(dir.contains("+Y (above)"));
    }

    #[test]
    fn query_spatial_rejects_non_finite_global_transform() {
        let mut world = World::new();
        let invalid = world
            .spawn((
                Name::new("Invalid"),
                GlobalTransform::from(Transform::from_translation(Vec3::NAN)),
            ))
            .id();
        let valid = world
            .spawn((Name::new("Valid"), GlobalTransform::default()))
            .id();

        let error = query_spatial(
            &mut world,
            QuerySpatialParams {
                entity_a: EntityRef::Id(invalid.to_bits()),
                entity_b: EntityRef::Id(valid.to_bits()),
            },
        )
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(error.message.contains("Invalid"));
        assert!(error.message.contains("requires finite GlobalTransform"));
    }

    #[test]
    fn query_spatial_neighborhood_radius_filtering() {
        let mut world = World::new();
        let center = world
            .spawn((
                Name::new("Center"),
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0)),
            ))
            .id();
        world.spawn((
            Name::new("Near"),
            GlobalTransform::from(Transform::from_xyz(1.0, 0.0, 0.0)),
        ));
        world.spawn((
            Name::new("Far"),
            GlobalTransform::from(Transform::from_xyz(100.0, 0.0, 0.0)),
        ));

        let result = query_spatial_neighborhood(
            &mut world,
            QuerySpatialNeighborhoodParams {
                entity: EntityRef::Id(center.to_bits()),
                radius: 5.0,
                max_results: None,
            },
        )
        .unwrap();

        assert_eq!(result["count"], 1);
        let neighbors = result["neighbors"].as_array().unwrap();
        assert_eq!(neighbors.len(), 1);
        let name = neighbors[0]["entity"].as_str().unwrap();
        assert!(name.contains("Near"));
    }

    #[test]
    fn query_spatial_neighborhood_reports_non_finite_neighbors() {
        let mut world = World::new();
        let center = world
            .spawn((
                Name::new("Center"),
                Transform::default(),
                GlobalTransform::default(),
            ))
            .id();
        world.spawn((
            Name::new("Invalid"),
            GlobalTransform::from(Transform::from_translation(Vec3::NAN)),
        ));

        let result = query_spatial_neighborhood(
            &mut world,
            QuerySpatialNeighborhoodParams {
                entity: EntityRef::Id(center.to_bits()),
                radius: 5.0,
                max_results: None,
            },
        )
        .unwrap();

        assert_eq!(result["count"], 0);
        assert_eq!(result["invalid_count"], 1);
        assert!(
            result["invalid_entities"][0]
                .as_str()
                .unwrap()
                .contains("Invalid")
        );
    }

    #[test]
    fn query_spatial_neighborhood_rejects_invalid_radius() {
        for radius in [-1.0, f32::NEG_INFINITY, f32::INFINITY, f32::NAN] {
            let error = query_spatial_neighborhood(
                &mut World::new(),
                QuerySpatialNeighborhoodParams {
                    entity: EntityRef::Id(0),
                    radius,
                    max_results: None,
                },
            )
            .unwrap_err();

            assert_eq!(error.code, ErrorCode::InvalidParams);
            assert_eq!(error.message, "radius must be >= 0 and finite");
        }
    }

    #[test]
    fn query_spatial_neighborhood_sort_order() {
        let mut world = World::new();
        let center = world
            .spawn((
                Name::new("Center"),
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0)),
            ))
            .id();
        world.spawn((
            Name::new("Mid"),
            GlobalTransform::from(Transform::from_xyz(5.0, 0.0, 0.0)),
        ));
        world.spawn((
            Name::new("Close"),
            GlobalTransform::from(Transform::from_xyz(1.0, 0.0, 0.0)),
        ));

        let result = query_spatial_neighborhood(
            &mut world,
            QuerySpatialNeighborhoodParams {
                entity: EntityRef::Id(center.to_bits()),
                radius: 10.0,
                max_results: None,
            },
        )
        .unwrap();

        let neighbors = result["neighbors"].as_array().unwrap();
        assert_eq!(neighbors.len(), 2);
        // Closest first
        let d0 = neighbors[0]["distance"].as_f64().unwrap();
        let d1 = neighbors[1]["distance"].as_f64().unwrap();
        assert!(d0 < d1);
    }

    #[test]
    fn entity_label_with_name() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Cube")).id();
        let label = super::entity_label(&world, entity);
        assert!(label.contains("Cube"));
        assert!(label.contains(&entity.to_bits().to_string()));
    }

    #[test]
    fn entity_label_without_name() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let label = super::entity_label(&world, entity);
        assert_eq!(label, entity.to_bits().to_string());
    }

    #[test]
    fn compute_world_aabb_identity_transform() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
                GlobalTransform::default(),
            ))
            .id();

        let result = compute_world_aabb(&world, entity).unwrap();
        assert!((result.min.x - (-1.0)).abs() < 1e-5);
        assert!((result.min.y - (-1.0)).abs() < 1e-5);
        assert!((result.min.z - (-1.0)).abs() < 1e-5);
        assert!((result.max.x - 1.0).abs() < 1e-5);
        assert!((result.max.y - 1.0).abs() < 1e-5);
        assert!((result.max.z - 1.0).abs() < 1e-5);
    }

    #[test]
    fn compute_world_aabb_rejects_non_finite_transform() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
                GlobalTransform::from(Transform::from_translation(Vec3::NAN)),
            ))
            .id();

        let error = compute_world_aabb(&world, entity).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(error.message.contains("non-finite"));
    }

    #[test]
    fn compute_world_aabb_translated() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
                GlobalTransform::from(Transform::from_xyz(5.0, 0.0, 0.0)),
            ))
            .id();

        let result = compute_world_aabb(&world, entity).unwrap();
        assert!((result.min.x - 4.0).abs() < 1e-5);
        assert!((result.max.x - 6.0).abs() < 1e-5);
    }

    #[test]
    fn compute_world_aabb_no_aabb() {
        let mut world = World::new();
        let entity = world.spawn(GlobalTransform::default()).id();

        let result = compute_world_aabb(&world, entity);
        assert!(result.is_err());
    }

    #[test]
    fn compute_world_aabb_no_transform() {
        let mut world = World::new();
        let entity = world
            .spawn(Aabb::from_min_max(
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(1.0, 1.0, 1.0),
            ))
            .id();

        let result = compute_world_aabb(&world, entity);
        assert!(result.is_err());
    }

    #[test]
    fn check_overlaps_no_overlaps() {
        let mut world = World::new();
        let ea = world
            .spawn((
                Aabb::from_min_max(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0)),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0)),
            ))
            .id();
        world.spawn((
            Aabb::from_min_max(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0)),
            GlobalTransform::from(Transform::from_xyz(10.0, 0.0, 0.0)),
        ));

        let result = check_overlaps(
            &mut world,
            CheckOverlapsParams {
                entity: EntityRef::Id(ea.to_bits()),
                include_siblings: true,
                max_float_gap: 0.1,
                ground_y: None,
            },
        )
        .unwrap();

        assert_eq!(result["overlap_count"], 0);
    }

    #[test]
    fn check_overlaps_with_overlap() {
        let mut world = World::new();
        let ea = world
            .spawn((
                Aabb::from_min_max(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0)),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0)),
            ))
            .id();
        world.spawn((
            Aabb::from_min_max(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0)),
            GlobalTransform::from(Transform::from_xyz(1.0, 0.0, 0.0)),
        ));

        let result = check_overlaps(
            &mut world,
            CheckOverlapsParams {
                entity: EntityRef::Id(ea.to_bits()),
                include_siblings: true,
                max_float_gap: 0.1,
                ground_y: None,
            },
        )
        .unwrap();

        assert!(result["overlap_count"].as_u64().unwrap() > 0);
        let overlaps = result["overlaps"].as_array().unwrap();
        assert!(!overlaps.is_empty());
        assert!(overlaps[0]["penetration_depth"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn check_overlaps_excludes_target_descendants() {
        let mut world = World::new();
        let target = world
            .spawn((Name::new("model"), GlobalTransform::default()))
            .id();
        let child = world
            .spawn((
                Name::new("mesh"),
                Aabb::from_min_max(Vec3::splat(-0.5), Vec3::splat(0.5)),
                GlobalTransform::default(),
            ))
            .id();
        world.entity_mut(target).add_child(child);

        let result = check_overlaps(
            &mut world,
            CheckOverlapsParams {
                entity: EntityRef::Name("model".into()),
                include_siblings: true,
                max_float_gap: 0.1,
                ground_y: None,
            },
        )
        .unwrap();

        assert_eq!(result["overlap_count"], 0);
        assert_eq!(result["overlaps"], serde_json::json!([]));
    }

    #[test]
    fn parent_child_overlap_is_filtered_from_both_overlap_tools() {
        let mut world = World::new();
        let parent = world
            .spawn((
                Name::new("parent"),
                Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
                GlobalTransform::default(),
            ))
            .id();
        let child = world
            .spawn((
                Name::new("child"),
                Aabb::from_min_max(Vec3::splat(-0.5), Vec3::splat(0.5)),
                GlobalTransform::default(),
            ))
            .id();
        world.entity_mut(parent).add_child(child);

        let one = check_overlaps(
            &mut world,
            CheckOverlapsParams {
                entity: EntityRef::Name("child".into()),
                include_siblings: false,
                max_float_gap: 0.1,
                ground_y: None,
            },
        )
        .unwrap();
        assert_eq!(one["overlap_count"], 0);

        let all = check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: None,
                include_siblings: false,
            },
        )
        .unwrap();
        assert_eq!(all["overlap_count"], 0);
    }

    #[test]
    fn check_overlaps_entity_sibling_filtering_deep_hierarchy() {
        let mut world = World::new();

        // Reproduce the exact bug report structure:
        // root_a → branch_a1 → overlap_a_mesh (Cuboid)
        // root_a → branch_a2 → overlap_b_mesh (Cuboid, overlapping)
        let root = world
            .spawn((
                Name::new("root_a"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        let branch1 = world
            .spawn((
                Name::new("branch_a1"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        let branch2 = world
            .spawn((
                Name::new("branch_a2"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        let mesh_a = world
            .spawn((
                Name::new("overlap_a_mesh"),
                Aabb::from_min_max(Vec3::new(-0.6, -0.6, -0.6), Vec3::new(0.6, 0.6, 0.6)),
                GlobalTransform::default(),
            ))
            .id();
        let mesh_b = world
            .spawn((
                Name::new("overlap_b_mesh"),
                Aabb::from_min_max(Vec3::new(-0.45, -0.6, -0.6), Vec3::new(0.75, 0.6, 0.6)),
                GlobalTransform::default(),
            ))
            .id();

        world.entity_mut(root).add_children(&[branch1, branch2]);
        world.entity_mut(branch1).add_children(&[mesh_a]);
        world.entity_mut(branch2).add_children(&[mesh_b]);

        // include_siblings=true -> should report the overlap
        let result_with = check_overlaps(
            &mut world,
            CheckOverlapsParams {
                entity: EntityRef::Name("overlap_a_mesh".into()),
                include_siblings: true,
                max_float_gap: 0.1,
                ground_y: None,
            },
        )
        .unwrap();
        assert!(
            result_with["overlap_count"].as_u64().unwrap() > 0,
            "Expected overlap with include_siblings=true"
        );

        // include_siblings=false -> should filter (same root ancestor)
        let result_without = check_overlaps(
            &mut world,
            CheckOverlapsParams {
                entity: EntityRef::Name("overlap_a_mesh".into()),
                include_siblings: false,
                max_float_gap: 0.1,
                ground_y: None,
            },
        )
        .unwrap();
        assert_eq!(
            result_without["overlap_count"].as_u64().unwrap(),
            0,
            "Expected no overlap with include_siblings=false (same root ancestor)"
        );
    }

    #[test]
    fn check_overlaps_floating_entity() {
        let mut world = World::new();
        let floating = world
            .spawn((
                Aabb::from_min_max(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5)),
                GlobalTransform::from(Transform::from_xyz(0.0, 10.0, 0.0)),
            ))
            .id();

        let result = check_overlaps(
            &mut world,
            CheckOverlapsParams {
                entity: EntityRef::Id(floating.to_bits()),
                include_siblings: true,
                max_float_gap: 0.1,
                ground_y: None,
            },
        )
        .unwrap();

        assert_eq!(result["floating"], true);
        assert_eq!(result["grounded"], false);
    }

    #[test]
    fn check_overlaps_grounded_entity() {
        let mut world = World::new();
        // Ground: y=0..1
        world.spawn((
            Aabb::from_min_max(Vec3::new(-5.0, 0.0, -5.0), Vec3::new(5.0, 1.0, 5.0)),
            GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0)),
        ));
        // Entity sitting on top: y=1..2
        let entity = world
            .spawn((
                Aabb::from_min_max(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5)),
                GlobalTransform::from(Transform::from_xyz(0.0, 1.0, 0.0)),
            ))
            .id();

        let result = check_overlaps(
            &mut world,
            CheckOverlapsParams {
                entity: EntityRef::Id(entity.to_bits()),
                include_siblings: true,
                max_float_gap: 0.1,
                ground_y: None,
            },
        )
        .unwrap();

        assert_eq!(result["grounded"], true);
    }

    #[test]
    fn check_all_overlaps_empty_world() {
        let mut world = World::new();

        let result = super::check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: None,
                include_siblings: false,
            },
        )
        .unwrap();

        assert_eq!(result["total_entities_with_aabb"], 0);
        assert_eq!(result["overlap_count"], 0);
    }

    #[test]
    fn check_all_overlaps_no_overlaps() {
        let mut world = World::new();
        world.spawn((
            Aabb::from_min_max(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0)),
            GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0)),
        ));
        world.spawn((
            Aabb::from_min_max(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0)),
            GlobalTransform::from(Transform::from_xyz(10.0, 0.0, 0.0)),
        ));

        let result = super::check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: None,
                include_siblings: false,
            },
        )
        .unwrap();

        assert_eq!(result["overlap_count"], 0);
    }

    #[test]
    fn check_all_overlaps_with_overlaps() {
        let mut world = World::new();
        world.spawn((
            Aabb::from_min_max(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0)),
            GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0)),
        ));
        world.spawn((
            Aabb::from_min_max(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0)),
            GlobalTransform::from(Transform::from_xyz(1.0, 0.0, 0.0)),
        ));

        let result = super::check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: None,
                include_siblings: false,
            },
        )
        .unwrap();

        assert!(result["overlap_count"].as_u64().unwrap() > 0);
    }

    #[test]
    fn check_all_overlaps_floating_detection() {
        let mut world = World::new();
        // Entity high in the air with nothing below
        world.spawn((
            Aabb::from_min_max(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5)),
            GlobalTransform::from(Transform::from_xyz(0.0, 50.0, 0.0)),
        ));

        let result = super::check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: None,
                include_siblings: false,
            },
        )
        .unwrap();

        assert!(result["floating_count"].as_u64().unwrap() > 0);
    }

    #[test]
    fn check_all_overlaps_max_results() {
        let mut world = World::new();
        // Create many overlapping entities
        for i in 0..5 {
            world.spawn((
                Aabb::from_min_max(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0)),
                GlobalTransform::from(Transform::from_xyz(i as f32 * 0.5, 0.0, 0.0)),
            ));
        }

        let result = super::check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: Some(1),
                max_float_gap: 0.1,
                ground_y: None,
                include_siblings: false,
            },
        )
        .unwrap();

        let overlaps = result["overlaps"].as_array().unwrap();
        assert!(overlaps.len() <= 1);
    }

    #[test]
    fn describe_direction_negative_z() {
        let result = describe_direction(Vec3::new(0.0, 0.0, -5.0));
        assert!(result.contains("-Z (behind)"));
    }

    #[test]
    fn describe_direction_all_three_axes() {
        let result = describe_direction(Vec3::new(5.0, 5.0, 5.0));
        assert!(result.contains("+X (right)"));
        assert!(result.contains("+Y (above)"));
        assert!(result.contains("+Z (forward)"));
    }

    #[test]
    fn query_spatial_neighborhood_max_results_truncation() {
        let mut world = World::new();
        let center = world
            .spawn((
                Name::new("Center"),
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0)),
            ))
            .id();
        for i in 1..=5 {
            world.spawn((
                Name::new(format!("Neighbor{}", i)),
                GlobalTransform::from(Transform::from_xyz(i as f32, 0.0, 0.0)),
            ));
        }

        let result = query_spatial_neighborhood(
            &mut world,
            QuerySpatialNeighborhoodParams {
                entity: EntityRef::Id(center.to_bits()),
                radius: 10.0,
                max_results: Some(2),
            },
        )
        .unwrap();

        assert_eq!(result["count"], 2);
        assert_eq!(result["total_count"], 5);
        assert_eq!(result["truncated"], true);
    }

    #[test]
    fn query_spatial_missing_transform_errors() {
        let mut world = World::new();
        // Entity with GlobalTransform but no Transform (simulates removal)
        let entity = world
            .spawn((
                Name::new("NoTransform"),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0)),
            ))
            .id();

        let result = query_spatial_neighborhood(
            &mut world,
            QuerySpatialNeighborhoodParams {
                entity: EntityRef::Id(entity.to_bits()),
                radius: 5.0,
                max_results: None,
            },
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("no Transform"),
            "Error should mention missing Transform, got: {}",
            err.message
        );
    }

    #[test]
    fn query_spatial_neighborhood_no_neighbors() {
        let mut world = World::new();
        let center = world
            .spawn((
                Name::new("Lonely"),
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0)),
            ))
            .id();

        let result = query_spatial_neighborhood(
            &mut world,
            QuerySpatialNeighborhoodParams {
                entity: EntityRef::Id(center.to_bits()),
                radius: 5.0,
                max_results: None,
            },
        )
        .unwrap();

        assert_eq!(result["count"], 0);
    }

    #[test]
    fn compute_world_aabb_falls_back_to_children() {
        let mut world = World::new();
        // Parent without Aabb
        let parent = world
            .spawn((
                Name::new("Parent"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        // Child with Aabb
        let child = world
            .spawn((
                Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
                GlobalTransform::default(),
            ))
            .id();
        world.entity_mut(parent).add_children(&[child]);

        let result = compute_world_aabb(&world, parent).unwrap();
        assert!((result.min.x - (-1.0)).abs() < 1e-5);
        assert!((result.max.x - 1.0).abs() < 1e-5);
        assert_eq!(result.entity, parent);
    }

    #[test]
    fn compute_world_aabb_merges_multiple_children() {
        let mut world = World::new();
        let parent = world
            .spawn((
                Name::new("Parent"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        let child_a = world
            .spawn((
                Aabb::from_min_max(Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 1.0)),
                GlobalTransform::default(),
            ))
            .id();
        let child_b = world
            .spawn((
                Aabb::from_min_max(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 3.0, 1.0)),
                GlobalTransform::default(),
            ))
            .id();
        world.entity_mut(parent).add_children(&[child_a, child_b]);

        let result = compute_world_aabb(&world, parent).unwrap();
        // Merged: min=(-1,0,0), max=(2,3,1)
        assert!((result.min.x - (-1.0)).abs() < 1e-5);
        assert!((result.max.x - 2.0).abs() < 1e-5);
        assert!((result.max.y - 3.0).abs() < 1e-5);
    }

    #[test]
    fn compute_world_aabb_no_aabb_no_children_errors() {
        let mut world = World::new();
        let entity = world.spawn(GlobalTransform::default()).id();

        let result = compute_world_aabb(&world, entity);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("no descendants with Aabb")
        );
    }

    #[test]
    fn entity_label_generic_name_shows_parent() {
        let mut world = World::new();
        let parent = world
            .spawn((
                Name::new("tree_0"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        let child = world.spawn(Name::new("geometry_0.PBRMaterial")).id();
        world.entity_mut(parent).add_children(&[child]);

        let label = entity_label(&world, child);
        assert!(label.contains("geometry_0"));
        assert!(label.contains("[parent: tree_0]"));
    }

    #[test]
    fn entity_label_non_generic_name_unchanged() {
        let mut world = World::new();
        let parent = world
            .spawn((
                Name::new("scene_root"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        let child = world.spawn(Name::new("my_custom_mesh")).id();
        world.entity_mut(parent).add_children(&[child]);

        let label = entity_label(&world, child);
        assert!(label.contains("my_custom_mesh"));
        assert!(!label.contains("[parent:"));
    }

    #[test]
    fn entity_label_repeated_name_uses_unique_ancestor() {
        let mut world = World::new();
        let root_a = world.spawn(Name::new("model_a")).id();
        let root_b = world.spawn(Name::new("model_b")).id();
        let scene_a = world.spawn(Name::new("Scene")).id();
        let scene_b = world.spawn(Name::new("Scene")).id();
        let mesh_a = world.spawn(Name::new("cube")).id();
        let mesh_b = world.spawn(Name::new("cube")).id();
        world.entity_mut(root_a).add_child(scene_a);
        world.entity_mut(root_b).add_child(scene_b);
        world.entity_mut(scene_a).add_child(mesh_a);
        world.entity_mut(scene_b).add_child(mesh_b);

        assert!(entity_label(&world, mesh_a).contains("[parent: model_a]"));
        assert!(entity_label(&world, mesh_b).contains("[parent: model_b]"));
    }

    #[test]
    fn entity_label_with_prebuilt_occurrences_matches_per_call_labels() {
        let mut world = World::new();
        let root_a = world.spawn(Name::new("model_a")).id();
        let root_b = world.spawn(Name::new("model_b")).id();
        let mesh_a = world.spawn(Name::new("cube")).id();
        let mesh_b = world.spawn(Name::new("cube")).id();
        let unnamed = world.spawn_empty().id();
        world.entity_mut(root_a).add_child(mesh_a);
        world.entity_mut(root_b).add_child(mesh_b);

        let occurrences = NameOccurrences::collect(&world);
        for entity in [root_a, root_b, mesh_a, mesh_b, unnamed] {
            assert_eq!(
                entity_label_with(&world, entity, &occurrences),
                entity_label(&world, entity),
            );
        }
    }

    #[test]
    fn entity_label_no_parent_unchanged() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("geometry_0")).id();

        let label = entity_label(&world, entity);
        assert!(label.contains("geometry_0"));
        // No parent → no [parent: ...] suffix
        assert!(!label.contains("[parent:"));
    }

    #[test]
    fn check_overlaps_single_sunken() {
        let mut world = World::new();
        // Entity with center at origin, AABB goes from -1 to +1 on all axes
        // So min_y = -1.0, which is below ground_y=0.0
        let entity = world
            .spawn((
                Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
                GlobalTransform::default(),
            ))
            .id();

        let result = check_overlaps(
            &mut world,
            CheckOverlapsParams {
                entity: EntityRef::Id(entity.to_bits()),
                include_siblings: true,
                max_float_gap: 0.1,
                ground_y: Some(0.0),
            },
        )
        .unwrap();

        assert!(result["sunken"].is_object());
        let depth = result["sunken"]["penetration_depth"].as_f64().unwrap();
        assert!((depth - 1.0).abs() < 1e-5);
        assert!(result.get("floating").is_none());
    }

    #[test]
    fn check_overlaps_uses_ground_y_as_support_plane() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Aabb::from_min_max(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5)),
                GlobalTransform::default(),
            ))
            .id();

        let result = check_overlaps(
            &mut world,
            CheckOverlapsParams {
                entity: EntityRef::Id(entity.to_bits()),
                include_siblings: true,
                max_float_gap: 0.1,
                ground_y: Some(0.0),
            },
        )
        .unwrap();

        assert_eq!(result["grounded"], true);
        assert!(result.get("floating").is_none());
        assert!(result["sunken"].is_null());
    }

    #[test]
    fn check_overlaps_no_ground_y_omits_sunken() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
                GlobalTransform::default(),
            ))
            .id();

        let result = check_overlaps(
            &mut world,
            CheckOverlapsParams {
                entity: EntityRef::Id(entity.to_bits()),
                include_siblings: true,
                max_float_gap: 0.1,
                ground_y: None,
            },
        )
        .unwrap();

        assert!(result.get("sunken").is_none());
    }

    #[test]
    fn check_all_overlaps_sunken_detection() {
        let mut world = World::new();
        // Entity centered at origin: AABB min_y = -1.0
        world.spawn((
            Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
            GlobalTransform::default(),
        ));

        let result = super::check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: Some(0.0),
                include_siblings: false,
            },
        )
        .unwrap();

        assert_eq!(result["sunken_count"], 1);
        let sunken = result["sunken_entities"].as_array().unwrap();
        assert_eq!(sunken.len(), 1);
        assert_eq!(result["floating_count"], 0);
    }

    #[test]
    fn check_all_overlaps_reports_non_finite_entities_separately() {
        let mut world = World::new();
        world.spawn((
            Name::new("Invalid"),
            Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
            GlobalTransform::from(Transform::from_translation(Vec3::NAN)),
        ));

        let result = check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: Some(0.0),
                include_siblings: false,
            },
        )
        .unwrap();

        assert_eq!(result["invalid_count"], 1);
        assert_eq!(result["floating_count"], 0);
        assert_eq!(result["sunken_count"], 0);
        assert!(
            result["invalid_entities"][0]
                .as_str()
                .unwrap()
                .contains("Invalid")
        );
    }

    #[test]
    fn check_all_overlaps_uses_ground_y_as_support_plane() {
        let mut world = World::new();
        world.spawn((
            Aabb::from_min_max(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5)),
            GlobalTransform::default(),
        ));

        let result = super::check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: Some(0.0),
                include_siblings: false,
            },
        )
        .unwrap();

        assert_eq!(result["floating_count"], 0);
        assert_eq!(result["sunken_count"], 0);
    }

    #[test]
    fn check_all_overlaps_no_sunken_above_ground() {
        let mut world = World::new();
        // Entity fully above ground
        world.spawn((
            Aabb::from_min_max(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 2.0, 1.0)),
            GlobalTransform::default(),
        ));

        let result = super::check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: Some(0.0),
                include_siblings: false,
            },
        )
        .unwrap();

        assert_eq!(result["sunken_count"], 0);
    }

    #[test]
    fn check_all_overlaps_no_ground_y_omits_fields() {
        let mut world = World::new();
        world.spawn((
            Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
            GlobalTransform::default(),
        ));

        let result = super::check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: None,
                include_siblings: false,
            },
        )
        .unwrap();

        assert!(result.get("sunken_count").is_none());
        assert!(result.get("sunken_entities").is_none());
    }

    #[test]
    fn check_all_overlaps_sibling_filtering_deep_hierarchy() {
        let mut world = World::new();

        // Simulate GLB model: Root → Bone1 → Mesh1, Root → Bone2 → Mesh2
        let root = world
            .spawn((
                Name::new("model_root"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        let bone1 = world
            .spawn((
                Name::new("bone1"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        let bone2 = world
            .spawn((
                Name::new("bone2"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        // Two overlapping mesh children under different bones but same root
        let mesh1 = world
            .spawn((
                Name::new("mesh1"),
                Aabb::from_min_max(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 2.0, 1.0)),
                GlobalTransform::default(),
            ))
            .id();
        let mesh2 = world
            .spawn((
                Name::new("mesh2"),
                Aabb::from_min_max(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.5, 0.5)),
                GlobalTransform::default(),
            ))
            .id();

        world.entity_mut(root).add_children(&[bone1, bone2]);
        world.entity_mut(bone1).add_children(&[mesh1]);
        world.entity_mut(bone2).add_children(&[mesh2]);

        // With include_siblings=true, overlaps should be reported
        let result_with = super::check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: None,
                include_siblings: true,
            },
        )
        .unwrap();
        assert!(
            result_with["overlap_count"].as_u64().unwrap() > 0,
            "Expected overlaps with include_siblings=true"
        );

        // With include_siblings=false, sibling overlaps should be filtered
        let result_without = super::check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: None,
                include_siblings: false,
            },
        )
        .unwrap();
        assert_eq!(
            result_without["overlap_count"].as_u64().unwrap(),
            0,
            "Expected no overlaps with include_siblings=false (same root ancestor)"
        );
    }

    #[test]
    // 3.141593 (the expected output) is PI rounded to 6 dp — no const form, so approx_constant
    // fires on the literal even though the input uses std::f32::consts::PI directly.
    #[allow(clippy::approx_constant)]
    fn round6_basic() {
        assert_eq!(super::round6(std::f32::consts::PI), 3.141593);
        assert_eq!(super::round6(0.0), 0.0);
        assert_eq!(super::round6(-1.23456789), -1.234568);
    }

    #[test]
    fn round6_f64_basic() {
        assert_eq!(super::round6_f64(0.0024348448496311903), 0.002435);
    }

    #[test]
    fn compute_penetration_cube_straddles_flat_ground() {
        // Separating the cube from the plane requires 1.0 on Y, but 16.0 on X or Z.
        let a = make_aabb(1, [-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        let b = make_aabb(2, [-15.0, 0.0, -15.0], [15.0, 0.0, 15.0]);
        let (depth, axis) = compute_penetration(&a, &b);
        assert_eq!(axis, "Y");
        assert!((depth - 1.0).abs() < 1e-5);
    }

    #[test]
    fn compute_penetration_no_degenerate_picks_min() {
        // Regression: all axes eligible, smallest overlap (Y=0.3) wins.
        let a = make_aabb(1, [0.0, 0.0, 0.0], [2.0, 2.0, 2.0]);
        let b = make_aabb(2, [1.0, 1.7, 1.0], [3.0, 3.7, 3.0]);
        let (depth, axis) = compute_penetration(&a, &b);
        assert_eq!(axis, "Y");
        assert!((depth - 0.3).abs() < 1e-5);
    }

    #[test]
    fn compute_penetration_flat_axis_uses_separation_distance() {
        // Separating on X requires 1.0, while separating on Y requires 0.5.
        let a = make_aabb(1, [0.0, 0.0, 0.0], [2.0, 2.0, 2.0]);
        let b = make_aabb(2, [1.0, 1.5, 1.0], [1.0, 4.0, 3.0]);
        let (depth, axis) = compute_penetration(&a, &b);
        assert_eq!(axis, "Y");
        assert!((depth - 0.5).abs() < 1e-5);
    }

    #[test]
    fn compute_penetration_all_degenerate_no_panic() {
        // Two coincident points: no eligible axis, fallback picks min overlap (all 0).
        let a = make_aabb(1, [1.0, 2.0, 3.0], [1.0, 2.0, 3.0]);
        let b = make_aabb(2, [1.0, 2.0, 3.0], [1.0, 2.0, 3.0]);
        let (depth, axis) = compute_penetration(&a, &b);
        assert_eq!(depth, 0.0);
        assert!(axis == "X" || axis == "Y" || axis == "Z");
    }

    #[test]
    fn compute_penetration_thin_wall_uses_contact_normal() {
        // Moving the cube 1.0 on X separates it from the wall; Y and Z require 6.0.
        let a = make_aabb(1, [-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        let b = make_aabb(2, [0.0, -5.0, -5.0], [0.0, 5.0, 5.0]);
        let (depth, axis) = compute_penetration(&a, &b);
        assert_eq!(axis, "X");
        assert!((depth - 1.0).abs() < 1e-5);
    }

    #[test]
    fn check_all_overlaps_sibling_filtering_different_roots() {
        let mut world = World::new();

        // Two independent root entities with overlapping children
        let root_a = world
            .spawn((
                Name::new("root_a"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        let root_b = world
            .spawn((
                Name::new("root_b"),
                GlobalTransform::default(),
                Transform::default(),
            ))
            .id();
        let child_a = world
            .spawn((
                Name::new("child_a"),
                Aabb::from_min_max(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 2.0, 1.0)),
                GlobalTransform::default(),
            ))
            .id();
        let child_b = world
            .spawn((
                Name::new("child_b"),
                Aabb::from_min_max(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.5, 0.5)),
                GlobalTransform::default(),
            ))
            .id();

        world.entity_mut(root_a).add_children(&[child_a]);
        world.entity_mut(root_b).add_children(&[child_b]);

        // Different roots - overlap should still be reported even with include_siblings=false
        let result = super::check_all_overlaps(
            &mut world,
            CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: None,
                include_siblings: false,
            },
        )
        .unwrap();
        assert!(
            result["overlap_count"].as_u64().unwrap() > 0,
            "Overlaps between different root hierarchies should not be filtered"
        );
    }
}
