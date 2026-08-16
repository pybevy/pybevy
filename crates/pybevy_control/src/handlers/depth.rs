use bevy::{
    ecs::{entity::Entity, name::Name, world::World},
    math::{Ray3d, Vec3},
    prelude::*,
};

use super::spatial::compute_world_aabb;
use crate::bridge::ControlError;

const EXPLICIT_SAMPLE_EXTENT: i64 = 800;

fn camera_basis(position: Vec3, target: Vec3) -> Result<(Vec3, Vec3, Vec3), ControlError> {
    let forward = (target - position).normalize_or_zero();
    if forward == Vec3::ZERO {
        return Err(ControlError::invalid_params(
            "position and look_at must be distinct finite points",
        ));
    }
    let reference_up = if forward.dot(Vec3::Y).abs() > 0.999 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let right = forward.cross(reference_up).normalize();
    let up = right.cross(forward).normalize();
    Ok((forward, right, up))
}

fn validate_sample_points(sample_points: &[[i64; 2]]) -> Result<Vec<[u32; 2]>, ControlError> {
    sample_points
        .iter()
        .enumerate()
        .map(|(index, &[x, y])| {
            if !(0..EXPLICIT_SAMPLE_EXTENT).contains(&x)
                || !(0..EXPLICIT_SAMPLE_EXTENT).contains(&y)
            {
                return Err(ControlError::invalid_params(format!(
                    "sample_points[{index}] must be within [0, 800) on both axes (got [{x}, {y}])"
                )));
            }

            Ok([x as u32, y as u32])
        })
        .collect()
}

