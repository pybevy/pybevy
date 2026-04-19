use std::{collections::HashMap, io::Cursor};

use base64::Engine;
use bevy::{
    ecs::world::World, math::Vec3A, prelude::*, render::view::window::screenshot::Screenshot,
};
use image::{ImageFormat, Rgb, RgbImage};
use tokio::sync::oneshot;

use super::{
    screenshot::{DebugCameraCleanup, setup_debug_camera},
    spatial::compute_world_aabb,
};
use crate::bridge::{ControlError, DebugCameraRequest};

/// A single viewpoint in a turnaround capture.
pub struct TurnaroundView {
    pub position: [f32; 3],
    pub label: String,
}

/// Resource tracking active turnaround captures.
#[derive(Resource, Default)]
pub struct PendingTurnarounds {
    pub active: Vec<ActiveTurnaround>,
}

pub struct ActiveTurnaround {
    pub response_tx: Option<oneshot::Sender<Result<serde_json::Value, ControlError>>>,
    pub viewpoints: Vec<TurnaroundView>,
    pub current_index: usize,
    pub captures: Vec<(usize, RgbImage)>,
    pub columns: u32,
    pub max_width: Option<u32>,
    pub frames_remaining: u32,
    pub hide_ui: bool,
    pub ui_restore: Option<Vec<(Entity, Visibility)>>,
    pub debug_cleanup: Option<DebugCameraCleanup>,
    pub look_at: [f32; 3],
    /// Entity of pending Screenshot component, if any
    pub pending_screenshot_entity: Option<Entity>,
}

/// Resource mapping turnaround screenshot entities to their turnaround index.
#[derive(Resource, Default)]
pub struct TurnaroundCaptures {
    pub map: HashMap<Entity, usize>,
}

/// Compute viewpoints for a turnaround capture.
/// Returns list of (position, label) pairs.
pub fn compute_viewpoints(
    look_at: [f32; 3],
    distance: f32,
    elevation_degrees: f32,
    view_count: u32,
    include_top: bool,
) -> Vec<TurnaroundView> {
    let center = Vec3::from_array(look_at);
    let elev_rad = elevation_degrees.to_radians();
    let y_offset = distance * elev_rad.sin();
    let horizontal_dist = distance * elev_rad.cos();

    let mut views = Vec::new();

    for i in 0..view_count {
        let angle = (i as f32 / view_count as f32) * std::f32::consts::TAU;
        let angle_degrees = (angle.to_degrees()).round() as i32;
        let x = center.x + horizontal_dist * angle.cos();
        let z = center.z + horizontal_dist * angle.sin();
        let y = center.y + y_offset;

        views.push(TurnaroundView {
            position: [x, y, z],
            label: format!("{angle_degrees}°"),
        });
    }

    if include_top {
        views.push(TurnaroundView {
            position: [center.x, center.y + distance, center.z + 0.001],
            label: "top".to_string(),
        });
    }

    views
}

/// Compute scene bounding box for auto-distance calculation.
pub fn compute_scene_bounds(world: &mut World) -> Option<(Vec3A, Vec3A)> {
    let mut query_state =
        world.query::<(Entity, &bevy::camera::primitives::Aabb, &GlobalTransform)>();
    let entities: Vec<Entity> = query_state.iter(world).map(|(e, _, _)| e).collect();

    if entities.is_empty() {
        return None;
    }

    let mut scene_min = Vec3A::splat(f32::MAX);
    let mut scene_max = Vec3A::splat(f32::MIN);

    for entity in &entities {
        if let Ok(aabb) = compute_world_aabb(world, *entity) {
            scene_min = scene_min.min(aabb.min);
            scene_max = scene_max.max(aabb.max);
        }
    }

    if scene_min.x < scene_max.x {
        Some((scene_min, scene_max))
    } else {
        None
    }
}

