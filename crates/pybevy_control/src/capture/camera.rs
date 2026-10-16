use std::cmp::Reverse;

use bevy::{
    camera::RenderTarget,
    ecs::world::World,
    prelude::*,
    window::{PrimaryWindow, Window, WindowRef},
};
use pybevy_core::public_error::{
    SCREENSHOT_CAMERA3D_REQUIRED, TURNAROUND_CAMERA3D_REQUIRED, capture_inactive_cameras,
    capture_no_camera, capture_no_target,
};

use crate::{
    capture::{
        depth::CaptureViewport,
        queue::{CaptureResponseKind, DebugCameraRequest},
    },
    protocol::ControlError,
};

/// Cleanup info for a debug camera set up for a screenshot.
pub struct DebugCameraCleanup {
    pub debug_entity: Entity,
    /// If we reused an existing camera: (original_transform, original_global_transform, original_is_active).
    /// If None, this was a freshly spawned camera that should be despawned on cleanup.
    pub reused_state: Option<(Transform, GlobalTransform, bool)>,
    /// Other cameras and their original `is_active` state before we disabled them.
    pub original_cameras: Vec<(Entity, bool)>,
}

/// The pose a capture holds on its camera until the capture releases it.
///
/// Camera controllers in `Update` rewrite the camera's `Transform` every frame,
/// so the pose is reapplied in `Last`, after them and before render extraction.
#[derive(Resource)]
pub(crate) struct CapturePoseHold {
    entity: Entity,
    transform: Transform,
}

pub fn hold_capture_pose(world: &mut World) {
    let Some(&CapturePoseHold { entity, transform }) = world.get_resource::<CapturePoseHold>()
    else {
        return;
    };
    if let Some(mut current) = world.get_mut::<Transform>(entity)
        && *current != transform
    {
        *current = transform;
    }
    let global = GlobalTransform::from(transform);
    if let Some(mut current) = world.get_mut::<GlobalTransform>(entity)
        && *current != global
    {
        *current = global;
    }
}

pub(super) fn validate_capture_camera(
    world: &mut World,
    response_kind: &CaptureResponseKind,
) -> Result<(), ControlError> {
    let operation = response_kind.operation_name();
    let primary_window = world
        .query_filtered::<Entity, (With<PrimaryWindow>, With<Window>)>()
        .iter(world)
        .next();
    let cameras = world
        .query::<(&Camera, Option<&RenderTarget>)>()
        .iter(world)
        .map(|(camera, target)| (camera.is_active, target.cloned()))
        .collect::<Vec<_>>();

    if cameras.is_empty() {
        return Err(ControlError::invalid_params(capture_no_camera(operation)));
    }

    let active = cameras
        .iter()
        .filter(|(is_active, _)| *is_active)
        .collect::<Vec<_>>();
    if active.is_empty() {
        return Err(ControlError::invalid_params(capture_inactive_cameras(
            operation,
        )));
    }

    let has_capturable_target = active.iter().any(|(_, target)| match target {
        Some(RenderTarget::Window(WindowRef::Primary)) => primary_window.is_some(),
        Some(RenderTarget::Window(WindowRef::Entity(entity))) => primary_window == Some(*entity),
        Some(RenderTarget::Image(_)) => true,
        Some(RenderTarget::None { .. } | RenderTarget::TextureView(_)) | None => false,
    });
    if !has_capturable_target {
        return Err(ControlError::invalid_params(capture_no_target(operation)));
    }

    Ok(())
}