/// Compute depth samples by casting rays from a camera position through sample points
/// against all entity world AABBs.
///
/// If position/look_at are None, tries to use the active scene camera.
/// If sample_points is None, generates an NxN grid based on grid_density.
pub fn compute_depth_samples(
    world: &mut World,
    position: &Option<[f32; 3]>,
    look_at: &Option<[f32; 3]>,
    sample_points: &Option<Vec<[i64; 2]>>,
    grid_density: &Option<u32>,
) -> Result<serde_json::Value, ControlError> {
    if matches!(grid_density, Some(0)) {
        return Err(ControlError::invalid_params(
            "grid_density must be at least 1",
        ));
    }

    let explicit_points = sample_points
        .as_deref()
        .map(validate_sample_points)
        .transpose()?;

    // Determine camera position and orientation
    let (cam_pos, cam_forward, cam_right, cam_up) = if let Some(pos) = position {
        let p = Vec3::from_array(*pos);
        let target = Vec3::from_array(look_at.unwrap_or([0.0, 0.0, 0.0]));
        let (forward, right, up) = camera_basis(p, target)?;
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
    let explicit_sampling = explicit_points.is_some();
    let points: Vec<[u32; 2]> = if let Some(pts) = explicit_points {
        pts
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
    let occurrences = super::spatial::NameOccurrences::collect(world);
    let mut samples = Vec::new();
    let divisor = if explicit_sampling {
        EXPLICIT_SAMPLE_EXTENT as f32
    } else {
        density as f32
    };
    let cell_offset = if explicit_sampling { 0.0 } else { 0.5 };
    for point in &points {
        // Convert to normalized coordinates (-1 to 1)
        let nx = ((point[0] as f32 + cell_offset) / divisor) * 2.0 - 1.0;
        let ny = -(((point[1] as f32 + cell_offset) / divisor) * 2.0 - 1.0); // flip Y

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

        let mut sample = if let Some((entity, distance)) = nearest_hit {
            let hit_pos = cam_pos + dir * distance;
            let name = world.get::<Name>(entity).map(|n| n.as_str().to_string());
            let label = super::spatial::entity_label_with(world, entity, &occurrences);
            serde_json::json!({
                "hit": true,
                "distance": distance,
                "world_position": [hit_pos.x, hit_pos.y, hit_pos.z],
                "entity_id": entity.to_bits(),
                "entity_name": name,
                "entity_label": label,
            })
        } else {
            serde_json::json!({
                "hit": false,
                "distance": null,
            })
        };
        let coordinate_key = if explicit_sampling { "pixel" } else { "grid" };
        sample[coordinate_key] = serde_json::json!([point[0], point[1]]);

        samples.push(sample);
    }

    let hit_count = samples.iter().filter(|s| s["hit"] == true).count();

    Ok(serde_json::json!({
        "sample_count": samples.len(),
        "hit_count": hit_count,
        "camera_position": [cam_pos.x, cam_pos.y, cam_pos.z],
        "coordinate_space": if explicit_sampling { "pixels_800x800" } else { "grid_indices" },
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

#[cfg(test)]
mod tests {
    use bevy::{camera::primitives::Aabb, ecs::entity::Entity, math::Vec3A};

    use super::{super::spatial::WorldAabb, *};

    fn make_aabb(min: [f32; 3], max: [f32; 3]) -> WorldAabb {
        WorldAabb {
            min: Vec3A::from_array(min),
            max: Vec3A::from_array(max),
            entity: Entity::from_bits(1),
        }
    }

    #[test]
    fn ray_aabb_hit_from_outside() {
        let aabb = make_aabb([1.0, 1.0, 1.0], [3.0, 3.0, 3.0]);
        let ray = Ray3d::new(Vec3::new(0.0, 2.0, 2.0), Dir3::X);
        let t = ray_aabb_intersection(&ray, &aabb);
        assert!(t.is_some());
        let t = t.unwrap();
        // Should hit at x=1.0, so t=1.0
        assert!((t - 1.0).abs() < 1e-4, "t={t}");
    }

    #[test]
    fn ray_aabb_miss_parallel() {
        let aabb = make_aabb([1.0, 1.0, 1.0], [3.0, 3.0, 3.0]);
        // Ray going in +X but above the box
        let ray = Ray3d::new(Vec3::new(0.0, 5.0, 2.0), Dir3::X);
        assert!(ray_aabb_intersection(&ray, &aabb).is_none());
    }

    #[test]
    fn ray_aabb_inside_returns_tmax() {
        let aabb = make_aabb([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        let ray = Ray3d::new(Vec3::new(5.0, 5.0, 5.0), Dir3::X);
        let t = ray_aabb_intersection(&ray, &aabb);
        assert!(t.is_some());
        // Inside the box, tmin < 0, so it returns tmax
        let t = t.unwrap();
        // Should exit at x=10, so tmax = 5.0
        assert!((t - 5.0).abs() < 1e-4, "t={t}");
    }

    #[test]
    fn ray_aabb_behind_returns_none() {
        let aabb = make_aabb([1.0, 1.0, 1.0], [3.0, 3.0, 3.0]);
        // Ray starts past the box and goes further away
        let ray = Ray3d::new(Vec3::new(5.0, 2.0, 2.0), Dir3::X);
        assert!(ray_aabb_intersection(&ray, &aabb).is_none());
    }

    #[test]
    fn ray_aabb_grazing_slightly_inside() {
        let aabb = make_aabb([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        // Ray slightly inside the top face (Y=0.999), should definitely hit
        let ray = Ray3d::new(Vec3::new(-1.0, 0.999, 0.5), Dir3::X);
        let t = ray_aabb_intersection(&ray, &aabb);
        assert!(t.is_some());
    }

    #[test]
    fn compute_depth_samples_explicit_position() {
        let mut world = World::new();
        // Use a large cube so that off-center grid rays still hit
        world.spawn((
            Aabb::from_min_max(Vec3::new(-5.0, -5.0, -5.0), Vec3::new(5.0, 5.0, 5.0)),
            GlobalTransform::default(),
            Name::new("Cube"),
        ));

        let position = Some([0.0_f32, 0.0, 10.0]);
        let look_at = Some([0.0_f32, 0.0, 0.0]);
        let sample_points = None;
        let grid_density = Some(3_u32);

        let result = compute_depth_samples(
            &mut world,
            &position,
            &look_at,
            &sample_points,
            &grid_density,
        )
        .unwrap();

        assert_eq!(result["sample_count"], 9); // 3x3 grid
        assert!(result["hit_count"].as_u64().unwrap() > 0); // rays should hit the large cube
        assert_eq!(result["coordinate_space"], "grid_indices");
        assert_eq!(result["samples"][0]["grid"], serde_json::json!([0, 0]));
        assert!(result["samples"][0].get("pixel").is_none());
        assert!(result["samples"][0].get("screen").is_none());
        let cam_pos = result["camera_position"].as_array().unwrap();
        assert!((cam_pos[0].as_f64().unwrap() - 0.0).abs() < 1e-5);
        assert!((cam_pos[1].as_f64().unwrap() - 0.0).abs() < 1e-5);
        assert!((cam_pos[2].as_f64().unwrap() - 10.0).abs() < 1e-5);
    }

    #[test]
    fn compute_depth_samples_no_entities() {
        let mut world = World::new();

        let position = Some([0.0_f32, 0.0, 10.0]);
        let look_at = Some([0.0_f32, 0.0, 0.0]);
        let sample_points = None;
        let grid_density = Some(2_u32);

        let result = compute_depth_samples(
            &mut world,
            &position,
            &look_at,
            &sample_points,
            &grid_density,
        )
        .unwrap();

        assert_eq!(result["sample_count"], 4); // 2x2 grid
        assert_eq!(result["hit_count"], 0);
    }

    #[test]
    fn odd_density_grid_samples_the_center_cell() {
        let mut world = World::new();
        world.spawn((
            Aabb::from_min_max(Vec3::splat(-0.25), Vec3::splat(0.25)),
            GlobalTransform::default(),
        ));

        let result = compute_depth_samples(
            &mut world,
            &Some([0.0, 0.0, 10.0]),
            &Some([0.0, 0.0, 0.0]),
            &None,
            &Some(3),
        )
        .unwrap();

        let center = &result["samples"][4];
        assert_eq!(center["grid"], serde_json::json!([1, 1]));
        assert_eq!(center["hit"], true);
    }

    #[test]
    fn compute_depth_samples_custom_sample_points() {
        let mut world = World::new();
        world.spawn((
            Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
            GlobalTransform::default(),
            Name::new("Cube"),
        ));

        let position = Some([0.0_f32, 0.0, 10.0]);
        let look_at = Some([0.0_f32, 0.0, 0.0]);
        let sample_points = Some(vec![[400_i64, 400]]);
        let grid_density = None;

        let result = compute_depth_samples(
            &mut world,
            &position,
            &look_at,
            &sample_points,
            &grid_density,
        )
        .unwrap();

        assert_eq!(result["sample_count"], 1);
        assert_eq!(result["coordinate_space"], "pixels_800x800");
        assert_eq!(result["samples"][0]["pixel"], serde_json::json!([400, 400]));
        assert!(result["samples"][0].get("grid").is_none());
        assert!(result["samples"][0].get("screen").is_none());
        // The center point (400,400) maps to (0,0) in normalized coords on an 800-pixel grid,
        // which shoots straight forward and should hit the cube at origin.
        assert!(result["hit_count"].as_u64().unwrap() > 0);
    }

    #[test]
    fn compute_depth_samples_rejects_out_of_range_points_before_camera_lookup() {
        let mut world = World::new();
        let sample_points = Some(vec![[99999_i64, -50]]);

        let error = compute_depth_samples(&mut world, &None, &None, &sample_points, &None)
            .expect_err("out-of-range coordinates must be rejected");

        assert_eq!(error.code, crate::bridge::ErrorCode::InvalidParams);
        assert_eq!(
            error.message,
            "sample_points[0] must be within [0, 800) on both axes (got [99999, -50])"
        );
    }

    #[test]
    fn compute_depth_samples_rejects_zero_grid_density_before_camera_lookup() {
        let mut world = World::new();

        let error = compute_depth_samples(&mut world, &None, &None, &None, &Some(0))
            .expect_err("zero grid density must be rejected");

        assert_eq!(error.code, crate::bridge::ErrorCode::InvalidParams);
        assert_eq!(error.message, "grid_density must be at least 1");
    }

    #[test]
    fn compute_depth_samples_no_camera_no_position_error() {
        let mut world = World::new();

        let position = None;
        let look_at = None;
        let sample_points = None;
        let grid_density = None;

        let result = compute_depth_samples(
            &mut world,
            &position,
            &look_at,
            &sample_points,
            &grid_density,
        );

        assert!(result.is_err());
    }

    #[test]
    fn compute_depth_samples_hit_returns_entity_info() {
        let mut world = World::new();
        world.spawn((
            Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
            GlobalTransform::default(),
            Name::new("TestCube"),
        ));

        let position = Some([0.0_f32, 0.0, 10.0]);
        let look_at = Some([0.0_f32, 0.0, 0.0]);
        let sample_points = None;
        let grid_density = Some(1_u32);

        let result = compute_depth_samples(
            &mut world,
            &position,
            &look_at,
            &sample_points,
            &grid_density,
        )
        .unwrap();

        let samples = result["samples"].as_array().unwrap();
        // With grid_density=1 we get a single sample at the center
        let sample = &samples[0];
        assert!(sample["hit"].as_bool().unwrap());
        assert_eq!(sample["entity_name"].as_str().unwrap(), "TestCube");
        assert!(sample["distance"].as_f64().unwrap() > 0.0);
        assert!(sample["world_position"].as_array().is_some());
    }

    #[test]
    fn compute_depth_samples_miss_returns_null_distance() {
        let mut world = World::new();
        // Entity far off to the side
        world.spawn((
            Aabb::from_min_max(
                Vec3::new(100.0, 100.0, 100.0),
                Vec3::new(101.0, 101.0, 101.0),
            ),
            GlobalTransform::default(),
            Name::new("FarAway"),
        ));

        let position = Some([0.0_f32, 0.0, 10.0]);
        let look_at = Some([0.0_f32, 0.0, 0.0]);
        let sample_points = None;
        let grid_density = Some(1_u32);

        let result = compute_depth_samples(
            &mut world,
            &position,
            &look_at,
            &sample_points,
            &grid_density,
        )
        .unwrap();

        let samples = result["samples"].as_array().unwrap();
        let sample = &samples[0];
        assert!(!sample["hit"].as_bool().unwrap());
        assert!(sample["distance"].is_null());
    }

    #[test]
    fn ray_aabb_diagonal_hit() {
        let aabb = make_aabb([2.0, 2.0, 2.0], [4.0, 4.0, 4.0]);
        let dir_vec = Vec3::new(1.0, 1.0, 1.0).normalize();
        let ray = Ray3d::new(Vec3::ZERO, Dir3::new(dir_vec).unwrap());
        let t = ray_aabb_intersection(&ray, &aabb);
        assert!(t.is_some());
        assert!(t.unwrap() > 0.0);
    }

    #[test]
    fn ray_aabb_negative_direction() {
        let aabb = make_aabb([-5.0, -1.0, -1.0], [-3.0, 1.0, 1.0]);
        let ray = Ray3d::new(Vec3::ZERO, Dir3::NEG_X);
        let t = ray_aabb_intersection(&ray, &aabb);
        assert!(t.is_some());
        // Should hit at x=-3.0, so t ≈ 3.0
        assert!((t.unwrap() - 3.0).abs() < 1e-4, "t={}", t.unwrap());
    }

    #[test]
    fn vertical_camera_basis_remains_orthonormal() {
        let (forward, right, up) = camera_basis(Vec3::new(0.0, 11.0, 0.0), Vec3::ZERO).unwrap();

        assert_eq!(forward, Vec3::NEG_Y);
        assert!((right.length() - 1.0).abs() < 1e-6);
        assert!((up.length() - 1.0).abs() < 1e-6);
        assert!(forward.dot(right).abs() < 1e-6);
        assert!(forward.dot(up).abs() < 1e-6);
        assert!(right.dot(up).abs() < 1e-6);
    }

    #[test]
    fn vertical_camera_produces_distinct_grid_samples() {
        let mut world = World::new();
        world.spawn((
            Aabb::from_min_max(
                Vec3::new(-100.0, -0.1, -100.0),
                Vec3::new(100.0, 0.1, 100.0),
            ),
            GlobalTransform::default(),
        ));

        let result = compute_depth_samples(
            &mut world,
            &Some([0.0, 11.0, 0.0]),
            &Some([0.0, 0.0, 0.0]),
            &None,
            &Some(3),
        )
        .unwrap();
        let samples = result["samples"].as_array().unwrap();

        assert_eq!(result["hit_count"], 9);
        assert_ne!(samples[0]["world_position"], samples[8]["world_position"]);
    }

    #[test]
    fn coincident_camera_points_are_rejected() {
        let error = camera_basis(Vec3::ONE, Vec3::ONE).unwrap_err();

        assert_eq!(error.code, crate::bridge::ErrorCode::InvalidParams);
        assert_eq!(
            error.message,
            "position and look_at must be distinct finite points"
        );
    }
}