/// Hide all UI Node entities by setting their visibility to Hidden.
/// Returns a list of (entity, original_visibility) for restoration.
fn hide_ui_nodes(world: &mut World) -> Vec<(Entity, Visibility)> {
    let mut ui_entities: Vec<(Entity, Visibility)> = Vec::new();
    let mut query = world.query::<(Entity, &Visibility, &bevy::ui::Node)>();
    for (entity, vis, _) in query.iter(world) {
        ui_entities.push((entity, *vis));
    }
    for (entity, _) in &ui_entities {
        if let Some(mut vis) = world.get_mut::<Visibility>(*entity) {
            *vis = Visibility::Hidden;
        }
    }
    ui_entities
}

/// Process pending turnaround captures (called each frame in Last schedule).
pub fn process_pending_turnarounds(world: &mut World) {
    let Some(mut pending) = world.remove_resource::<PendingTurnarounds>() else {
        return;
    };

    if pending.active.is_empty() {
        world.insert_resource(pending);
        return;
    }

    let mut still_active = Vec::new();

    for mut turnaround in pending.active.drain(..) {
        if turnaround.frames_remaining > 0 {
            turnaround.frames_remaining -= 1;
            still_active.push(turnaround);
            continue;
        }

        // Check if we have a pending screenshot that was captured
        if let Some(screenshot_entity) = turnaround.pending_screenshot_entity.take() {
            let tc = world.get_resource_or_insert_with(TurnaroundCaptures::default);
            // If the capture is still pending, keep waiting
            if tc.map.contains_key(&screenshot_entity) {
                turnaround.pending_screenshot_entity = Some(screenshot_entity);
                still_active.push(turnaround);
                continue;
            }
            // Capture was collected (handled by observer) — continue to next viewpoint
        }

        // Clean up previous debug camera if any.
        if let Some(cleanup) = turnaround.debug_cleanup.take() {
            if let Some((orig_transform, orig_global_transform, was_active)) = cleanup.reused_state
            {
                // Reused camera: restore original transform, global transform, and active state
                if let Some(mut t) = world.get_mut::<Transform>(cleanup.debug_entity) {
                    *t = orig_transform;
                }
                if let Some(mut gt) = world.get_mut::<GlobalTransform>(cleanup.debug_entity) {
                    *gt = orig_global_transform;
                }
                if let Some(mut cam) = world.get_mut::<Camera>(cleanup.debug_entity) {
                    cam.is_active = was_active;
                }
            } else {
                // Spawned camera: despawn immediately (not via commands) so
                // setup_debug_camera won't include it in the next original_cameras list.
                if let Ok(entity_mut) = world.get_entity_mut(cleanup.debug_entity) {
                    entity_mut.despawn();
                }
            }
            for (cam_entity, was_active) in cleanup.original_cameras {
                if let Some(mut cam) = world.get_mut::<Camera>(cam_entity) {
                    cam.is_active = was_active;
                }
            }
        }

        // Are we done?
        if turnaround.current_index >= turnaround.viewpoints.len() {
            // Restore UI
            if let Some(restore) = turnaround.ui_restore.take() {
                for (ui_entity, original_vis) in restore {
                    if let Some(mut vis) = world.get_mut::<Visibility>(ui_entity) {
                        *vis = original_vis;
                    }
                }
            }

            // Composite and send response
            let result = composite_turnaround(&mut turnaround);
            if let Some(tx) = turnaround.response_tx.take() {
                let _ = tx.send(result);
            }
            continue;
        }

        // Hide UI on first viewpoint
        if turnaround.current_index == 0 && turnaround.hide_ui && turnaround.ui_restore.is_none() {
            turnaround.ui_restore = Some(hide_ui_nodes(world));
        }

        // Set up debug camera for current viewpoint
        let view = &turnaround.viewpoints[turnaround.current_index];
        let debug_req = DebugCameraRequest {
            position: view.position,
            look_at: turnaround.look_at,
        };
        let cleanup = setup_debug_camera(world, &debug_req);
        turnaround.debug_cleanup = Some(cleanup);

        // Wait 2 frames for camera to render, then capture
        turnaround.frames_remaining = 2;
        turnaround.pending_screenshot_entity = None;

        // We'll spawn the screenshot on the next iteration when frames_remaining hits 0
        // Actually, we need to schedule the screenshot spawn after the delay
        // Let's set a flag to spawn after delay
        still_active.push(turnaround);
    }

    pending.active = still_active;

    // Second pass: spawn screenshots for turnarounds that just finished their delay
    let mut screenshots_to_spawn = Vec::new();
    for (idx, turnaround) in pending.active.iter_mut().enumerate() {
        if turnaround.frames_remaining == 0
            && turnaround.pending_screenshot_entity.is_none()
            && turnaround.current_index < turnaround.viewpoints.len()
            && turnaround.debug_cleanup.is_some()
        {
            screenshots_to_spawn.push(idx);
        }
    }

    let has_window = world
        .query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .is_some();

    if has_window {
        for idx in screenshots_to_spawn {
            let entity = world.spawn(Screenshot::primary_window()).id();
            let capture_index = pending.active[idx].current_index;

            let mut tc = world.get_resource_or_insert_with(TurnaroundCaptures::default);
            tc.map.insert(entity, capture_index);

            pending.active[idx].pending_screenshot_entity = Some(entity);
            pending.active[idx].current_index += 1;
            pending.active[idx].frames_remaining = 2;
        }
    } else {
        // Headless fallback: capture from HeadlessFrameBuffer
        if let Ok(rgb) = super::screenshot::read_headless_frame(world) {
            for idx in screenshots_to_spawn {
                let capture_index = pending.active[idx].current_index;
                pending.active[idx]
                    .captures
                    .push((capture_index, rgb.clone()));
                pending.active[idx].current_index += 1;
                pending.active[idx].frames_remaining = 2;
            }
        }
    }

    world.insert_resource(pending);
}

