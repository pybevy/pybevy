use std::{
    collections::{HashMap, VecDeque},
    io::Cursor,
};

use base64::Engine;
use bevy::{
    ecs::world::World,
    gizmos::config::{DefaultGizmoConfigGroup, GizmoConfigStore},
    prelude::*,
    render::view::window::screenshot::{Screenshot, ScreenshotCaptured},
    window::PrimaryWindow,
};
use image::{ImageFormat, Rgb, RgbImage};
use tokio::sync::oneshot;

use crate::bridge::{ControlError, DebugCameraRequest, InternalOverlayUi, PendingScreenshots};

/// Resource storing the latest GPU readback frame for headless screenshots.
///
/// Updated each frame by a system registered by `ImageCopyPlugin`.
/// Read by screenshot/timeline/turnaround handlers when no primary window exists.
#[derive(Resource, Default)]
pub struct HeadlessFrameBuffer {
    pub latest: Option<(Vec<u8>, u32, u32)>,
}

/// Cleanup info for a debug camera set up for a screenshot.
pub struct DebugCameraCleanup {
    pub debug_entity: Entity,
    /// If we reused an existing camera: (original_transform, original_global_transform, original_is_active).
    /// If None, this was a freshly spawned camera that should be despawned on cleanup.
    pub reused_state: Option<(Transform, GlobalTransform, bool)>,
    /// Other cameras and their original `is_active` state before we disabled them.
    pub original_cameras: Vec<(Entity, bool)>,
}

/// Per-screenshot responder stored until the observer fires.
pub struct ScreenshotResponder {
    pub response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    pub max_width: Option<u32>,
    pub debug_cleanup: Option<DebugCameraCleanup>,
    pub ui_restore: Option<Vec<(Entity, Visibility)>>,
    /// If gizmos were toggled for this screenshot, the original enabled state to restore.
    pub gizmo_restore: Option<bool>,
}

/// Resource mapping screenshot Entity → responder info.
#[derive(Resource, Default)]
pub struct PendingScreenshotResponders {
    pub map: HashMap<Entity, ScreenshotResponder>,
}

/// Staged debug screenshots waiting for the debug camera to render before capture.
#[derive(Resource, Default)]
struct StagedDebugScreenshots {
    pending: Vec<StagedDebugScreenshot>,
}

struct StagedDebugScreenshot {
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    frames_remaining: u32,
    with_gizmos: bool,
    max_width: Option<u32>,
    debug_cleanup: DebugCameraCleanup,
    ui_restore: Option<Vec<(Entity, Visibility)>>,
}

/// Resource tracking active timelines (multi-frame capture sequences).
#[derive(Resource, Default)]
pub struct PendingTimelines {
    pub active: HashMap<u64, ActiveTimeline>,
    pub next_id: u64,
}

/// A single active timeline capturing N frames over time.
pub struct ActiveTimeline {
    pub response_tx: Option<oneshot::Sender<Result<serde_json::Value, ControlError>>>,
    pub max_width: Option<u32>,
    pub columns: u32,
    pub debug_cleanup: Option<DebugCameraCleanup>,
    pub schedule: VecDeque<u32>,
    pub total_captures: u32,
    pub next_capture_index: u32,
    pub collected: Vec<(u32, RgbImage)>,
}

/// Resource mapping screenshot Entity → (timeline_id, capture_index).
#[derive(Resource, Default)]
pub struct TimelineCaptures {
    pub map: HashMap<Entity, (u64, u32)>,
}

/// Compute capture schedule as frame deltas.
///
/// For count=6, total=60: targets [0,12,24,36,48,60], deltas [0,12,12,12,12,12]
pub fn compute_schedule(total_frames: u32, capture_count: u32) -> VecDeque<u32> {
    if capture_count <= 1 {
        return VecDeque::from([0]);
    }
    let interval = total_frames as f64 / (capture_count - 1) as f64;
    let mut schedule = VecDeque::new();
    let mut prev = 0u32;
    for i in 0..capture_count {
        let target = (i as f64 * interval).round() as u32;
        schedule.push_back(target - prev);
        prev = target;
    }
    schedule
}

