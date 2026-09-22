use bevy::{
    camera::{Projection, RenderTarget},
    ecs::{entity::Entity, name::Name, world::World},
    math::{Ray3d, UVec2, Vec3},
    prelude::*,
    window::PrimaryWindow,
};

use super::{screenshot::select_capture_camera_3d, spatial::compute_world_aabb};
use crate::bridge::ControlError;

const EXPLICIT_SAMPLE_EXTENT: i64 = 800;

#[derive(Debug)]
pub struct PendingDepthAnalysis {
    pub camera_entity: Entity,
    pub position: Option<[f32; 3]>,
    pub look_at: Option<[f32; 3]>,
    pub sample_points: Option<Vec<[i64; 2]>>,
    pub grid_density: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureViewport {
    pub physical_position: UVec2,
    pub physical_size: UVec2,
}

fn camera_transform(position: Vec3, target: Vec3) -> Result<GlobalTransform, ControlError> {
    let forward = (target - position).normalize_or_zero();
    if !position.is_finite() || !target.is_finite() || forward == Vec3::ZERO {
        return Err(ControlError::invalid_params(
            "position and look_at must be distinct finite points",
        ));
    }
    let reference_up = if forward.dot(Vec3::Y).abs() > 0.999 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    Ok(GlobalTransform::from(
        Transform::from_translation(position).looking_at(target, reference_up),
    ))
}

fn projection_ray(
    projection: &Projection,
    camera_transform: &GlobalTransform,
    normalized_x: f32,
    normalized_y: f32,
) -> Result<Ray3d, ControlError> {
    let clip_from_view = projection.get_clip_from_view();
    let determinant = clip_from_view.determinant();
    if !determinant.is_finite() || determinant == 0.0 {
        return Err(ControlError::invalid_params(
            "capture_depth camera projection is not invertible",
        ));
    }

    let view_from_clip = clip_from_view.inverse();
    if !view_from_clip.is_finite() {
        return Err(ControlError::invalid_params(
            "capture_depth camera projection is not invertible",
        ));
    }
    let ndc_x = normalized_x * 2.0 - 1.0;
    let ndc_y = -(normalized_y * 2.0 - 1.0);
    let view_near = view_from_clip.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
    let view_far = view_from_clip.project_point3(Vec3::new(ndc_x, ndc_y, f32::EPSILON));
    let world_near = camera_transform.transform_point(view_near);
    let world_far = camera_transform.transform_point(view_far);
    let direction = Dir3::new(world_far - world_near).map_err(|_| {
        ControlError::invalid_params("capture_depth camera projection produced an invalid ray")
    })?;
    Ok(Ray3d::new(world_near, direction))
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

pub fn validate_depth_sampling(
    sample_points: &Option<Vec<[i64; 2]>>,
    grid_density: &Option<u32>,
) -> Result<(), ControlError> {
    if matches!(grid_density, Some(0)) {
        return Err(ControlError::invalid_params(
            "grid_density must be at least 1",
        ));
    }
    sample_points
        .as_deref()
        .map(validate_sample_points)
        .transpose()?;
    Ok(())
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
    validate_depth_sampling(sample_points, grid_density)?;
    let camera_entity = select_capture_camera_3d(world)
        .ok_or_else(|| ControlError::not_found("No Camera3d found for capture_depth projection"))?;
    compute_depth_samples_for_camera(
        world,
        camera_entity,
        position,
        look_at,
        sample_points,
        grid_density,
    )
}

pub fn prepare_depth_capture(
    world: &mut World,
    request: &PendingDepthAnalysis,
) -> Result<(RenderTarget, Option<CaptureViewport>, serde_json::Value), ControlError> {
    validate_depth_capture_camera(world, request.camera_entity)?;
    let entity = world.get_entity(request.camera_entity).map_err(|_| {
        ControlError::not_found("Selected capture_depth camera was despawned before capture")
    })?;
    let camera = entity.get::<Camera>().expect("camera validated above");
    let target = entity.get::<RenderTarget>().cloned().ok_or_else(|| {
        ControlError::not_found(
            "Selected capture_depth camera lost its RenderTarget component before capture",
        )
    })?;
    if matches!(target, RenderTarget::None { .. }) {
        return Err(ControlError::invalid_params(
            "Selected capture_depth camera uses RenderTarget.None and cannot produce an RGB capture",
        ));
    }
    let viewport = camera.viewport.as_ref().map(|viewport| CaptureViewport {
        physical_position: viewport.physical_position,
        physical_size: viewport.physical_size,
    });
    let depth = compute_depth_samples_for_camera(
        world,
        request.camera_entity,
        &None,
        &None,
        &request.sample_points,
        &request.grid_density,
    )?;
    Ok((target, viewport, depth))
}

pub fn validate_depth_capture_camera(
    world: &World,
    camera_entity: Entity,
) -> Result<(), ControlError> {
    let entity = world.get_entity(camera_entity).map_err(|_| {
        ControlError::not_found("Selected capture_depth camera was despawned before capture")
    })?;
    if !entity.contains::<Camera3d>() {
        return Err(ControlError::not_found(
            "Selected capture_depth camera lost its Camera3d component before capture",
        ));
    }
    let camera = entity.get::<Camera>().ok_or_else(|| {
        ControlError::not_found(
            "Selected capture_depth camera lost its Camera component before capture",
        )
    })?;
    if !camera.is_active {
        return Err(ControlError::invalid_params(
            "Selected capture_depth camera became inactive before capture",
        ));
    }
    if !entity.contains::<Projection>() {
        return Err(ControlError::not_found(
            "Selected capture_depth camera lost its Projection component before capture",
        ));
    }
    if !entity.contains::<Transform>() {
        return Err(ControlError::not_found(
            "Selected capture_depth camera lost its Transform component before capture",
        ));
    }
    if !entity.contains::<GlobalTransform>() {
        return Err(ControlError::not_found(
            "Selected capture_depth camera lost its GlobalTransform component before capture",
        ));
    }
    if !entity.contains::<RenderTarget>() {
        return Err(ControlError::not_found(
            "Selected capture_depth camera lost its RenderTarget component before capture",
        ));
    }
    Ok(())
}

pub fn compute_depth_samples_for_camera(
    world: &mut World,
    camera_entity: Entity,
    position: &Option<[f32; 3]>,
    look_at: &Option<[f32; 3]>,
    sample_points: &Option<Vec<[i64; 2]>>,
    grid_density: &Option<u32>,
) -> Result<serde_json::Value, ControlError> {
    validate_depth_sampling(sample_points, grid_density)?;
    let explicit_points = sample_points
        .as_deref()
        .map(validate_sample_points)
        .transpose()?;
    let projection = world
        .get::<Projection>(camera_entity)
        .cloned()
        .ok_or_else(|| ControlError::not_found("Selected Camera3d has no Projection component"))?;
    let camera_transform = if let Some(pos) = position {
        camera_transform(
            Vec3::from_array(*pos),
            Vec3::from_array(look_at.unwrap_or([0.0, 0.0, 0.0])),
        )?
    } else {
        *world.get::<GlobalTransform>(camera_entity).ok_or_else(|| {
            ControlError::not_found("Selected Camera3d has no GlobalTransform component")
        })?
    };
    let cam_pos = camera_transform.translation();

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
        let normalized_x = (point[0] as f32 + cell_offset) / divisor;
        let normalized_y = (point[1] as f32 + cell_offset) / divisor;
        let ray = projection_ray(&projection, &camera_transform, normalized_x, normalized_y)?;

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
            let hit_pos = ray.get_point(distance);
            let camera_distance = cam_pos.distance(hit_pos);
            let name = world.get::<Name>(entity).map(|n| n.as_str().to_string());
            let label = super::spatial::entity_label_with(world, entity, &occurrences);
            serde_json::json!({
                "hit": true,
                "distance": camera_distance,
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

    let primary_window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next();
    let camera = world
        .get::<Camera>(camera_entity)
        .ok_or_else(|| ControlError::not_found("Selected Camera3d has no Camera component"))?;
    let target = world
        .get::<RenderTarget>(camera_entity)
        .cloned()
        .ok_or_else(|| {
            ControlError::not_found("Selected Camera3d has no RenderTarget component")
        })?;
    let normalized_target = target.normalize(primary_window);
    let camera_order = camera.order;
    let camera_name = world
        .get::<Name>(camera_entity)
        .map(|name| name.as_str().to_string());
    let viewport = camera.viewport.as_ref().map(|viewport| {
        serde_json::json!({
            "physical_position": [viewport.physical_position.x, viewport.physical_position.y],
            "physical_size": [viewport.physical_size.x, viewport.physical_size.y],
        })
    });
    let mut pass_query = world.query::<(Entity, &Camera, &RenderTarget, Option<&Name>)>();
    let mut target_camera_passes = pass_query
        .iter(world)
        .filter(|(entity, pass_camera, pass_target, _)| {
            pass_camera.is_active
                && normalized_target
                    .as_ref()
                    .map_or(*entity == camera_entity, |selected| {
                        pass_target.normalize(primary_window).as_ref() == Some(selected)
                    })
        })
        .map(|(entity, pass_camera, _, name)| {
            (
                pass_camera.order,
                entity.to_bits(),
                serde_json::json!({
                    "camera_entity_id": entity.to_bits(),
                    "camera_name": name.map(|name| name.as_str()),
                    "camera_order": pass_camera.order,
                    "is_selected": entity == camera_entity,
                }),
            )
        })
        .collect::<Vec<_>>();
    target_camera_passes.sort_by_key(|(order, entity, _)| (*order, *entity));
    let target_camera_passes = target_camera_passes
        .into_iter()
        .map(|(_, _, pass)| pass)
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "sample_count": samples.len(),
        "hit_count": hit_count,
        "camera_entity_id": camera_entity.to_bits(),
        "camera_name": camera_name,
        "camera_order": camera_order,
        "camera_position": [cam_pos.x, cam_pos.y, cam_pos.z],
        "viewport": viewport,
        "target_camera_passes": target_camera_passes,
        "coordinate_space": if explicit_sampling { "normalized_800x800" } else { "grid_indices" },
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
    use bevy::{
        asset::Assets,
        camera::{
            Camera3d, OrthographicProjection, PerspectiveProjection, Projection, primitives::Aabb,
        },
        ecs::entity::Entity,
        image::Image,
        math::Vec3A,
        transform::components::Transform,
    };

    use super::{super::spatial::WorldAabb, *};

    fn make_aabb(min: [f32; 3], max: [f32; 3]) -> WorldAabb {
        WorldAabb {
            min: Vec3A::from_array(min),
            max: Vec3A::from_array(max),
            entity: Entity::from_bits(1),
        }
    }

    fn spawn_camera(world: &mut World, fov: f32, aspect_ratio: f32) -> Entity {
        world
            .spawn((
                Camera3d::default(),
                Projection::Perspective(PerspectiveProjection {
                    fov,
                    aspect_ratio,
                    ..PerspectiveProjection::default()
                }),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 10.0)),
            ))
            .id()
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
        spawn_camera(&mut world, std::f32::consts::FRAC_PI_3, 1.0);
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
        spawn_camera(&mut world, std::f32::consts::FRAC_PI_3, 1.0);

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
    fn depth_provenance_lists_ordered_active_passes_on_the_selected_target() {
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let target = images.add(Image::default());
        world.insert_resource(images);
        world.spawn((
            Camera3d::default(),
            Camera::default(),
            Projection::Perspective(PerspectiveProjection::default()),
            GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 10.0)),
            RenderTarget::Image(target.clone().into()),
            Name::new("base"),
        ));
        let selected = world
            .spawn((
                Camera3d::default(),
                Camera {
                    order: 5,
                    ..default()
                },
                Projection::Perspective(PerspectiveProjection::default()),
                GlobalTransform::from(Transform::from_xyz(10.0, 0.0, 0.0)),
                RenderTarget::Image(target.into()),
                Name::new("selected"),
            ))
            .id();

        let result = compute_depth_samples(&mut world, &None, &None, &None, &Some(1)).unwrap();

        assert_eq!(result["camera_entity_id"], selected.to_bits());
        assert_eq!(result["camera_name"], "selected");
        assert_eq!(result["camera_order"], 5);
        assert_eq!(result["target_camera_passes"][0]["camera_name"], "base");
        assert_eq!(result["target_camera_passes"][0]["camera_order"], 0);
        assert_eq!(result["target_camera_passes"][0]["is_selected"], false);
        assert_eq!(result["target_camera_passes"][1]["camera_name"], "selected");
        assert_eq!(result["target_camera_passes"][1]["camera_order"], 5);
        assert_eq!(result["target_camera_passes"][1]["is_selected"], true);
    }

    fn spawn_complete_capture_camera(world: &mut World) -> Entity {
        world
            .spawn((
                Camera3d::default(),
                Camera::default(),
                Projection::default(),
                Transform::default(),
                GlobalTransform::default(),
                RenderTarget::default(),
            ))
            .id()
    }

    #[test]
    fn delayed_depth_camera_validation_reports_identity_breaks() {
        let cases: &[(fn(&mut World, Entity), &str)] = &[
            (
                |world, entity| {
                    world.despawn(entity);
                },
                "Selected capture_depth camera was despawned before capture",
            ),
            (
                |world, entity| {
                    world.entity_mut(entity).remove::<Camera3d>();
                },
                "Selected capture_depth camera lost its Camera3d component before capture",
            ),
            (
                |world, entity| {
                    world.entity_mut(entity).remove::<Projection>();
                },
                "Selected capture_depth camera lost its Projection component before capture",
            ),
            (
                |world, entity| {
                    world.entity_mut(entity).remove::<Transform>();
                },
                "Selected capture_depth camera lost its Transform component before capture",
            ),
            (
                |world, entity| {
                    world.entity_mut(entity).remove::<GlobalTransform>();
                },
                "Selected capture_depth camera lost its GlobalTransform component before capture",
            ),
            (
                |world, entity| {
                    world.entity_mut(entity).remove::<RenderTarget>();
                },
                "Selected capture_depth camera lost its RenderTarget component before capture",
            ),
            (
                |world, entity| {
                    world.get_mut::<Camera>(entity).unwrap().is_active = false;
                },
                "Selected capture_depth camera became inactive before capture",
            ),
        ];

        for (break_identity, expected) in cases {
            let mut world = World::new();
            let entity = spawn_complete_capture_camera(&mut world);
            break_identity(&mut world, entity);

            let error = validate_depth_capture_camera(&world, entity).unwrap_err();
            assert_eq!(&error.message, expected);
        }
    }

    #[test]
    fn odd_density_grid_samples_the_center_cell() {
        let mut world = World::new();
        spawn_camera(&mut world, std::f32::consts::FRAC_PI_3, 1.0);
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
        spawn_camera(&mut world, std::f32::consts::FRAC_PI_3, 1.0);
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
        assert_eq!(result["coordinate_space"], "normalized_800x800");
        assert_eq!(result["samples"][0]["pixel"], serde_json::json!([400, 400]));
        assert!(result["samples"][0].get("grid").is_none());
        assert!(result["samples"][0].get("screen").is_none());
        // The center point (400,400) maps to (0,0) in normalized coords on an 800-pixel grid,
        // which shoots straight forward and should hit the cube at origin.
        assert!(result["hit_count"].as_u64().unwrap() > 0);
    }

    #[test]
    fn compute_depth_samples_uses_the_cameras_field_of_view() {
        fn hit_x_with(fov: f32) -> f64 {
            let mut world = World::new();
            world.spawn((
                Aabb::from_min_max(
                    Vec3::new(-100.0, -100.0, -1.0),
                    Vec3::new(100.0, 100.0, 1.0),
                ),
                GlobalTransform::default(),
                Name::new("Cube"),
            ));
            spawn_camera(&mut world, fov, 1.0);

            let result =
                compute_depth_samples(&mut world, &None, &None, &Some(vec![[600_i64, 400]]), &None)
                    .unwrap();
            result["samples"][0]["world_position"][0].as_f64().unwrap()
        }

        let narrow = hit_x_with(0.2);
        let wide = hit_x_with(2.0);
        assert!(wide > narrow * 10.0, "narrow={narrow}, wide={wide}");
    }

    #[test]
    fn compute_depth_samples_uses_the_cameras_aspect_ratio() {
        fn hit_x_with(aspect_ratio: f32) -> f64 {
            let mut world = World::new();
            world.spawn((
                Aabb::from_min_max(
                    Vec3::new(-100.0, -100.0, -1.0),
                    Vec3::new(100.0, 100.0, 1.0),
                ),
                GlobalTransform::default(),
            ));
            spawn_camera(&mut world, std::f32::consts::FRAC_PI_2, aspect_ratio);

            let result =
                compute_depth_samples(&mut world, &None, &None, &Some(vec![[600_i64, 400]]), &None)
                    .unwrap();
            result["samples"][0]["world_position"][0].as_f64().unwrap()
        }

        let square = hit_x_with(1.0);
        let wide = hit_x_with(2.0);
        assert!(wide > square * 1.9, "square={square}, wide={wide}");
    }

    #[test]
    fn compute_depth_samples_uses_an_orthographic_projection() {
        let mut world = World::new();
        world.spawn((
            Aabb::from_min_max(
                Vec3::new(-100.0, -100.0, -1.0),
                Vec3::new(100.0, 100.0, 1.0),
            ),
            GlobalTransform::default(),
        ));
        let mut projection = OrthographicProjection::default_3d();
        projection.area = Rect::new(-4.0, -2.0, 4.0, 2.0);
        world.spawn((
            Camera3d::default(),
            Projection::Orthographic(projection),
            GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 10.0)),
        ));