/// Handle turnaround screenshot captures from the observer.
pub fn handle_turnaround_capture(
    entity: Entity,
    image: &bevy::image::Image,
    turnarounds: &mut PendingTurnarounds,
    turnaround_captures: &mut TurnaroundCaptures,
) -> bool {
    let Some(capture_index) = turnaround_captures.map.remove(&entity) else {
        return false;
    };

    let dyn_img = match image.clone().try_into_dynamic() {
        Ok(d) => d,
        Err(e) => {
            bevy::log::error!("[MCP] Turnaround capture failed to convert image: {e:?}");
            return true;
        }
    };
    let rgb = dyn_img.to_rgb8();

    // Find the active turnaround and store the capture
    for turnaround in &mut turnarounds.active {
        if turnaround
            .pending_screenshot_entity
            .is_some_and(|e| !turnaround_captures.map.contains_key(&e))
        {
            turnaround.captures.push((capture_index, rgb));
            return true;
        }
    }

    // Fallback: store in first active turnaround
    if let Some(turnaround) = turnarounds.active.first_mut() {
        turnaround.captures.push((capture_index, rgb));
    }

    true
}

/// Composite turnaround captures into a contact sheet with angle labels.
fn composite_turnaround(
    turnaround: &mut ActiveTurnaround,
) -> Result<serde_json::Value, ControlError> {
    turnaround.captures.sort_by_key(|(idx, _)| *idx);

    let cols = turnaround.columns.max(1) as usize;
    let count = turnaround.captures.len();
    let rows = count.div_ceil(cols);

    if count == 0 {
        return Err(ControlError::internal("No turnaround captures collected"));
    }

    let cell_w = turnaround.captures[0].1.width();
    let cell_h = turnaround.captures[0].1.height();
    let label_height = 20u32;

    let canvas_w = cell_w * cols as u32;
    let canvas_h = (cell_h + label_height) * rows as u32;

    let mut canvas = RgbImage::from_pixel(canvas_w, canvas_h, Rgb([30, 30, 30]));

    for (i, (capture_idx, frame)) in turnaround.captures.iter().enumerate() {
        let col = (i % cols) as u32;
        let row = (i / cols) as u32;
        let x = col * cell_w;
        let y = row * (cell_h + label_height);

        // Resize frame to cell size if needed
        let resized;
        let src = if frame.width() != cell_w || frame.height() != cell_h {
            resized = image::imageops::resize(
                frame,
                cell_w,
                cell_h,
                image::imageops::FilterType::Triangle,
            );
            &resized
        } else {
            frame
        };

        image::imageops::overlay(&mut canvas, src, x as i64, y as i64);

        // Draw label bar
        let label = if *capture_idx < turnaround.viewpoints.len() {
            &turnaround.viewpoints[*capture_idx].label
        } else {
            "?"
        };

        // Simple label: colored bar with hue cycling
        let hue = (i as f64 / count.max(1) as f64) * 300.0;
        let color = hsv_to_rgb(hue, 0.7, 0.85);
        let bar_y = y + cell_h;
        for bx in x..x + cell_w {
            for by in bar_y..bar_y + label_height {
                if bx < canvas_w && by < canvas_h {
                    canvas.put_pixel(bx, by, color);
                }
            }
        }
        let _ = label; // Label text rendering would require a font; the color bar provides ordering
    }

    // Resize if max_width set
    if let Some(max_w) = turnaround.max_width
        && canvas.width() > max_w
    {
        let scale = max_w as f64 / canvas.width() as f64;
        let new_height = (canvas.height() as f64 * scale).round() as u32;
        canvas = image::imageops::resize(
            &canvas,
            max_w,
            new_height,
            image::imageops::FilterType::Triangle,
        );
    }

    let width = canvas.width();
    let height = canvas.height();

    let mut buf = Vec::new();
    canvas
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| ControlError::internal(format!("Failed to encode turnaround PNG: {e}")))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);

    // Build view metadata
    let views: Vec<serde_json::Value> = turnaround
        .viewpoints
        .iter()
        .enumerate()
        .map(|(i, v)| {
            serde_json::json!({
                "index": i,
                "label": v.label,
                "position": v.position,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "image": b64,
        "width": width,
        "height": height,
        "format": "png",
        "encoding": "base64",
        "view_count": count,
        "views": views,
    }))
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> Rgb<u8> {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Rgb([
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ])
}

#[cfg(test)]
mod tests {
    use bevy::camera::primitives::Aabb;

    use super::*;

    #[test]
    fn compute_viewpoints_default_6_views_with_top() {
        let views = compute_viewpoints([0.0, 0.0, 0.0], 10.0, 25.0, 6, true);
        // 6 orbital + 1 top = 7
        assert_eq!(views.len(), 7);

        // Last view should be "top"
        assert_eq!(views[6].label, "top");
    }

    #[test]
    fn compute_viewpoints_correct_distance() {
        let look_at = [0.0, 0.0, 0.0];
        let distance = 10.0;
        let elevation = 25.0;
        let views = compute_viewpoints(look_at, distance, elevation, 6, true);

        let center = Vec3::from_array(look_at);
        for view in &views[..6] {
            // Each orbital viewpoint should be at the correct distance from look_at
            let pos = Vec3::from_array(view.position);
            let dist = (pos - center).length();
            assert!(
                (dist - distance).abs() < 0.01,
                "View {} at distance {}, expected {}",
                view.label,
                dist,
                distance
            );
        }
    }

    #[test]
    fn compute_viewpoints_correct_elevation() {
        let look_at = [0.0, 0.0, 0.0];
        let distance = 10.0;
        let elevation = 25.0;
        let views = compute_viewpoints(look_at, distance, elevation, 4, false);

        let elev_rad = elevation.to_radians();
        let expected_y = distance * elev_rad.sin();

        for view in &views {
            assert!(
                (view.position[1] - expected_y).abs() < 0.01,
                "View {} Y={}, expected {}",
                view.label,
                view.position[1],
                expected_y
            );
        }
    }

    #[test]
    fn compute_viewpoints_without_top() {
        let views = compute_viewpoints([0.0, 0.0, 0.0], 10.0, 25.0, 6, false);
        assert_eq!(views.len(), 6);
        // No "top" label
        assert!(views.iter().all(|v| v.label != "top"));
    }

    #[test]
    fn compute_viewpoints_labels() {
        let views = compute_viewpoints([0.0, 0.0, 0.0], 10.0, 25.0, 6, true);
        assert_eq!(views[0].label, "0°");
        assert_eq!(views[1].label, "60°");
        assert_eq!(views[2].label, "120°");
        assert_eq!(views[3].label, "180°");
        assert_eq!(views[4].label, "240°");
        assert_eq!(views[5].label, "300°");
        assert_eq!(views[6].label, "top");
    }

    #[test]
    fn compute_viewpoints_single_view() {
        let views = compute_viewpoints([1.0, 2.0, 3.0], 5.0, 30.0, 1, false);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].label, "0°");
    }

    #[test]
    fn compute_viewpoints_top_position() {
        let look_at = [1.0, 2.0, 3.0];
        let distance = 10.0;
        let views = compute_viewpoints(look_at, distance, 25.0, 3, true);
        let top = &views[3];
        assert_eq!(top.label, "top");
        assert!((top.position[0] - look_at[0]).abs() < 0.01);
        assert!((top.position[1] - (look_at[1] + distance)).abs() < 0.01);
        // Z has small offset to avoid degenerate look_at
        assert!((top.position[2] - (look_at[2] + 0.001)).abs() < 0.01);
    }

    #[test]
    fn compute_scene_bounds_empty_world() {
        let mut world = World::default();
        assert!(compute_scene_bounds(&mut world).is_none());
    }

    #[test]
    fn compute_scene_bounds_single_entity() {
        let mut world = World::default();
        let aabb = Aabb::from_min_max(Vec3::new(-1.0, -2.0, -3.0), Vec3::new(1.0, 2.0, 3.0));
        world.spawn((aabb, GlobalTransform::default()));

        let result = compute_scene_bounds(&mut world);
        assert!(result.is_some());
        let (min, max) = result.unwrap();
        assert!((min.x - (-1.0)).abs() < 0.01);
        assert!((min.y - (-2.0)).abs() < 0.01);
        assert!((min.z - (-3.0)).abs() < 0.01);
        assert!((max.x - 1.0).abs() < 0.01);
        assert!((max.y - 2.0).abs() < 0.01);
        assert!((max.z - 3.0).abs() < 0.01);
    }

    #[test]
    fn compute_scene_bounds_multiple_entities() {
        let mut world = World::default();

        // Entity at origin with small AABB
        let aabb1 = Aabb::from_min_max(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5));
        world.spawn((aabb1, GlobalTransform::default()));

        // Entity translated to x=10 with AABB [-1,1]
        let aabb2 = Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        world.spawn((
            aabb2,
            GlobalTransform::from(Transform::from_xyz(10.0, 0.0, 0.0)),
        ));

        let result = compute_scene_bounds(&mut world);
        assert!(result.is_some());
        let (min, max) = result.unwrap();

        // Scene should encompass both: min.x <= -0.5, max.x >= 11.0
        assert!(min.x <= -0.5 + 0.01);
        assert!(max.x >= 11.0 - 0.01);
        // Y should span at least [-1, 1]
        assert!(min.y <= -1.0 + 0.01);
        assert!(max.y >= 1.0 - 0.01);
    }

    #[test]
    fn compute_scene_bounds_translated_entity() {
        let mut world = World::default();
        // Unit AABB [-1,1] translated to (5,5,5) → world bounds [4,6]
        let aabb = Aabb::from_min_max(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        world.spawn((
            aabb,
            GlobalTransform::from(Transform::from_xyz(5.0, 5.0, 5.0)),
        ));

        let result = compute_scene_bounds(&mut world);
        assert!(result.is_some());
        let (min, max) = result.unwrap();
        assert!((min.x - 4.0).abs() < 0.01);
        assert!((min.y - 4.0).abs() < 0.01);
        assert!((min.z - 4.0).abs() < 0.01);
        assert!((max.x - 6.0).abs() < 0.01);
        assert!((max.y - 6.0).abs() < 0.01);
        assert!((max.z - 6.0).abs() < 0.01);
    }

    #[test]
    fn hsv_to_rgb_red() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), Rgb([255, 0, 0]));
    }

    #[test]
    fn hsv_to_rgb_yellow() {
        assert_eq!(hsv_to_rgb(60.0, 1.0, 1.0), Rgb([255, 255, 0]));
    }

    #[test]
    fn hsv_to_rgb_cyan() {
        assert_eq!(hsv_to_rgb(180.0, 1.0, 1.0), Rgb([0, 255, 255]));
    }

    #[test]
    fn hsv_to_rgb_black() {
        assert_eq!(hsv_to_rgb(0.0, 0.0, 0.0), Rgb([0, 0, 0]));
    }

    #[test]
    fn hsv_to_rgb_gray() {
        let result = hsv_to_rgb(0.0, 0.0, 0.5);
        // 0.5 * 255 = 127.5, rounds to 128
        assert!((result.0[0] as i32 - 128).abs() <= 1);
        assert!((result.0[1] as i32 - 128).abs() <= 1);
        assert!((result.0[2] as i32 - 128).abs() <= 1);
    }

    fn make_turnaround(
        viewpoints: Vec<TurnaroundView>,
        captures: Vec<(usize, RgbImage)>,
        columns: u32,
        max_width: Option<u32>,
    ) -> ActiveTurnaround {
        ActiveTurnaround {
            response_tx: None,
            viewpoints,
            current_index: 0,
            captures,
            columns,
            max_width,
            frames_remaining: 0,
            hide_ui: false,
            ui_restore: None,
            debug_cleanup: None,
            look_at: [0.0, 0.0, 0.0],
            pending_screenshot_entity: None,
        }
    }

    #[test]
    fn composite_turnaround_single_capture() {
        let mut t = make_turnaround(
            vec![TurnaroundView {
                position: [0.0, 0.0, 10.0],
                label: "0°".into(),
            }],
            vec![(0, RgbImage::from_pixel(4, 4, Rgb([100, 100, 100])))],
            3,
            None,
        );
        let result = composite_turnaround(&mut t);
        assert!(result.is_ok());
        let val = result.unwrap();
        assert!(val.get("image").is_some());
        assert!(val.get("width").is_some());
        assert!(val.get("height").is_some());
        assert_eq!(val["view_count"], 1);
    }

    #[test]
    fn composite_turnaround_empty_captures() {
        let mut t = make_turnaround(
            vec![TurnaroundView {
                position: [0.0, 0.0, 10.0],
                label: "0°".into(),
            }],
            vec![],
            3,
            None,
        );
        let result = composite_turnaround(&mut t);
        assert!(result.is_err());
    }

    #[test]
    fn composite_turnaround_max_width_resize() {
        let mut t = make_turnaround(
            vec![TurnaroundView {
                position: [0.0, 0.0, 10.0],
                label: "0°".into(),
            }],
            vec![(0, RgbImage::from_pixel(100, 100, Rgb([50, 50, 50])))],
            1,
            Some(50),
        );
        let result = composite_turnaround(&mut t);
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["width"], 50);
    }

    #[test]
    fn composite_turnaround_grid_layout() {
        let captures: Vec<(usize, RgbImage)> = (0..4)
            .map(|i| (i, RgbImage::from_pixel(10, 10, Rgb([80, 80, 80]))))
            .collect();
        let viewpoints: Vec<TurnaroundView> = (0..4)
            .map(|i| TurnaroundView {
                position: [0.0, 0.0, 10.0],
                label: format!("{i}"),
            })
            .collect();
        let mut t = make_turnaround(viewpoints, captures, 2, None);
        let result = composite_turnaround(&mut t);
        assert!(result.is_ok());
        let val = result.unwrap();
        // 2 columns * 10px = 20px width
        assert_eq!(val["width"], 20);
    }
}