/// Process pending screenshot requests (called each frame in Last schedule).
///
/// Flow:
/// 1. Count down `frames_remaining` on normal pending screenshots
/// 2. When ready: if `debug_camera` is set, set up the debug camera and stage with extra delay
/// 3. Count down staged debug screenshots
/// 4. When staged screenshot is ready, spawn the Screenshot entity for capture
pub fn process_pending_screenshots(world: &mut World) {
    let Some(mut pending) = world.remove_resource::<PendingScreenshots>() else {
        return;
    };

    if pending.pending.is_empty()
        && world
            .get_resource::<StagedDebugScreenshots>()
            .is_none_or(|s| s.pending.is_empty())
    {
        world.insert_resource(pending);
        return;
    }

    let mut remaining = Vec::new();
    let mut ready = Vec::new();

    for mut screenshot in pending.pending.drain(..) {
        if screenshot.frames_remaining > 0 {
            screenshot.frames_remaining -= 1;
            remaining.push(screenshot);
        } else {
            ready.push(screenshot);
        }
    }

    pending.pending = remaining;
    world.insert_resource(pending);

    // Process ready screenshots
    for mut screenshot in ready {
        // Always hide internal overlay entities (hot reload UI)
        let mut overlay_restore = hide_internal_overlay(world);
        // Additionally hide all authored UI if requested
        let ui_restore = if screenshot.hide_ui {
            let mut all = hide_ui_nodes(world);
            // Merge overlay restores (avoid duplicates)
            all.append(&mut overlay_restore);
            Some(all)
        } else if !overlay_restore.is_empty() {
            Some(overlay_restore)
        } else {
            None
        };

        if let Some(debug_req) = screenshot.debug_camera.take() {
            // Set up debug camera and stage for extra frames
            let cleanup = setup_debug_camera(world, &debug_req);

            let mut staged = world.get_resource_or_insert_with(StagedDebugScreenshots::default);
            staged.pending.push(StagedDebugScreenshot {
                response_tx: screenshot.response_tx,
                frames_remaining: 2,
                with_gizmos: screenshot.with_gizmos,
                max_width: screenshot.max_width,
                debug_cleanup: cleanup,
                ui_restore,
            });
        } else {
            // Normal path: spawn Screenshot entity immediately
            let has_window = world
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .iter(world)
                .next()
                .is_some();

            if has_window {
                let gizmo_restore = if !screenshot.with_gizmos {
                    set_gizmos_enabled(world, false)
                } else {
                    None
                };

                let entity = world.spawn(Screenshot::primary_window()).id();

                let mut responders =
                    world.get_resource_or_insert_with(PendingScreenshotResponders::default);
                responders.map.insert(
                    entity,
                    ScreenshotResponder {
                        response_tx: screenshot.response_tx,
                        max_width: screenshot.max_width,
                        debug_cleanup: None,
                        ui_restore,
                        gizmo_restore,
                    },
                );
            } else {
                // Headless fallback: read from HeadlessFrameProvider
                let result = capture_headless_frame(world, screenshot.max_width);
                let _ = screenshot.response_tx.send(result);
            }
        }
    }

    // Process staged debug screenshots
    if let Some(mut staged) = world.remove_resource::<StagedDebugScreenshots>() {
        let mut still_waiting = Vec::new();

        for mut s in staged.pending.drain(..) {
            if s.frames_remaining > 0 {
                s.frames_remaining -= 1;
                still_waiting.push(s);
            } else {
                // Debug camera has rendered — capture now
                let has_window = world
                    .query_filtered::<Entity, With<PrimaryWindow>>()
                    .iter(world)
                    .next()
                    .is_some();

                if has_window {
                    let gizmo_restore = if !s.with_gizmos {
                        set_gizmos_enabled(world, false)
                    } else {
                        None
                    };

                    let entity = world.spawn(Screenshot::primary_window()).id();

                    let mut responders =
                        world.get_resource_or_insert_with(PendingScreenshotResponders::default);
                    responders.map.insert(
                        entity,
                        ScreenshotResponder {
                            response_tx: s.response_tx,
                            max_width: s.max_width,
                            debug_cleanup: Some(s.debug_cleanup),
                            ui_restore: s.ui_restore,
                            gizmo_restore,
                        },
                    );
                } else {
                    // Headless fallback
                    let result = capture_headless_frame(world, s.max_width);
                    let _ = s.response_tx.send(result);
                }
            }
        }

        staged.pending = still_waiting;
        world.insert_resource(staged);
    }
}

