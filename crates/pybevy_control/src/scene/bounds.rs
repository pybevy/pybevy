use bevy::{
    camera::primitives::Aabb, ecs::world::World, math::Vec3A,
    transform::components::GlobalTransform,
};

use crate::{
    protocol::{ControlError, EntityRef},
    scene::{
        entity::resolve_entity,
        spatial::{compute_world_aabb, round6},
    },
};

/// Get the bounding box (AABB) of an entity, both local and world-space.
/// Falls back to merging descendant AABBs for WorldAssetRoot/GLB hierarchy entities.
pub fn get_bounding_box(
    world: &mut World,
    entity_ref: EntityRef,
) -> Result<serde_json::Value, ControlError> {
    let entity = resolve_entity(world, &entity_ref)?;

    // Try entity's own Aabb first
    if let Some(aabb) = world.get::<Aabb>(entity) {
        let center = aabb.center;
        let half = aabb.half_extents;
        let local_min = center - half;
        let local_max = center + half;

        let r = round6;
        let local = serde_json::json!({
            "center": [r(center.x), r(center.y), r(center.z)],
            "half_extents": [r(half.x), r(half.y), r(half.z)],
            "min": [r(local_min.x), r(local_min.y), r(local_min.z)],
            "max": [r(local_max.x), r(local_max.y), r(local_max.z)],
        });

        // Compute world-space bounds by transforming AABB corners
        let world_bounds = if let Some(gt) = world.get::<GlobalTransform>(entity) {
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
    let merged = compute_world_aabb(world, entity)
        .map_err(|_| ControlError::not_found("Entity has no Aabb (no mesh?)"))?;

    let world_center = (merged.min + merged.max) * 0.5;
    let world_size = merged.max - merged.min;
    let r = round6;

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

#[cfg(test)]
mod tests {
    use bevy::{ecs::name::Name, math::Vec3, transform::components::Transform};

    use super::*;

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
}
