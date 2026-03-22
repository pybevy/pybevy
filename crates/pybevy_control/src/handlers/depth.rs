use bevy::{
    ecs::{entity::Entity, name::Name, world::World},
    math::{Ray3d, Vec3},
    prelude::*,
};

use super::spatial::compute_world_aabb;
use crate::bridge::ControlError;

/// Compute depth samples by casting rays from a camera position through sample points
/// against all entity world AABBs.
///
/// If position/look_at are None, tries to use the active scene camera.
/// If sample_points is None, generates an NxN grid based on grid_density.
pub fn compute_depth_samples(
    world: &mut World,
    position: &Option<[f32; 3]>,
    look_at: &Option<[f32; 3]>,
    sample_points: &Option<Vec<[u32; 2]>>,
    grid_density: &Option<u32>,
) -> Result<serde_json::Value, ControlError> {
    // Determine camera position and orientation
    let (cam_pos, cam_forward, cam_right, cam_up) = if let Some(pos) = position {
        let p = Vec3::from_array(*pos);
        let target = Vec3::from_array(look_at.unwrap_or([0.0, 0.0, 0.0]));
        let forward = (target - p).normalize_or_zero();
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        (p, forward, right, up)
    } else {
        // Try to find active scene camera
        let mut query = world.query::<(&Camera, &GlobalTransform)>();
        let mut found = None;
        for (cam, gt) in query.iter(world) {
            if cam.is_active {
                let t = gt.compute_transform();
                found = Some((
                    t.translation,
                    t.forward().as_vec3(),
                    t.right().as_vec3(),
                    t.up().as_vec3(),
                ));
                break;
            }
        }
        found.ok_or_else(|| {
            ControlError::not_found("No active camera found and no position specified")
        })?
    };

    // Generate sample points
    let density = grid_density.unwrap_or(8);
    let points: Vec<[u32; 2]> = if let Some(pts) = sample_points {
        pts.clone()
    } else {
        // Generate NxN grid
        let mut pts = Vec::new();
        for row in 0..density {
            for col in 0..density {
                pts.push([col, row]);
            }
        }
        pts
    };

    // For grid-based sampling, convert grid coords to normalized screen coords
    // and then to ray directions using a simple perspective model
    let fov_half_tan = (30.0_f32).to_radians().tan(); // ~60° FOV

    // Collect all entity AABBs
    let mut aabb_query =
        world.query::<(Entity, &bevy::camera::primitives::Aabb, &GlobalTransform)>();
    let entities: Vec<Entity> = aabb_query.iter(world).map(|(e, _, _)| e).collect();

    let mut aabbs = Vec::new();
    for entity in &entities {
        if let Ok(aabb) = compute_world_aabb(world, *entity) {
            aabbs.push(aabb);
        }
    }

    // Cast rays and find intersections
    let mut samples = Vec::new();
    let grid_size = if sample_points.is_some() {
        800.0
    } else {
        density as f32
    };

    for point in &points {
        // Convert to normalized coordinates (-1 to 1)
        let nx = (point[0] as f32 / grid_size) * 2.0 - 1.0;
        let ny = -((point[1] as f32 / grid_size) * 2.0 - 1.0); // flip Y

        // Compute ray direction
        let dir = (cam_forward + cam_right * nx * fov_half_tan + cam_up * ny * fov_half_tan)
            .normalize_or_zero();

        let ray = Ray3d::new(cam_pos, Dir3::new(dir).unwrap_or(Dir3::NEG_Z));

        // Find nearest AABB intersection
        let mut nearest_hit: Option<(Entity, f32)> = None;

        for aabb in &aabbs {
            if let Some(t) = ray_aabb_intersection(&ray, aabb)
                && t > 0.0
                && (nearest_hit.is_none() || t < nearest_hit.unwrap().1)
            {
                nearest_hit = Some((aabb.entity, t));
            }
        }

        let sample = if let Some((entity, distance)) = nearest_hit {
            let hit_pos = cam_pos + dir * distance;
            let name = world.get::<Name>(entity).map(|n| n.as_str().to_string());
            let label = super::spatial::entity_label(world, entity);
            serde_json::json!({
                "screen": [point[0], point[1]],
                "hit": true,
                "distance": distance,
                "world_position": [hit_pos.x, hit_pos.y, hit_pos.z],
                "entity_id": entity.to_bits(),
                "entity_name": name,
                "entity_label": label,
            })
        } else {
            serde_json::json!({
                "screen": [point[0], point[1]],
                "hit": false,
                "distance": null,
            })
        };

        samples.push(sample);
    }

    let hit_count = samples.iter().filter(|s| s["hit"] == true).count();

    Ok(serde_json::json!({
        "sample_count": samples.len(),
        "hit_count": hit_count,
        "camera_position": [cam_pos.x, cam_pos.y, cam_pos.z],
        "samples": samples,
    }))
}

/// Ray-AABB intersection using the slab method.
/// Returns the distance along the ray to the nearest intersection point, or None.
fn ray_aabb_intersection(ray: &Ray3d, aabb: &super::spatial::WorldAabb) -> Option<f32> {
    let inv_dir = Vec3::new(
        1.0 / ray.direction.x,
        1.0 / ray.direction.y,
        1.0 / ray.direction.z,
    );

    let t1 = (aabb.min.x - ray.origin.x) * inv_dir.x;
    let t2 = (aabb.max.x - ray.origin.x) * inv_dir.x;
    let t3 = (aabb.min.y - ray.origin.y) * inv_dir.y;
    let t4 = (aabb.max.y - ray.origin.y) * inv_dir.y;
    let t5 = (aabb.min.z - ray.origin.z) * inv_dir.z;
    let t6 = (aabb.max.z - ray.origin.z) * inv_dir.z;

    let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
    let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

    if tmax < 0.0 || tmin > tmax {
        None
    } else if tmin < 0.0 {
        Some(tmax)
    } else {
        Some(tmin)
    }
}