/// Process pending timelines: decrement schedule, spawn captures when ready.
pub fn process_pending_timelines(world: &mut World) {
    let Some(mut timelines) = world.remove_resource::<PendingTimelines>() else {
        return;
    };

    if timelines.active.is_empty() {
        world.insert_resource(timelines);
        return;
    }

    // Collect timeline IDs that need a capture this frame
    let mut captures_to_spawn: Vec<(u64, u32)> = Vec::new();
    let mut completed_ids: Vec<u64> = Vec::new();

    for (&id, timeline) in timelines.active.iter_mut() {
        if timeline.schedule.is_empty() {
            // All captures scheduled, waiting for collection
            if timeline.response_tx.is_none() {
                completed_ids.push(id);
            }
            continue;
        }

        // Decrement front of schedule
        if let Some(front) = timeline.schedule.front_mut() {
            if *front > 0 {
                *front -= 1;
            } else {
                // Time to capture
                let capture_index = timeline.next_capture_index;
                timeline.next_capture_index += 1;
                timeline.schedule.pop_front();
                captures_to_spawn.push((id, capture_index));
            }
        }
    }

    // Remove fully completed timelines (response already sent)
    for id in completed_ids {
        timelines.active.remove(&id);
    }

    world.insert_resource(timelines);

    let has_window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .is_some();

    if has_window {
        // Spawn screenshot entities for captures
        for (timeline_id, capture_index) in captures_to_spawn {
            let entity = world.spawn(Screenshot::primary_window()).id();

            let mut timeline_captures =
                world.get_resource_or_insert_with(TimelineCaptures::default);
            timeline_captures
                .map
                .insert(entity, (timeline_id, capture_index));
        }
    } else {
        // Headless fallback: read from HeadlessFrameBuffer
        if let Ok(rgb) = read_headless_frame(world) {
            let mut timelines = world.resource_mut::<PendingTimelines>();
            for (timeline_id, capture_index) in captures_to_spawn {
                if let Some(timeline) = timelines.active.get_mut(&timeline_id) {
                    timeline.collected.push((capture_index, rgb.clone()));
                    if timeline.collected.len() as u32 == timeline.total_captures {
                        let result = composite_contact_sheet(timeline);
                        if let Some(tx) = timeline.response_tx.take() {
                            let _ = tx.send(result);
                        }
                    }
                }
            }
        }
    }
}

/// Set gizmo visibility for the default gizmo group, returning the original enabled state.
fn set_gizmos_enabled(world: &mut World, enabled: bool) -> Option<bool> {
    let mut store = world.get_resource_mut::<GizmoConfigStore>()?;
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    let was_enabled = config.enabled;
    if was_enabled != enabled {
        config.enabled = enabled;
        Some(was_enabled)
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
        // Force immediate render-pipeline visibility update
        // (bypass PostUpdate VisibilityPropagate which has already run)
        if let Some(mut inherited) = world.get_mut::<InheritedVisibility>(*entity) {
            *inherited = InheritedVisibility::HIDDEN;
        }
        if let Some(mut view_vis) = world.get_mut::<ViewVisibility>(*entity) {
            *view_vis = ViewVisibility::HIDDEN;
        }
    }
    ui_entities
}