pub(super) fn selected_camera_screenshot_target(
    world: &mut World,
    operation: &str,
) -> Result<Option<(RenderTarget, Option<CaptureViewport>)>, ControlError> {
    let selected = select_capture_camera_3d(world)
        .map(|entity| (entity, "Camera3d"))
        .or_else(|| select_capture_camera_2d(world).map(|entity| (entity, "Camera2d")));
    let Some((entity, camera_kind)) = selected else {
        return Ok(None);
    };
    let camera = world
        .get::<Camera>(entity)
        .expect("selected capture camera has Camera");
    let viewport = camera.viewport.as_ref().map(|viewport| CaptureViewport {
        physical_position: viewport.physical_position,
        physical_size: viewport.physical_size,
    });
    let target = world.get::<RenderTarget>(entity).cloned().ok_or_else(|| {
        ControlError::invalid_params(format!(
            "{operation} selected highest-order {camera_kind} has no RenderTarget"
        ))
    })?;
    let capturable = match &target {
        RenderTarget::Image(_) => true,
        RenderTarget::Window(WindowRef::Primary) => world
            .query_filtered::<Entity, (With<PrimaryWindow>, With<Window>)>()
            .iter(world)
            .next()
            .is_some(),
        RenderTarget::Window(WindowRef::Entity(window)) => world.get::<Window>(*window).is_some(),
        RenderTarget::None { .. } | RenderTarget::TextureView(_) => false,
    };
    if !capturable {
        return Err(ControlError::invalid_params(format!(
            "{operation} selected highest-order {camera_kind} has no capturable target"
        )));
    }
    Ok(Some((target, viewport)))
}

/// Set up a debug camera for a screenshot.
///
/// Reuses an existing Camera3d when possible to avoid a crash in
/// `prepare_lights` - a newly spawned Camera3d won't have cascade shadow
/// data populated by `build_directional_light_cascades` (runs in PostUpdate,
/// before Last where we spawn), causing an `unwrap()` panic on the cascade
/// lookup.  Reusing an existing camera preserves the cascade data.
///
/// Falls back to spawning a new camera only if no Camera3d exists.
pub(crate) fn setup_debug_camera(
    world: &mut World,
    req: &DebugCameraRequest,
) -> Result<DebugCameraCleanup, ControlError> {
    setup_debug_camera_with_up_for_entity(world, req, Vec3::Y, None, SCREENSHOT_CAMERA3D_REQUIRED)
}

pub(crate) fn setup_debug_camera_with_up(
    world: &mut World,
    req: &DebugCameraRequest,
    up: Vec3,
) -> Result<DebugCameraCleanup, ControlError> {
    setup_debug_camera_with_up_for_entity(world, req, up, None, TURNAROUND_CAMERA3D_REQUIRED)
}

pub(super) fn setup_debug_camera_for_entity(
    world: &mut World,
    req: &DebugCameraRequest,
    preferred_entity: Option<Entity>,
) -> Result<DebugCameraCleanup, ControlError> {
    setup_debug_camera_with_up_for_entity(
        world,
        req,
        Vec3::Y,
        preferred_entity,
        SCREENSHOT_CAMERA3D_REQUIRED,
    )
}

