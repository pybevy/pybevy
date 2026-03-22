use bevy::{
    ecs::{entity::Entity, name::Name, world::World},
    math::Vec3A,
    prelude::*,
};

use super::scene::resolve_entity;
use crate::bridge::{ControlError, EntityRef};

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
fn compute_entity_aabb(world: &World, entity: Entity) -> Option<WorldAabb> {
    let aabb = world.get::<bevy::camera::primitives::Aabb>(entity)?;
    let gt = world.get::<GlobalTransform>(entity)?;

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

    Some(WorldAabb {
        min: world_min,
        max: world_max,
        entity,
    })
}

/// Recursively collect world AABBs from all descendants that have Aabb.
fn collect_descendant_aabbs(world: &World, entity: Entity) -> Vec<WorldAabb> {
    let mut result = Vec::new();
    let Some(children) = world.get::<Children>(entity) else {
        return result;
    };
    for child in children.iter() {
        if let Some(aabb) = compute_entity_aabb(world, child) {
            result.push(aabb);
        }
        // Recurse into grandchildren
        result.extend(collect_descendant_aabbs(world, child));
    }
    result
}

/// Compute world-space AABB by transforming local Aabb corners via GlobalTransform.
/// Falls back to merging descendant AABBs for SceneRoot/hierarchy entities.
pub fn compute_world_aabb(world: &World, entity: Entity) -> Result<WorldAabb, ControlError> {
    // Fast path: entity has its own Aabb
    if let Some(aabb) = compute_entity_aabb(world, entity) {
        return Ok(aabb);
    }

    // Fallback: merge AABBs from descendants (handles SceneRoot/GLB hierarchies)
    let descendant_aabbs = collect_descendant_aabbs(world, entity);
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
    let overlap_x = (a.max.x.min(b.max.x) - a.min.x.max(b.min.x)).max(0.0);
    let overlap_y = (a.max.y.min(b.max.y) - a.min.y.max(b.min.y)).max(0.0);
    let overlap_z = (a.max.z.min(b.max.z) - a.min.z.max(b.min.z)).max(0.0);

    // The penetration depth is the minimum overlap (shallowest axis)
    if overlap_x <= overlap_y && overlap_x <= overlap_z {
        (overlap_x, "X")
    } else if overlap_y <= overlap_z {
        (overlap_y, "Y")
    } else {
        (overlap_z, "Z")
    }
}