/// Hide internal overlay UI entities (hot reload status, error text) unconditionally.
fn hide_internal_overlay(world: &mut World) -> Vec<(Entity, Visibility)> {
    let mut overlay_entities: Vec<(Entity, Visibility)> = Vec::new();
    let mut query = world.query::<(Entity, &Visibility, &InternalOverlayUi)>();
    for (entity, vis, _) in query.iter(world) {
        overlay_entities.push((entity, *vis));
    }
    for (entity, _) in &overlay_entities {
        if let Some(mut vis) = world.get_mut::<Visibility>(*entity) {
            *vis = Visibility::Hidden;
        }
        // Force immediate render-pipeline visibility update
        // (bypass PostUpdate VisibilityPropagate which has already run)
        if let Some(mut inherited) = world.get_mut::<InheritedVisibility>(*entity) {
            *inherited = InheritedVisibility::HIDDEN;
        }
        if let Some(mut view_vis) = world.get_mut::<ViewVisibility>(*entity) {
            *view_vis = ViewVisibility::HIDDEN;
        }
    }
    overlay_entities
}

/// Set up a debug camera for a screenshot.
///
/// Reuses an existing Camera3d when possible to avoid a crash in
/// `prepare_lights` — a newly spawned Camera3d won't have cascade shadow
/// data populated by `build_directional_light_cascades` (runs in PostUpdate,
/// before Last where we spawn), causing an `unwrap()` panic on the cascade
/// lookup.  Reusing an existing camera preserves the cascade data.
///
/// Falls back to spawning a new camera only if no Camera3d exists.
pub(crate) fn setup_debug_camera(
    world: &mut World,
    req: &DebugCameraRequest,
) -> DebugCameraCleanup {
    let position = Vec3::from_array(req.position);
    let look_at = Vec3::from_array(req.look_at);
    let target_transform = Transform::from_translation(position).looking_at(look_at, Vec3::Y);

    // Collect all cameras and their active state
    let all_cameras: Vec<(Entity, bool)> = {
        let mut query = world.query::<(Entity, &Camera)>();
        query.iter(world).map(|(e, c)| (e, c.is_active)).collect()
    };

    // Find an existing Camera3d to reuse (prefer an active one)
    let reuse_entity: Option<Entity> = {
        let mut query = world.query_filtered::<(Entity, &Camera), With<Camera3d>>();
        let candidates: Vec<(Entity, bool)> =
            query.iter(world).map(|(e, c)| (e, c.is_active)).collect();
        candidates
            .iter()
            .find(|(_, active)| *active)
            .or(candidates.first())
            .map(|(e, _)| *e)
    };

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
        // runs in the next PostUpdate — this function runs in Last.
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

        DebugCameraCleanup {
            debug_entity: reuse,
            reused_state: Some((
                original_transform,
                original_global_transform,
                original_active,
            )),
            original_cameras: other_cameras,
        }
    } else {
        // No Camera3d exists — spawn a new one.
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

        DebugCameraCleanup {
            debug_entity,
            reused_state: None,
            original_cameras,
        }
    }
}

/// Clean up a debug camera: restore or despawn it, then restore original cameras.
fn cleanup_debug_camera(
    cleanup: DebugCameraCleanup,
    commands: &mut Commands,
    cameras: &mut Query<&mut Camera>,
    transforms: &mut Query<&mut Transform>,
    global_transforms: &mut Query<&mut GlobalTransform>,
) {
    if let Some((orig_transform, orig_global_transform, was_active)) = cleanup.reused_state {
        if let Ok(mut t) = transforms.get_mut(cleanup.debug_entity) {
            *t = orig_transform;
        }
        // Restore GlobalTransform so propagate_transforms doesn't need an extra frame
        if let Ok(mut gt) = global_transforms.get_mut(cleanup.debug_entity) {
            *gt = orig_global_transform;
        }
        if let Ok(mut cam) = cameras.get_mut(cleanup.debug_entity) {
            cam.is_active = was_active;
        }
    } else {
        commands.entity(cleanup.debug_entity).despawn();
    }
    for (cam_entity, was_active) in cleanup.original_cameras {
        if let Ok(mut cam) = cameras.get_mut(cam_entity) {
            cam.is_active = was_active;
        }
    }
}

