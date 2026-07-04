use bevy::{
    ecs::{entity::Entity, name::Name, world::World},
    math::Vec3A,
    prelude::*,
};

use super::pyo3::scene::resolve_entity;
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
/// Falls back to merging descendant AABBs for WorldAssetRoot/hierarchy entities.
pub fn compute_world_aabb(world: &World, entity: Entity) -> Result<WorldAabb, ControlError> {
    // Fast path: entity has its own Aabb
    if let Some(aabb) = compute_entity_aabb(world, entity) {
        return Ok(aabb);
    }

    // Fallback: merge AABBs from descendants (handles WorldAssetRoot/GLB hierarchies)
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
/// Skips axes where either AABB has near-zero extent (e.g. flat plane geometry).
pub fn compute_penetration(a: &WorldAabb, b: &WorldAabb) -> (f32, &'static str) {
    const EPS: f32 = 1e-4;

    let overlap_x = (a.max.x.min(b.max.x) - a.min.x.max(b.min.x)).max(0.0);
    let overlap_y = (a.max.y.min(b.max.y) - a.min.y.max(b.min.y)).max(0.0);
    let overlap_z = (a.max.z.min(b.max.z) - a.min.z.max(b.min.z)).max(0.0);

    let size_a_x = a.max.x - a.min.x;
    let size_a_y = a.max.y - a.min.y;
    let size_a_z = a.max.z - a.min.z;
    let size_b_x = b.max.x - b.min.x;
    let size_b_y = b.max.y - b.min.y;
    let size_b_z = b.max.z - b.min.z;

    let axis_eligible = |sa: f32, sb: f32| sa > EPS && sb > EPS;
    let elig_x = axis_eligible(size_a_x, size_b_x);
    let elig_y = axis_eligible(size_a_y, size_b_y);
    let elig_z = axis_eligible(size_a_z, size_b_z);

    // Fallback when no axis is eligible: preserve original min-overlap selection.
    if !elig_x && !elig_y && !elig_z {
        if overlap_x <= overlap_y && overlap_x <= overlap_z {
            return (overlap_x, "X");
        } else if overlap_y <= overlap_z {
            return (overlap_y, "Y");
        } else {
            return (overlap_z, "Z");
        }
    }

    // Pick minimum overlap among eligible axes only.
    let mut best: Option<(f32, &'static str)> = None;
    let mut consider = |elig: bool, depth: f32, name: &'static str| {
        if !elig {
            return;
        }
        match best {
            Some((d, _)) if depth >= d => {}
            _ => best = Some((depth, name)),
        }
    };
    consider(elig_x, overlap_x, "X");
    consider(elig_y, overlap_y, "Y");
    consider(elig_z, overlap_z, "Z");
    best.unwrap()
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
        let child_of = world.get::<ChildOf>(current)?;
        let parent = child_of.parent();
        if let Some(name) = world.get::<Name>(parent)
            && !is_generic_name(Some(name.as_str()))
        {
            return Some(name.as_str().to_string());
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
    if is_generic_name(name_str)
        && let Some(ancestor) = find_ancestor_name(world, entity)
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
    params: CheckOverlapsParams,
) -> Result<serde_json::Value, ControlError> {
    let CheckOverlapsParams {
        entity: entity_ref,
        include_siblings,
        max_float_gap,
        ground_y,
    } = params;
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
        if let Some(root) = target_root
            && world.get::<ChildOf>(*entity).is_some()
            && find_root_ancestor(world, *entity) == root
        {
            continue;
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
    fn round6_basic() {
        assert_eq!(super::round6(3.1415927), 3.141593);
        assert_eq!(super::round6(0.0), 0.0);
        assert_eq!(super::round6(-1.23456789), -1.234568);
    }

    #[test]
    fn round6_f64_basic() {
        assert_eq!(super::round6_f64(0.0024348448496311903), 0.002435);
    }

    #[test]
    fn compute_penetration_cube_straddles_flat_ground() {
        // Cube straddling a flat plane (Y extent = 0). Must not pick the degenerate Y axis.
        // Cube is fully contained on X and Z within the plane, so overlap on those axes is 2.0.
        let a = make_aabb(1, [-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        let b = make_aabb(2, [-15.0, 0.0, -15.0], [15.0, 0.0, 15.0]);
        let (depth, axis) = compute_penetration(&a, &b);
        assert_ne!(axis, "Y", "Y is degenerate and must be skipped");
        assert!(axis == "X" || axis == "Z");
        assert!(depth > 0.0, "expected positive depth, got {}", depth);
        assert!(
            (depth - 2.0).abs() < 1e-5,
            "expected depth ~2.0, got {}",
            depth
        );
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
    fn compute_penetration_one_axis_degenerate_excluded() {
        // b is flat on X. Eligible axes Y (overlap 0.5) and Z (overlap 1.0). Y wins.
        let a = make_aabb(1, [0.0, 0.0, 0.0], [2.0, 2.0, 2.0]);
        let b = make_aabb(2, [1.0, 1.5, 1.0], [1.0, 4.0, 3.0]);
        let (depth, axis) = compute_penetration(&a, &b);
        assert_ne!(axis, "X");
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
    fn compute_penetration_thin_wall_skips_x() {
        // Vertical thin wall: b has degenerate X. Picked axis must be Y or Z.
        let a = make_aabb(1, [-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        let b = make_aabb(2, [0.0, -5.0, -5.0], [0.0, 5.0, 5.0]);
        let (_depth, axis) = compute_penetration(&a, &b);
        assert_ne!(axis, "X");
        assert!(axis == "Y" || axis == "Z");
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