fn setup_debug_camera_with_up_for_entity(
    world: &mut World,
    req: &DebugCameraRequest,
    up: Vec3,
    preferred_entity: Option<Entity>,
    camera2d_only_error: &'static str,
) -> Result<DebugCameraCleanup, ControlError> {
    let position = Vec3::from_array(req.position);
    let look_at = Vec3::from_array(req.look_at);
    let target_transform = Transform::from_translation(position).looking_at(look_at, up);

    // Collect all cameras and their active state
    let all_cameras: Vec<(Entity, bool)> = {
        let mut query = world.query::<(Entity, &Camera)>();
        query.iter(world).map(|(e, c)| (e, c.is_active)).collect()
    };

    // Find an existing Camera3d to reuse (prefer an active one)
    let reuse_entity = preferred_entity
        .or_else(|| select_capture_camera_3d(world))
        .or_else(|| select_any_camera_3d(world));

    if let Some(reuse) = reuse_entity {
        let original_transform = world.get::<Transform>(reuse).copied().unwrap_or_default();
        let original_global_transform = world
            .get::<GlobalTransform>(reuse)
            .copied()
            .unwrap_or_default();
        let original_active = all_cameras
            .iter()
            .find(|(e, _)| *e == reuse)
            .map(|(_, a)| *a)
            .unwrap_or(true);

        // Move camera to debug position and ensure it is active.
        // Set both Transform and GlobalTransform so that view uniforms
        // (view.world_position) are correct even before propagate_transforms
        // runs in the next PostUpdate - this function runs in Last.
        if let Some(mut t) = world.get_mut::<Transform>(reuse) {
            *t = target_transform;
        }
        if let Some(mut gt) = world.get_mut::<GlobalTransform>(reuse) {
            *gt = GlobalTransform::from(target_transform);
        }
        if let Some(mut cam) = world.get_mut::<Camera>(reuse) {
            cam.is_active = true;
        }

        // Disable all other cameras
        let other_cameras: Vec<(Entity, bool)> = all_cameras
            .into_iter()
            .filter(|(e, _)| *e != reuse)
            .collect();
        for (entity, _) in &other_cameras {
            if let Some(mut cam) = world.get_mut::<Camera>(*entity) {
                cam.is_active = false;
            }
        }

        world.insert_resource(CapturePoseHold {
            entity: reuse,
            transform: target_transform,
        });

        Ok(DebugCameraCleanup {
            debug_entity: reuse,
            reused_state: Some((
                original_transform,
                original_global_transform,
                original_active,
            )),
            original_cameras: other_cameras,
        })
    } else {
        let has_camera2d = world
            .query_filtered::<Entity, With<Camera2d>>()
            .iter(world)
            .next()
            .is_some();
        if has_camera2d {
            return Err(ControlError::invalid_params(camera2d_only_error));
        }

        // No Camera3d exists - spawn a new one.
        // Cascade crash is unlikely here since the scene probably has no directional lights.
        let original_cameras = all_cameras.clone();
        for (entity, _) in &all_cameras {
            if let Some(mut cam) = world.get_mut::<Camera>(*entity) {
                cam.is_active = false;
            }
        }

        let debug_entity = world
            .spawn((
                Camera3d::default(),
                target_transform,
                GlobalTransform::from(target_transform),
            ))
            .id();
        world.insert_resource(CapturePoseHold {
            entity: debug_entity,
            transform: target_transform,
        });

        Ok(DebugCameraCleanup {
            debug_entity,
            reused_state: None,
            original_cameras,
        })
    }
}

pub(crate) fn select_capture_camera_3d(world: &mut World) -> Option<Entity> {
    let mut query = world.query_filtered::<(Entity, &Camera), With<Camera3d>>();
    query
        .iter(world)
        .filter(|(_, camera)| camera.is_active)
        .min_by_key(|(entity, camera)| (Reverse(camera.order), entity.to_bits()))
        .map(|(entity, _)| entity)
}

fn select_capture_camera_2d(world: &mut World) -> Option<Entity> {
    let mut query = world.query_filtered::<(Entity, &Camera), With<Camera2d>>();
    query
        .iter(world)
        .filter(|(_, camera)| camera.is_active)
        .min_by_key(|(entity, camera)| (Reverse(camera.order), entity.to_bits()))
        .map(|(entity, _)| entity)
}

fn select_any_camera_3d(world: &mut World) -> Option<Entity> {
    let mut query = world.query_filtered::<(Entity, &Camera), With<Camera3d>>();
    query
        .iter(world)
        .min_by_key(|(entity, camera)| (Reverse(camera.order), entity.to_bits()))
        .map(|(entity, _)| entity)
}