/// Global observer triggered by Bevy when a screenshot is captured.
/// Handles normal screenshots, timeline captures, and turnaround captures.
#[allow(clippy::too_many_arguments)]
pub fn screenshot_captured_observer(
    trigger: On<ScreenshotCaptured>,
    mut responders: ResMut<PendingScreenshotResponders>,
    mut timeline_captures: ResMut<TimelineCaptures>,
    mut timelines: ResMut<PendingTimelines>,
    mut turnarounds: ResMut<super::turnaround::PendingTurnarounds>,
    mut turnaround_captures: ResMut<super::turnaround::TurnaroundCaptures>,
    gizmo_store: Option<ResMut<GizmoConfigStore>>,
    mut commands: Commands,
    mut cameras: Query<&mut Camera>,
    mut transforms: Query<&mut Transform>,
    mut global_transforms: Query<&mut GlobalTransform>,
    mut visibility_query: Query<&mut Visibility>,
) {
    let entity = trigger.entity;

    // Check if this is a turnaround capture
    if super::turnaround::handle_turnaround_capture(
        entity,
        &trigger.image,
        &mut turnarounds,
        &mut turnaround_captures,
    ) {
        return;
    }

    // Check if this is a timeline capture
    if let Some((timeline_id, capture_index)) = timeline_captures.map.remove(&entity) {
        let img = trigger.image.clone();
        let dyn_img = match img.try_into_dynamic() {
            Ok(d) => d,
            Err(e) => {
                bevy::log::error!("[MCP] Timeline capture failed to convert image: {e:?}");
                return;
            }
        };
        let rgb = dyn_img.to_rgb8();

        if let Some(timeline) = timelines.active.get_mut(&timeline_id) {
            timeline.collected.push((capture_index, rgb));

            // Check if all captures collected
            if timeline.collected.len() as u32 == timeline.total_captures {
                let result = composite_contact_sheet(timeline);
                if let Some(tx) = timeline.response_tx.take() {
                    let _ = tx.send(result);
                }

                // Clean up debug camera if present
                if let Some(cleanup) = timeline.debug_cleanup.take() {
                    cleanup_debug_camera(
                        cleanup,
                        &mut commands,
                        &mut cameras,
                        &mut transforms,
                        &mut global_transforms,
                    );
                }
            }
        }

        return;
    }

    // Normal screenshot handling
    let Some(responder) = responders.map.remove(&entity) else {
        return;
    };

    let img = trigger.image.clone();
    let result = encode_screenshot(img, responder.max_width);
    let _ = responder.response_tx.send(result);

    // Restore hidden UI nodes
    if let Some(restore) = responder.ui_restore {
        for (ui_entity, original_vis) in restore {
            if let Ok(mut vis) = visibility_query.get_mut(ui_entity) {
                *vis = original_vis;
            }
        }
    }

    // Restore gizmo visibility
    if let Some(was_enabled) = responder.gizmo_restore
        && let Some(mut store) = gizmo_store
    {
        let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
        config.enabled = was_enabled;
    }

    // Clean up debug camera if present
    if let Some(cleanup) = responder.debug_cleanup {
        cleanup_debug_camera(
            cleanup,
            &mut commands,
            &mut cameras,
            &mut transforms,
            &mut global_transforms,
        );
    }
}

/// Composite timeline captures into a contact sheet grid image.
fn composite_contact_sheet(
    timeline: &mut ActiveTimeline,
) -> Result<serde_json::Value, ControlError> {
    // Sort by capture index
    timeline.collected.sort_by_key(|(idx, _)| *idx);

    let cols = timeline.columns.max(1) as usize;
    let count = timeline.collected.len();
    let rows = count.div_ceil(cols);

    if count == 0 {
        return Err(ControlError::internal("No captures collected"));
    }

    // Use the first image's dimensions as cell size
    let cell_w = timeline.collected[0].1.width();
    let cell_h = timeline.collected[0].1.height();
    let bar_height = 4u32;

    let canvas_w = cell_w * cols as u32;
    let canvas_h = (cell_h + bar_height) * rows as u32;

    // Dark background
    let mut canvas = RgbImage::from_pixel(canvas_w, canvas_h, Rgb([40, 40, 40]));

    for (i, (_, frame)) in timeline.collected.iter().enumerate() {
        let col = (i % cols) as u32;
        let row = (i / cols) as u32;
        let x = col * cell_w;
        let y = row * (cell_h + bar_height);

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

        // Colored bar at bottom (hue cycling for visual ordering)
        let hue = (i as f64 / count.max(1) as f64) * 300.0; // 0..300 degrees
        let color = hsv_to_rgb(hue, 0.8, 0.9);
        let bar_y = y + cell_h;
        for bx in x..x + cell_w {
            for by in bar_y..bar_y + bar_height {
                if bx < canvas_w && by < canvas_h {
                    canvas.put_pixel(bx, by, color);
                }
            }
        }
    }

    // Resize if max_width set
    if let Some(max_w) = timeline.max_width
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
        .map_err(|e| ControlError::internal(format!("Failed to encode timeline PNG: {e}")))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);

    Ok(serde_json::json!({
        "image": b64,
        "width": width,
        "height": height,
        "format": "png",
        "encoding": "base64"
    }))
}