/// Human-readable direction description.
/// Threshold: ignore axes < 15% of dominant axis magnitude.
pub fn describe_direction(dir: Vec3) -> String {
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

/// Walk up ChildOf chain (max 10 levels) to find first non-generic named ancestor.
pub fn find_ancestor_name(world: &World, entity: Entity) -> Option<String> {
    let mut current = entity;
    for _ in 0..10 {
        let Some(child_of) = world.get::<ChildOf>(current) else {
            return None;
        };
        let parent = child_of.parent();
        if let Some(name) = world.get::<Name>(parent) {
            if !is_generic_name(Some(name.as_str())) {
                return Some(name.as_str().to_string());
            }
        }
        current = parent;
    }
    None
}

/// Get entity name or ID label for display.
/// For generic GLB mesh children, appends the parent's name for context.
pub fn entity_label(world: &World, entity: Entity) -> String {
    let name = world.get::<Name>(entity);
    let name_str = name.as_ref().map(|n| n.as_str());

    let base = match name_str {
        Some(n) => format!("\"{}\" ({})", n, entity.to_bits()),
        None => format!("{}", entity.to_bits()),
    };

    // Append parent context for generic or unnamed entities that have a parent
    if is_generic_name(name_str) {
        if let Some(ancestor) = find_ancestor_name(world, entity) {
            return format!("{} [parent: {}]", base, ancestor);
        }
    }

    base
}

// ── Tool handlers ──

/// Pairwise spatial query: distance, direction, AABB overlap between two entities.
pub fn query_spatial(
    world: &mut World,
    entity_a: EntityRef,
    entity_b: EntityRef,
) -> Result<serde_json::Value, ControlError> {
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
    entity_ref: EntityRef,
    radius: f32,
    max_results: Option<usize>,
) -> Result<serde_json::Value, ControlError> {
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

    // Collect all entities with GlobalTransform
    let mut query_state = world.query::<(Entity, &GlobalTransform)>();
    let mut neighbors: Vec<(Entity, f32, Vec3)> = Vec::new();

    for (entity, gt) in query_state.iter(world) {
        if entity == center_entity {
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
    let truncated = neighbors.len() > limit;
    let neighbors: Vec<_> = neighbors.into_iter().take(limit).collect();

    let results: Vec<serde_json::Value> = neighbors
        .iter()
        .map(|(entity, dist, pos)| {
            let dir = *pos - center_pos;
            serde_json::json!({
                "entity": entity_label(world, *entity),
                "distance": round6(*dist),
                "position": [round6(pos.x), round6(pos.y), round6(pos.z)],
                "direction": describe_direction(dir),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "center": entity_label(world, center_entity),
        "center_position": [round6(center_pos.x), round6(center_pos.y), round6(center_pos.z)],
        "radius": radius,
        "count": results.len(),
        "truncated": truncated,
        "neighbors": results,
    }))
}

/// Check overlaps for a single entity against all others.
pub fn check_overlaps(
    world: &mut World,
    entity_ref: EntityRef,
    include_siblings: bool,
    max_float_gap: f32,
    ground_y: Option<f32>,
) -> Result<serde_json::Value, ControlError> {
    let target = resolve_entity(world, &entity_ref)?;
    let target_aabb = compute_world_aabb(world, target)?;

    // Find root ancestor for sibling filtering (walk full hierarchy, not just direct parent)
    let target_root = if !include_siblings && world.get::<ChildOf>(target).is_some() {
        Some(find_root_ancestor(world, target))
    } else {
        None
    };

    // Collect all entities with Aabb + GlobalTransform
    let mut query_state =
        world.query::<(Entity, &bevy::camera::primitives::Aabb, &GlobalTransform)>();
    let all_entities: Vec<Entity> = query_state.iter(world).map(|(e, _, _)| e).collect();

    let mut overlaps = Vec::new();
    let mut nearest_below: Option<(Entity, f32)> = None;

    for entity in &all_entities {
        if *entity == target {
            continue;
        }

        // Skip entities sharing the same root ancestor (parented parts overlap by design)
        if let Some(root) = target_root {
            if world.get::<ChildOf>(*entity).is_some() && find_root_ancestor(world, *entity) == root
            {
                continue;
            }
        }

        let Ok(other_aabb) = compute_world_aabb(world, *entity) else {
            continue;
        };

        if aabbs_overlap(&target_aabb, &other_aabb) {
            let (depth, axis) = compute_penetration(&target_aabb, &other_aabb);
            overlaps.push(serde_json::json!({
                "entity": entity_label(world, *entity),
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

    let grounded = nearest_below
        .as_ref()
        .is_some_and(|(_, gap)| *gap <= max_float_gap);

    let mut result = serde_json::json!({
        "entity": entity_label(world, target),
        "overlap_count": overlaps.len(),
        "overlaps": overlaps,
        "grounded": grounded,
    });

    if let Some((below_entity, gap)) = nearest_below {
        result["nearest_surface_below"] = serde_json::json!({
            "entity": entity_label(world, below_entity),
            "gap": round6(gap),
        });
    } else {
        result["nearest_surface_below"] = serde_json::json!(null);
        if !grounded {
            result["floating"] = serde_json::json!(true);
        }
    }

    // Ground penetration detection
    if let Some(gy) = ground_y {
        let penetration = gy - target_aabb.min.y;
        if penetration > 0.001 {
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
    min_penetration: Option<f32>,
    max_results: Option<usize>,
    max_float_gap: f32,
    ground_y: Option<f32>,
    include_siblings: bool,
) -> Result<serde_json::Value, ControlError> {
    let min_pen = min_penetration.unwrap_or(0.001);
    let max_res = max_results.unwrap_or(100);

    // Collect all world AABBs
    let mut query_state =
        world.query::<(Entity, &bevy::camera::primitives::Aabb, &GlobalTransform)>();
    let entities: Vec<Entity> = query_state.iter(world).map(|(e, _, _)| e).collect();

    let mut aabbs: Vec<WorldAabb> = Vec::new();
    for entity in &entities {
        if let Ok(aabb) = compute_world_aabb(world, *entity) {
            aabbs.push(aabb);
        }
    }

    // Sort by AABB min-Y for sweep-and-prune
    aabbs.sort_by(|a, b| {
        a.min
            .y
            .partial_cmp(&b.min.y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

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
                    let has_parent_i = world.get::<ChildOf>(aabbs[i].entity).is_some();
                    let has_parent_j = world.get::<ChildOf>(aabbs[j].entity).is_some();
                    if has_parent_i && has_parent_j {
                        let root_i = find_root_ancestor(world, aabbs[i].entity);
                        let root_j = find_root_ancestor(world, aabbs[j].entity);
                        if root_i == root_j {
                            has_ground_contact = true;
                            continue;
                        }
                    }
                }
                let (depth, axis) = compute_penetration(&aabbs[i], &aabbs[j]);
                if depth >= min_pen && overlaps.len() < max_res {
                    overlaps.push(serde_json::json!({
                        "entity_a": entity_label(world, aabbs[i].entity),
                        "entity_b": entity_label(world, aabbs[j].entity),
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

        if !has_ground_contact && floating_entities.len() < 20 {
            floating_entities.push(entity_label(world, aabbs[i].entity));
        }
    }

    let mut result = serde_json::json!({
        "total_entities_with_aabb": aabbs.len(),
        "overlap_count": overlaps.len(),
        "overlaps": overlaps,
        "floating_count": floating_entities.len(),
        "floating_entities": floating_entities,
    });

    // Ground penetration detection
    if let Some(gy) = ground_y {
        let mut sunken_entities = Vec::new();
        for aabb in &aabbs {
            let penetration = gy - aabb.min.y;
            if penetration > 0.001 {
                sunken_entities.push(serde_json::json!({
                    "entity": entity_label(world, aabb.entity),
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