pub(crate) fn cleanup_debug_camera_world(cleanup: DebugCameraCleanup, world: &mut World) {
    if world
        .get_resource::<CapturePoseHold>()
        .is_some_and(|hold| hold.entity == cleanup.debug_entity)
    {
        world.remove_resource::<CapturePoseHold>();
    }
    if let Some((original_transform, original_global_transform, was_active)) = cleanup.reused_state
    {
        if let Some(mut transform) = world.get_mut::<Transform>(cleanup.debug_entity) {
            *transform = original_transform;
        }
        if let Some(mut global_transform) = world.get_mut::<GlobalTransform>(cleanup.debug_entity) {
            *global_transform = original_global_transform;
        }
        if let Some(mut camera) = world.get_mut::<Camera>(cleanup.debug_entity) {
            camera.is_active = was_active;
        }
    } else if world.get_entity(cleanup.debug_entity).is_ok() {
        world.despawn(cleanup.debug_entity);
    }

    for (entity, was_active) in cleanup.original_cameras {
        if let Some(mut camera) = world.get_mut::<Camera>(entity) {
            camera.is_active = was_active;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        camera::{RenderTarget, Viewport},
        image::Image,
        window::WindowRef,
    };
    use tokio::sync::oneshot;

    use super::*;
    use crate::capture::{
        analysis::FrameStatsOptions,
        hold::CaptureHold,
        queue::{CaptureResponseKind, DebugCameraRequest, PendingScreenshots},
        screenshot::{StagedScreenshot, StagedScreenshots, process_pending_screenshots},
        test_support::{
            make_test_image, spawn_structurally_valid_headless_camera, world_with_gizmos,
        },
    };

    #[test]
    fn capture_camera_validation_distinguishes_missing_inactive_and_target_errors() {
        let mut no_camera = World::new();
        let error =
            validate_capture_camera(&mut no_camera, &CaptureResponseKind::Screenshot).unwrap_err();
        assert!(error.message.starts_with("capture_screenshot requires"));
        assert!(error.message.contains("no Camera entities"));

        let mut inactive = World::new();
        inactive.spawn((
            Camera {
                is_active: false,
                ..default()
            },
            RenderTarget::None {
                size: UVec2::new(1, 1),
            },
        ));
        let error =
            validate_capture_camera(&mut inactive, &CaptureResponseKind::Screenshot).unwrap_err();
        assert!(error.message.starts_with("capture_screenshot requires"));
        assert!(error.message.contains("all Camera entities are inactive"));

        let mut missing_target = World::new();
        missing_target.spawn(Camera::default());
        let error = validate_capture_camera(&mut missing_target, &CaptureResponseKind::Screenshot)
            .unwrap_err();
        assert!(error.message.contains("capturable RenderTarget"));
        assert!(!error.message.contains("renderer"));

        let mut none_target = World::new();
        none_target.spawn((
            Camera::default(),
            RenderTarget::None {
                size: UVec2::new(1, 1),
            },
        ));
        let error = validate_capture_camera(&mut none_target, &CaptureResponseKind::Screenshot)
            .unwrap_err();
        assert!(error.message.contains("RenderTarget.None"));
    }

    #[test]
    fn capture_camera_validation_accepts_unmaterialized_image_target() {
        let mut world = World::new();
        spawn_structurally_valid_headless_camera(&mut world);

        validate_capture_camera(&mut world, &CaptureResponseKind::Screenshot).unwrap();
        assert!(world.get_resource::<Assets<Image>>().is_none());
    }

    #[test]
    fn capture_camera_validation_rejects_unresolved_primary_window_target() {
        let mut world = World::new();
        world.spawn((Camera::default(), RenderTarget::Window(WindowRef::Primary)));

        let error = validate_capture_camera(
            &mut world,
            &CaptureResponseKind::Stats(FrameStatsOptions {
                grid: 1,
                region: None,
                sample_points: None,
            }),
        )
        .unwrap_err();
        assert!(error.message.starts_with("capture_stats requires"));
        assert!(error.message.contains("unavailable window target"));
    }

    #[test]
    fn setup_debug_camera_reuses_existing_camera3d() {
        let mut world = World::new();
        let original_transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let cam_entity = world.spawn((Camera3d::default(), original_transform)).id();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        // Should reuse the existing entity, not spawn a new one
        assert_eq!(cleanup.debug_entity, cam_entity);
        assert!(cleanup.reused_state.is_some());

        let (saved_transform, _saved_global_transform, saved_active) =
            cleanup.reused_state.unwrap();
        assert_eq!(saved_transform.translation, original_transform.translation);
        assert!(saved_active); // Camera3d defaults to active

        // Camera should be moved to the debug position
        let current = world.get::<Transform>(cam_entity).unwrap();
        assert!(
            (current.translation.x - 10.0).abs() < 0.01,
            "Camera should be at debug position"
        );
    }

    #[test]
    fn capture_camera_selection_uses_highest_active_order_then_lowest_entity() {
        let mut world = World::new();
        let low = world
            .spawn((Camera3d::default(), Camera::default(), Name::new("low")))
            .id();
        let first_high = world
            .spawn((
                Camera3d::default(),
                Camera {
                    order: 5,
                    ..default()
                },
                Name::new("first-high"),
            ))
            .id();
        let second_high = world
            .spawn((
                Camera3d::default(),
                Camera {
                    order: 5,
                    ..default()
                },
                Name::new("second-high"),
            ))
            .id();
        world.spawn((
            Camera3d::default(),
            Camera {
                order: 100,
                is_active: false,
                ..default()
            },
            Name::new("inactive"),
        ));

        assert_ne!(low, first_high);
        let expected = [first_high, second_high]
            .into_iter()
            .min_by_key(|entity| entity.to_bits());
        assert_eq!(select_capture_camera_3d(&mut world), expected);
    }

    #[test]
    fn screenshot_target_selects_highest_order_active_2d_camera() {
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let lower_target = images.add(make_test_image(64, 48));
        let lower_id = lower_target.id();
        let higher_target = images.add(make_test_image(128, 96));
        world.insert_resource(images);
        world.spawn((
            Camera2d,
            Camera {
                order: -1,
                ..default()
            },
            RenderTarget::Image(lower_target.into()),
        ));
        let higher = world
            .spawn((
                Camera2d,
                Camera {
                    order: 5,
                    ..default()
                },
                RenderTarget::Image(higher_target.clone().into()),
            ))
            .id();

        let (target, _) = selected_camera_screenshot_target(&mut world, "capture_screenshot")
            .unwrap()
            .unwrap();
        let RenderTarget::Image(image_target) = target else {
            panic!("selected Camera2d should use its image target");
        };
        assert_eq!(image_target.handle.id(), higher_target.id());

        world.get_mut::<Camera>(higher).unwrap().is_active = false;
        let (target, _) = selected_camera_screenshot_target(&mut world, "capture_screenshot")
            .unwrap()
            .unwrap();
        let RenderTarget::Image(image_target) = target else {
            panic!("active Camera2d should use its image target");
        };
        assert_eq!(image_target.handle.id(), lower_id);
    }

    #[test]
    fn screenshot_target_uses_selected_camera_image_and_viewport() {
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let lower = images.add(make_test_image(64, 48));
        let selected = images.add(make_test_image(128, 96));
        world.insert_resource(images);
        world.spawn((
            Camera3d::default(),
            Camera {
                order: -1,
                ..default()
            },
            RenderTarget::Image(lower.into()),
        ));
        world.spawn((
            Camera3d::default(),
            Camera {
                order: 5,
                viewport: Some(Viewport {
                    physical_position: UVec2::new(16, 16),
                    physical_size: UVec2::new(96, 64),
                    ..default()
                }),
                ..default()
            },
            RenderTarget::Image(selected.clone().into()),
        ));

        let (target, viewport) =
            selected_camera_screenshot_target(&mut world, "capture_screenshot")
                .unwrap()
                .unwrap();
        let RenderTarget::Image(image_target) = target else {
            panic!("selected camera should use its image target");
        };
        assert_eq!(image_target.handle.id(), selected.id());
        assert_eq!(viewport.unwrap().physical_size, UVec2::new(96, 64));
    }

    #[test]
    fn screenshot_target_rejects_uncapturable_selected_camera_without_fallback() {
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let lower = images.add(make_test_image(64, 48));
        world.insert_resource(images);
        world.spawn((Camera3d::default(), RenderTarget::Image(lower.into())));
        world.spawn((
            Camera3d::default(),
            Camera {
                order: 5,
                ..default()
            },
            RenderTarget::None {
                size: UVec2::new(64, 48),
            },
        ));

        let error =
            selected_camera_screenshot_target(&mut world, "capture_screenshot").unwrap_err();
        assert_eq!(
            error.message,
            "capture_screenshot selected highest-order Camera3d has no capturable target"
        );
    }

    #[test]
    fn explicit_depth_pose_reuses_the_preserved_camera_entity() {
        let mut world = World::new();
        let first = world.spawn((Camera3d::default(), Camera::default())).id();
        let selected = world
            .spawn((
                Camera3d::default(),
                Camera {
                    order: 5,
                    ..default()
                },
            ))
            .id();
        let request = DebugCameraRequest {
            position: [8.0, 3.0, 1.0],
            look_at: [0.0, 0.0, 0.0],
        };

        let cleanup = setup_debug_camera_for_entity(&mut world, &request, Some(selected)).unwrap();

        assert_eq!(cleanup.debug_entity, selected);
        assert!(!world.get::<Camera>(first).unwrap().is_active);
        assert_eq!(
            world.get::<Transform>(selected).unwrap().translation,
            Vec3::new(8.0, 3.0, 1.0)
        );
        cleanup_debug_camera_world(cleanup, &mut world);
        assert!(world.get::<Camera>(first).unwrap().is_active);
        assert!(world.get::<Camera>(selected).unwrap().is_active);
    }

    #[test]
    fn setup_debug_camera_supports_a_vertical_top_view() {
        let mut world = World::new();
        let camera = world.spawn(Camera3d::default()).id();
        let request = DebugCameraRequest {
            position: [0.0, 10.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };

        setup_debug_camera_with_up(&mut world, &request, Vec3::Z).unwrap();

        let transform = world.get::<Transform>(camera).unwrap();
        assert!(transform.rotation.is_finite());
        assert!(transform.forward().as_vec3().dot(Vec3::NEG_Y) > 0.999);
    }

    #[test]
    fn setup_debug_camera_disables_other_cameras() {
        let mut world = World::new();
        let cam_a = world
            .spawn((Camera3d::default(), Transform::default()))
            .id();
        let cam_b = world
            .spawn((Camera3d::default(), Transform::default()))
            .id();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        // One camera is reused (active), the other is disabled
        let reused = cleanup.debug_entity;
        let other = if reused == cam_a { cam_b } else { cam_a };

        assert!(world.get::<Camera>(reused).unwrap().is_active);
        assert!(!world.get::<Camera>(other).unwrap().is_active);

        // original_cameras should contain only the non-reused camera
        assert_eq!(cleanup.original_cameras.len(), 1);
        assert_eq!(cleanup.original_cameras[0].0, other);
    }

    #[test]
    fn setup_debug_camera_spawns_when_no_camera3d() {
        let mut world = World::new();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        // Should have spawned a new camera (reused_state is None)
        assert!(cleanup.reused_state.is_none());
        assert!(world.get::<Camera3d>(cleanup.debug_entity).is_some());
    }

    #[test]
    fn setup_debug_camera_rejects_camera2d_only_scene() {
        let mut world = World::new();
        let camera = world.spawn(Camera2d).id();
        let req = DebugCameraRequest {
            position: [0.0, 0.0, 500.0],
            look_at: [0.0, 0.0, 0.0],
        };

        let error = setup_debug_camera(&mut world, &req).err().unwrap();
        assert!(error.message.contains("require a Camera3d"));
        assert!(world.get::<Camera>(camera).unwrap().is_active);
        assert_eq!(
            world
                .query_filtered::<Entity, With<Camera3d>>()
                .iter(&world)
                .count(),
            0
        );
    }

    #[test]
    fn cleanup_debug_camera_world_despawns_temporary_camera() {
        let mut world = World::new();
        let cleanup = setup_debug_camera(
            &mut world,
            &DebugCameraRequest {
                position: [10.0, 5.0, 0.0],
                look_at: [0.0, 0.0, 0.0],
            },
        )
        .unwrap();
        let debug_entity = cleanup.debug_entity;

        cleanup_debug_camera_world(cleanup, &mut world);

        assert!(world.get_entity(debug_entity).is_err());
    }

    #[test]
    fn setup_debug_camera_prefers_active_camera() {
        let mut world = World::new();
        let inactive = world
            .spawn((Camera3d::default(), Transform::default()))
            .id();
        // Deactivate the first camera
        world.get_mut::<Camera>(inactive).unwrap().is_active = false;

        let active = world
            .spawn((Camera3d::default(), Transform::default()))
            .id();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        // Should prefer the active camera
        assert_eq!(cleanup.debug_entity, active);
    }

    #[test]
    fn setup_debug_camera_reuses_deterministic_inactive_camera() {
        let mut world = World::new();
        world.spawn((
            Camera3d::default(),
            Camera {
                order: 1,
                is_active: false,
                ..default()
            },
            Transform::default(),
        ));
        let preferred = world
            .spawn((
                Camera3d::default(),
                Camera {
                    order: 5,
                    is_active: false,
                    ..default()
                },
                Transform::default(),
            ))
            .id();
        let camera_count = world
            .query_filtered::<Entity, With<Camera3d>>()
            .iter(&world)
            .count();

        let cleanup = setup_debug_camera(
            &mut world,
            &DebugCameraRequest {
                position: [10.0, 5.0, 0.0],
                look_at: [0.0, 0.0, 0.0],
            },
        )
        .unwrap();

        assert_eq!(cleanup.debug_entity, preferred);
        assert!(cleanup.reused_state.is_some());
        assert_eq!(
            world
                .query_filtered::<Entity, With<Camera3d>>()
                .iter(&world)
                .count(),
            camera_count
        );
        cleanup_debug_camera_world(cleanup, &mut world);
        assert!(!world.get::<Camera>(preferred).unwrap().is_active);
    }

    #[test]
    fn setup_debug_camera_cleanup_restores_reused_camera() {
        let mut world = World::new();
        let original_transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let cam = world.spawn((Camera3d::default(), original_transform)).id();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        cleanup_debug_camera_world(cleanup, &mut world);

        // Camera should be restored to original position
        let restored = world.get::<Transform>(cam).unwrap();
        assert_eq!(restored.translation, original_transform.translation);
        assert!(world.get::<Camera>(cam).unwrap().is_active);
    }

    #[test]
    fn headless_debug_screenshot_restores_reused_camera() {
        let mut world = world_with_gizmos();
        let original_transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let camera = world
            .spawn((
                Camera3d::default(),
                original_transform,
                GlobalTransform::from(original_transform),
            ))
            .id();
        let cleanup = setup_debug_camera(
            &mut world,
            &DebugCameraRequest {
                position: [10.0, 5.0, 0.0],
                look_at: [0.0, 0.0, 0.0],
            },
        )
        .unwrap();
        let (response_tx, _response_rx) = oneshot::channel();
        world.insert_resource(StagedScreenshots {
            pending: vec![StagedScreenshot {
                response_tx,
                frames_remaining: 0,
                with_gizmos: false,
                hold: CaptureHold {
                    leased: true,
                    debug_cleanup: Some(cleanup),
                    ..Default::default()
                },
                max_width: None,
                depth_analysis: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
                baseline_headless_sequence: None,
                frames_waited: 0,
            }],
        });
        world.insert_resource(PendingScreenshots::default());

        process_pending_screenshots(&mut world);

        assert_eq!(
            world.get::<Transform>(camera).unwrap().translation,
            original_transform.translation
        );
        assert!(world.get::<Camera>(camera).unwrap().is_active);
    }

    #[test]
    fn setup_debug_camera_sets_global_transform() {
        let mut world = World::new();
        let original_transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let original_global = GlobalTransform::from(original_transform);
        let cam = world
            .spawn((Camera3d::default(), original_transform, original_global))
            .id();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        // GlobalTransform should be updated to match the debug position
        let gt = world.get::<GlobalTransform>(cam).unwrap();
        let debug_pos = gt.translation();
        assert!(
            (debug_pos.x - 10.0).abs() < 0.01,
            "GlobalTransform should be at debug position, got {debug_pos:?}"
        );

        // Saved state should contain the original GlobalTransform
        let (_saved_t, saved_gt, _saved_active) = cleanup.reused_state.unwrap();
        assert_eq!(
            saved_gt.translation(),
            original_global.translation(),
            "Should save original GlobalTransform for cleanup"
        );
    }

    #[test]
    fn setup_debug_camera_spawns_with_global_transform() {
        let mut world = World::new();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        // Spawned camera should have GlobalTransform set
        let gt = world.get::<GlobalTransform>(cleanup.debug_entity);
        assert!(
            gt.is_some(),
            "Spawned debug camera should have GlobalTransform"
        );
        let debug_pos = gt.unwrap().translation();
        assert!(
            (debug_pos.x - 10.0).abs() < 0.01,
            "Spawned GlobalTransform should match debug position, got {debug_pos:?}"
        );
    }
}