/// Convert HSV (h: 0..360, s: 0..1, v: 0..1) to RGB.
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

/// Encode a Bevy Image as a base64 PNG string, optionally resizing.
fn encode_screenshot(
    img: bevy::image::Image,
    max_width: Option<u32>,
) -> Result<serde_json::Value, ControlError> {
    let dyn_img = img.try_into_dynamic().map_err(|e| {
        ControlError::internal(format!("Failed to convert screenshot image: {e:?}"))
    })?;

    // Discard alpha (stores HDR brightness) to get a clean RGB image
    let rgb = dyn_img.to_rgb8();
    encode_rgb_screenshot(rgb, max_width)
}

/// Encode an RgbImage as a base64 PNG string, optionally resizing.
fn encode_rgb_screenshot(
    mut rgb: RgbImage,
    max_width: Option<u32>,
) -> Result<serde_json::Value, ControlError> {
    // Resize if max_width is set and image is wider
    if let Some(max_w) = max_width
        && rgb.width() > max_w
    {
        let scale = max_w as f64 / rgb.width() as f64;
        let new_height = (rgb.height() as f64 * scale).round() as u32;
        rgb = image::imageops::resize(
            &rgb,
            max_w,
            new_height,
            image::imageops::FilterType::Triangle,
        );
    }

    let width = rgb.width();
    let height = rgb.height();

    let mut buf = Vec::new();
    rgb.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| ControlError::internal(format!("Failed to encode PNG: {e}")))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);

    Ok(serde_json::json!({
        "image": b64,
        "width": width,
        "height": height,
        "format": "png",
        "encoding": "base64"
    }))
}

/// Read the latest frame from `HeadlessFrameBuffer` and convert to an RgbImage.
pub fn read_headless_frame(world: &World) -> Result<RgbImage, ControlError> {
    let buffer = world.get_resource::<HeadlessFrameBuffer>().ok_or_else(|| {
        ControlError::internal(
            "No primary window and no HeadlessFrameBuffer registered. \
             Add ImageCopyPlugin and a camera with RenderTarget.image() for headless screenshots."
                .to_string(),
        )
    })?;

    let (rgba_bytes, w, h) = buffer.latest.as_ref().ok_or_else(|| {
        ControlError::internal(
            "HeadlessFrameBuffer has no frame yet. \
             The render pipeline may not have produced a frame yet."
                .to_string(),
        )
    })?;

    let mut rgb = RgbImage::new(*w, *h);
    for y in 0..*h {
        for x in 0..*w {
            let i = ((y * *w + x) * 4) as usize;
            if i + 2 < rgba_bytes.len() {
                rgb.put_pixel(
                    x,
                    y,
                    Rgb([rgba_bytes[i], rgba_bytes[i + 1], rgba_bytes[i + 2]]),
                );
            }
        }
    }

    Ok(rgb)
}

/// Capture a frame from the headless readback buffer and encode it as a screenshot.
fn capture_headless_frame(
    world: &mut World,
    max_width: Option<u32>,
) -> Result<serde_json::Value, ControlError> {
    let rgb = read_headless_frame(world)?;
    encode_rgb_screenshot(rgb, max_width)
}