        let result =
            compute_depth_samples(&mut world, &None, &None, &Some(vec![[600, 400]]), &None)
                .unwrap();

        assert_eq!(result["hit_count"], 1);
        let hit_x = result["samples"][0]["world_position"][0].as_f64().unwrap();
        assert!((hit_x - 2.0).abs() < 1e-5, "hit_x={hit_x}");
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

        let error = result.unwrap_err();
        assert_eq!(error.code, crate::bridge::ErrorCode::NotFound);
        assert_eq!(
            error.message,
            "No Camera3d found for capture_depth projection"
        );
    }

    #[test]
    fn compute_depth_samples_hit_returns_entity_info() {
        let mut world = World::new();
        spawn_camera(&mut world, std::f32::consts::FRAC_PI_3, 1.0);
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
        spawn_camera(&mut world, std::f32::consts::FRAC_PI_3, 1.0);
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
    fn vertical_camera_transform_remains_orthonormal() {
        let transform = camera_transform(Vec3::new(0.0, 11.0, 0.0), Vec3::ZERO)
            .unwrap()
            .compute_transform();
        let forward = transform.forward().as_vec3();
        let right = transform.right().as_vec3();
        let up = transform.up().as_vec3();

        assert!(forward.dot(Vec3::NEG_Y) > 0.999_999);
        assert!((right.length() - 1.0).abs() < 1e-6);
        assert!((up.length() - 1.0).abs() < 1e-6);
        assert!(forward.dot(right).abs() < 1e-6);
        assert!(forward.dot(up).abs() < 1e-6);
        assert!(right.dot(up).abs() < 1e-6);
    }

    #[test]
    fn vertical_camera_produces_distinct_grid_samples() {
        let mut world = World::new();
        spawn_camera(&mut world, std::f32::consts::FRAC_PI_3, 1.0);
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
        let error = camera_transform(Vec3::ONE, Vec3::ONE).unwrap_err();

        assert_eq!(error.code, crate::bridge::ErrorCode::InvalidParams);
        assert_eq!(
            error.message,
            "position and look_at must be distinct finite points"
        );
    }
}
