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

use crate::bridge::{
    ControlError, DebugCameraRequest, InternalOverlayUi, OverlaySuppression, PendingScreenshots,
};

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
    /// Extra JSON fields to merge into the screenshot response.
    pub extra_response: Option<serde_json::Value>,
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
    extra_response: Option<serde_json::Value>,
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
    /// Whether this timeline holds an `OverlaySuppression` refcount
    /// (taken at first capture, released on completion).
    pub overlay_suppressed: bool,
}

/// Resource mapping screenshot Entity → (timeline_id, capture_index).
#[derive(Resource, Default)]
pub struct TimelineCaptures {
    pub map: HashMap<Entity, (u64, u32)>,
}

/// Compute capture schedule as frame deltas.
///
/// For count=6, total=60: targets [0,12,24,36,48,60], deltas [0,12,12,12,12,12]
/// Upper bound on timeline captures. Beyond this a contact sheet is unwieldy and
/// the capture cost balloons; below 1 the schedule is a single frame while
/// total_captures stays 0, so the completion check never matches and the request
/// hangs until it times out.
pub const MAX_TIMELINE_CAPTURES: u32 = 20;

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
        // Always suppress the internal overlay (hot reload UI); released when
        // the screenshot completes
        suppress_internal_overlay(world);
        // Additionally hide all authored UI if requested
        let ui_restore = if screenshot.hide_ui {
            Some(hide_ui_nodes(world))
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
                extra_response: screenshot.extra_response,
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
                        extra_response: screenshot.extra_response,
                    },
                );
            } else {
                // Headless fallback: read from HeadlessFrameProvider
                let result = capture_headless_frame(world, screenshot.max_width);
                let result = merge_extra_response(result, screenshot.extra_response);
                let _ = screenshot.response_tx.send(result);
                release_internal_overlay(world);
                if let Some(restore) = ui_restore {
                    restore_ui_nodes(world, restore);
                }
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
                            extra_response: s.extra_response,
                        },
                    );
                } else {
                    // Headless fallback
                    let result = capture_headless_frame(world, s.max_width);
                    let result = merge_extra_response(result, s.extra_response);
                    let _ = s.response_tx.send(result);
                    release_internal_overlay(world);
                    if let Some(restore) = s.ui_restore {
                        restore_ui_nodes(world, restore);
                    }
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

    // Suppress the internal hot-reload overlay for the duration of any
    // timeline that is about to capture; released when the timeline
    // completes. The refcount makes overlapping timelines compose.
    for (id, _) in &captures_to_spawn {
        let needs_suppress = timelines
            .active
            .get(id)
            .is_some_and(|t| !t.overlay_suppressed);
        if needs_suppress {
            suppress_internal_overlay(world);
            if let Some(timeline) = timelines.active.get_mut(id) {
                timeline.overlay_suppressed = true;
            }
        }
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
            let mut overlay_releases: u32 = 0;
            {
                let mut timelines = world.resource_mut::<PendingTimelines>();
                for (timeline_id, capture_index) in captures_to_spawn {
                    if let Some(timeline) = timelines.active.get_mut(&timeline_id) {
                        timeline.collected.push((capture_index, rgb.clone()));
                        if timeline.collected.len() as u32 == timeline.total_captures {
                            let result = composite_contact_sheet(timeline);
                            if let Some(tx) = timeline.response_tx.take() {
                                let _ = tx.send(result);
                            }
                            if timeline.overlay_suppressed {
                                timeline.overlay_suppressed = false;
                                overlay_releases += 1;
                            }
                        }
                    }
                }
            }
            for _ in 0..overlay_releases {
                release_internal_overlay(world);
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

/// Hide authored UI Node entities by setting their visibility to Hidden.
/// Returns a list of (entity, original_visibility) for restoration. Internal
/// overlay entities are excluded: they are owned by `OverlaySuppression`.
fn hide_ui_nodes(world: &mut World) -> Vec<(Entity, Visibility)> {
    let mut ui_entities: Vec<(Entity, Visibility)> = Vec::new();
    let mut query = world
        .query_filtered::<(Entity, &Visibility, &bevy::ui::Node), Without<InternalOverlayUi>>();
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

/// Restore UI nodes hidden by `hide_ui_nodes` to their recorded visibility.
fn restore_ui_nodes(world: &mut World, restore: Vec<(Entity, Visibility)>) {
    for (entity, original_vis) in restore {
        if let Some(mut vis) = world.get_mut::<Visibility>(entity) {
            *vis = original_vis;
        }
    }
}

/// Suppress the internal overlay (hot reload status, error text): bump the
/// `OverlaySuppression` refcount and force-hide the overlay entities so a
/// capture triggered later this same frame is already clean (visibility
/// propagation has run by now). The overlay's own render system re-applies
/// visibility from the refcount every frame afterwards, and restores the
/// overlay once `release_internal_overlay` brings the count back to zero.
pub(crate) fn suppress_internal_overlay(world: &mut World) {
    world
        .get_resource_or_insert_with(OverlaySuppression::default)
        .0 += 1;

    let mut overlay_entities: Vec<Entity> = Vec::new();
    let mut query = world.query_filtered::<Entity, With<InternalOverlayUi>>();
    for entity in query.iter(world) {
        overlay_entities.push(entity);
    }
    for entity in overlay_entities {
        if let Some(mut vis) = world.get_mut::<Visibility>(entity) {
            *vis = Visibility::Hidden;
        }
        // Force immediate render-pipeline visibility update
        // (bypass PostUpdate VisibilityPropagate which has already run)
        if let Some(mut inherited) = world.get_mut::<InheritedVisibility>(entity) {
            *inherited = InheritedVisibility::HIDDEN;
        }
        if let Some(mut view_vis) = world.get_mut::<ViewVisibility>(entity) {
            *view_vis = ViewVisibility::HIDDEN;
        }
    }
}

/// Release one `suppress_internal_overlay` hold.
pub(crate) fn release_internal_overlay(world: &mut World) {
    if let Some(mut suppression) = world.get_resource_mut::<OverlaySuppression>() {
        suppression.0 = suppression.0.saturating_sub(1);
    }
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
    mut overlay_suppression: Option<ResMut<OverlaySuppression>>,
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

                // Release the overlay suppression held by this timeline
                if timeline.overlay_suppressed {
                    timeline.overlay_suppressed = false;
                    if let Some(suppression) = overlay_suppression.as_deref_mut() {
                        suppression.0 = suppression.0.saturating_sub(1);
                    }
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
    let result = merge_extra_response(result, responder.extra_response);
    let _ = responder.response_tx.send(result);

    // Release this screenshot's overlay suppression
    if let Some(suppression) = overlay_suppression.as_deref_mut() {
        suppression.0 = suppression.0.saturating_sub(1);
    }

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
/// If `extra` is `Some`, merges its fields into the screenshot result alongside
/// screenshot/screenshot_width/screenshot_height. On screenshot error, the extra
/// fields are still returned with a null screenshot.
fn merge_extra_response(
    result: Result<serde_json::Value, ControlError>,
    extra: Option<serde_json::Value>,
) -> Result<serde_json::Value, ControlError> {
    let Some(extra) = extra else {
        return result;
    };
    let mut merged = match result {
        Ok(sj) => serde_json::json!({
            "screenshot": sj.get("image"),
            "screenshot_width": sj.get("width"),
            "screenshot_height": sj.get("height"),
        }),
        Err(_) => serde_json::json!({
            "screenshot": null,
        }),
    };
    if let (Some(base), serde_json::Value::Object(extra_map)) = (merged.as_object_mut(), extra) {
        for (k, v) in extra_map {
            base.insert(k, v);
        }
    }
    Ok(merged)
}

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

#[cfg(test)]
mod tests {
    use bevy::{
        asset::RenderAssetUsages,
        image::Image,
        render::{
            render_resource::{Extent3d, TextureDimension, TextureFormat},
            view::window::screenshot::Screenshot,
        },
        ui::Node,
    };

    use super::*;
    use crate::bridge::PendingScreenshot;

    #[test]
    fn compute_schedule_even_distribution() {
        let schedule = compute_schedule(60, 4);
        // 4 captures across 60 frames: targets [0, 20, 40, 60], deltas [0, 20, 20, 20]
        assert_eq!(schedule.len(), 4);
        assert_eq!(schedule[0], 0);
        assert_eq!(schedule[1], 20);
        assert_eq!(schedule[2], 20);
        assert_eq!(schedule[3], 20);
    }

    #[test]
    fn compute_schedule_single_capture() {
        let schedule = compute_schedule(60, 1);
        assert_eq!(schedule.len(), 1);
        assert_eq!(schedule[0], 0);
    }

    #[test]
    fn compute_schedule_two_captures() {
        let schedule = compute_schedule(60, 2);
        assert_eq!(schedule.len(), 2);
        assert_eq!(schedule[0], 0);
        assert_eq!(schedule[1], 60);
    }

    #[test]
    fn compute_schedule_six_captures() {
        let schedule = compute_schedule(60, 6);
        assert_eq!(schedule.len(), 6);
        // First capture at frame 0
        assert_eq!(schedule[0], 0);
        // Sum of all deltas should equal total_frames
        let total: u32 = schedule.iter().sum();
        assert_eq!(total, 60);
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
        let cleanup = setup_debug_camera(&mut world, &req);

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
        let cleanup = setup_debug_camera(&mut world, &req);

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
        let cleanup = setup_debug_camera(&mut world, &req);

        // Should have spawned a new camera (reused_state is None)
        assert!(cleanup.reused_state.is_none());
        assert!(world.get::<Camera3d>(cleanup.debug_entity).is_some());
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
        let cleanup = setup_debug_camera(&mut world, &req);

        // Should prefer the active camera
        assert_eq!(cleanup.debug_entity, active);
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
        let cleanup = setup_debug_camera(&mut world, &req);

        // Simulate cleanup (same logic as turnaround handler)
        if let Some((orig_transform, orig_global_transform, was_active)) = cleanup.reused_state {
            if let Some(mut t) = world.get_mut::<Transform>(cleanup.debug_entity) {
                *t = orig_transform;
            }
            if let Some(mut gt) = world.get_mut::<GlobalTransform>(cleanup.debug_entity) {
                *gt = orig_global_transform;
            }
            if let Some(mut c) = world.get_mut::<Camera>(cleanup.debug_entity) {
                c.is_active = was_active;
            }
        }
        for (entity, was_active) in cleanup.original_cameras {
            if let Some(mut c) = world.get_mut::<Camera>(entity) {
                c.is_active = was_active;
            }
        }

        // Camera should be restored to original position
        let restored = world.get::<Transform>(cam).unwrap();
        assert_eq!(restored.translation, original_transform.translation);
        assert!(world.get::<Camera>(cam).unwrap().is_active);
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
        let cleanup = setup_debug_camera(&mut world, &req);

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
        let cleanup = setup_debug_camera(&mut world, &req);

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

    #[test]
    fn compute_schedule_zero_total_frames() {
        let schedule = compute_schedule(0, 3);
        assert_eq!(schedule.len(), 3);
        // All deltas should be 0
        for delta in &schedule {
            assert_eq!(*delta, 0);
        }
    }

    fn world_with_gizmos() -> World {
        let mut world = World::new();
        let mut store = GizmoConfigStore::default();
        store.insert(GizmoConfig::default(), DefaultGizmoConfigGroup);
        world.insert_resource(store);
        world
    }

    fn gizmos_enabled(world: &World) -> bool {
        let store = world.resource::<GizmoConfigStore>();
        let (config, _) = store.config::<DefaultGizmoConfigGroup>();
        config.enabled
    }

    #[test]
    fn set_gizmos_enabled_disables_when_enabled() {
        let mut world = world_with_gizmos();
        assert!(gizmos_enabled(&world));

        let restore = set_gizmos_enabled(&mut world, false);
        assert!(!gizmos_enabled(&world));
        assert_eq!(restore, Some(true));
    }

    #[test]
    fn set_gizmos_enabled_noop_when_already_matches() {
        let mut world = world_with_gizmos();
        // Gizmos default to enabled; setting true should be a no-op
        let restore = set_gizmos_enabled(&mut world, true);
        assert!(gizmos_enabled(&world));
        assert_eq!(restore, None);
    }

    #[test]
    fn set_gizmos_enabled_returns_none_without_resource() {
        let mut world = World::new();
        let restore = set_gizmos_enabled(&mut world, false);
        assert_eq!(restore, None);
    }

    #[test]
    fn set_gizmos_enabled_roundtrip_restores_state() {
        let mut world = world_with_gizmos();
        assert!(gizmos_enabled(&world));

        // Disable gizmos
        let restore = set_gizmos_enabled(&mut world, false);
        assert!(!gizmos_enabled(&world));

        // Restore original state
        if let Some(was_enabled) = restore {
            set_gizmos_enabled(&mut world, was_enabled);
        }
        assert!(gizmos_enabled(&world));
    }

    #[test]
    fn set_gizmos_enabled_double_disable_second_is_noop() {
        let mut world = world_with_gizmos();

        let first = set_gizmos_enabled(&mut world, false);
        assert_eq!(first, Some(true));

        // Already disabled — should be a no-op
        let second = set_gizmos_enabled(&mut world, false);
        assert_eq!(second, None);
    }

    /// Set up a World with gizmos + PendingScreenshots + a PrimaryWindow entity.
    fn world_with_pending_screenshot(
        with_gizmos: bool,
    ) -> (
        World,
        oneshot::Receiver<Result<serde_json::Value, ControlError>>,
    ) {
        let mut world = world_with_gizmos();
        // Spawn a PrimaryWindow entity so the windowed screenshot path is taken
        world.spawn(PrimaryWindow);
        let (tx, rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                with_gizmos,
                max_width: None,
                debug_camera: None,
                hide_ui: false,
                extra_response: None,
            }],
        });
        (world, rx)
    }

    #[test]
    fn process_screenshot_disables_gizmos_for_normal_capture() {
        let (mut world, _rx) = world_with_pending_screenshot(false);
        assert!(gizmos_enabled(&world));

        process_pending_screenshots(&mut world);

        // Gizmos should be disabled after processing a non-gizmo screenshot
        assert!(!gizmos_enabled(&world));
    }

    #[test]
    fn process_screenshot_leaves_gizmos_for_gizmo_capture() {
        let (mut world, _rx) = world_with_pending_screenshot(true);
        assert!(gizmos_enabled(&world));

        process_pending_screenshots(&mut world);

        // Gizmos should remain enabled for a gizmo screenshot
        assert!(gizmos_enabled(&world));
    }

    #[test]
    fn process_screenshot_spawns_screenshot_entity() {
        let (mut world, _rx) = world_with_pending_screenshot(false);

        process_pending_screenshots(&mut world);

        // A Screenshot entity should have been spawned
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 1);
    }

    #[test]
    fn process_screenshot_stores_gizmo_restore_in_responder() {
        let (mut world, _rx) = world_with_pending_screenshot(false);

        process_pending_screenshots(&mut world);

        let responders = world.resource::<PendingScreenshotResponders>();
        assert_eq!(responders.map.len(), 1);
        let responder = responders.map.values().next().unwrap();
        // Should store the original enabled state for restoration
        assert_eq!(responder.gizmo_restore, Some(true));
    }

    #[test]
    fn process_screenshot_gizmo_capture_has_no_restore() {
        let (mut world, _rx) = world_with_pending_screenshot(true);

        process_pending_screenshots(&mut world);

        let responders = world.resource::<PendingScreenshotResponders>();
        let responder = responders.map.values().next().unwrap();
        // Gizmo screenshot should not need restoration
        assert_eq!(responder.gizmo_restore, None);
    }

    #[test]
    fn process_screenshot_with_delay_does_not_capture_yet() {
        let mut world = world_with_gizmos();
        let (tx, _rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx: tx,
                frames_remaining: 2,
                with_gizmos: false,
                max_width: None,
                debug_camera: None,
                hide_ui: false,
                extra_response: None,
            }],
        });

        process_pending_screenshots(&mut world);

        // Should not have spawned a Screenshot yet
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 0);
        // Gizmos should still be enabled (not toggled yet)
        assert!(gizmos_enabled(&world));
        // Request should still be pending with decremented count
        let pending = world.resource::<PendingScreenshots>();
        assert_eq!(pending.pending.len(), 1);
        assert_eq!(pending.pending[0].frames_remaining, 1);
    }

    #[test]
    fn process_screenshot_debug_camera_stages_with_gizmo_flag() {
        let mut world = world_with_gizmos();
        // Need a Camera3d for setup_debug_camera to reuse
        world.spawn((Camera3d::default(), Transform::default()));

        let (tx, _rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                with_gizmos: true,
                max_width: Some(512),
                debug_camera: Some(DebugCameraRequest {
                    position: [10.0, 5.0, 0.0],
                    look_at: [0.0, 0.0, 0.0],
                }),
                hide_ui: false,
                extra_response: None,
            }],
        });

        process_pending_screenshots(&mut world);

        // Debug camera path stages the screenshot (extra 2 frame delay)
        // so no Screenshot entity yet, and gizmos untouched
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 0);
        assert!(gizmos_enabled(&world));

        // Staged screenshot should exist with with_gizmos preserved
        let staged = world.resource::<StagedDebugScreenshots>();
        assert_eq!(staged.pending.len(), 1);
        assert!(staged.pending[0].with_gizmos);
        assert_eq!(staged.pending[0].max_width, Some(512));
    }

    #[test]
    fn process_staged_debug_screenshot_toggles_gizmos_on_capture() {
        let mut world = world_with_gizmos();
        // Spawn a PrimaryWindow entity so the windowed screenshot path is taken
        world.spawn(PrimaryWindow);

        // Directly insert a staged screenshot (simulating debug camera already set up)
        let (tx, _rx) = oneshot::channel();
        let cleanup = DebugCameraCleanup {
            debug_entity: world
                .spawn((Camera3d::default(), Transform::default()))
                .id(),
            reused_state: None,
            original_cameras: vec![],
        };
        world.insert_resource(StagedDebugScreenshots {
            pending: vec![StagedDebugScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                with_gizmos: false, // Normal screenshot via debug camera
                max_width: None,
                debug_cleanup: cleanup,
                ui_restore: None,
                extra_response: None,
            }],
        });
        // Need PendingScreenshots resource for the function to proceed
        world.insert_resource(PendingScreenshots::default());

        process_pending_screenshots(&mut world);

        // Gizmos should be disabled (non-gizmo screenshot)
        assert!(!gizmos_enabled(&world));
        // Screenshot entity should have been spawned
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 1);
    }

    #[test]
    fn hsv_to_rgb_red() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), Rgb([255, 0, 0]));
    }

    #[test]
    fn hsv_to_rgb_green() {
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), Rgb([0, 255, 0]));
    }

    #[test]
    fn hsv_to_rgb_blue() {
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), Rgb([0, 0, 255]));
    }

    #[test]
    fn hsv_to_rgb_white() {
        assert_eq!(hsv_to_rgb(0.0, 0.0, 1.0), Rgb([255, 255, 255]));
    }

    #[test]
    fn hsv_to_rgb_black() {
        assert_eq!(hsv_to_rgb(0.0, 0.0, 0.0), Rgb([0, 0, 0]));
    }

    #[test]
    fn hsv_to_rgb_half_saturation() {
        let color = hsv_to_rgb(0.0, 0.5, 1.0);
        assert_eq!(color.0[0], 255);
        assert!((color.0[1] as i16 - 128).abs() <= 1);
        assert!((color.0[2] as i16 - 128).abs() <= 1);
    }

    #[test]
    fn composite_contact_sheet_single_frame() {
        let img = RgbImage::from_pixel(4, 4, Rgb([100, 100, 100]));
        let (tx, _rx) = oneshot::channel();
        let mut timeline = ActiveTimeline {
            response_tx: Some(tx),
            max_width: None,
            columns: 3,
            debug_cleanup: None,
            schedule: VecDeque::new(),
            total_captures: 1,
            next_capture_index: 0,
            collected: vec![(0, img)],
            overlay_suppressed: false,
        };

        let result = composite_contact_sheet(&mut timeline);
        assert!(result.is_ok());
        let val = result.unwrap();
        assert!(val.get("image").is_some());
        assert!(val["image"].as_str().unwrap().len() > 0);
        assert!(val.get("width").is_some());
        assert!(val.get("height").is_some());
        assert_eq!(val["format"], "png");
    }

    #[test]
    fn composite_contact_sheet_grid_layout() {
        let mut collected = Vec::new();
        for i in 0..4u32 {
            collected.push((i, RgbImage::from_pixel(4, 4, Rgb([100, 100, 100]))));
        }
        let (tx, _rx) = oneshot::channel();
        let mut timeline = ActiveTimeline {
            response_tx: Some(tx),
            max_width: None,
            columns: 2,
            debug_cleanup: None,
            schedule: VecDeque::new(),
            total_captures: 4,
            next_capture_index: 0,
            collected,
            overlay_suppressed: false,
        };

        let result = composite_contact_sheet(&mut timeline).unwrap();
        let bar_height = 4u32;
        // 2 columns, 4 images → 2 rows
        assert_eq!(result["width"], 2 * 4); // cols * cell_w
        assert_eq!(result["height"], (4 + bar_height) * 2); // (cell_h + bar) * rows
    }

    #[test]
    fn composite_contact_sheet_empty_captures() {
        let (tx, _rx) = oneshot::channel();
        let mut timeline = ActiveTimeline {
            response_tx: Some(tx),
            max_width: None,
            columns: 3,
            debug_cleanup: None,
            schedule: VecDeque::new(),
            total_captures: 0,
            next_capture_index: 0,
            collected: vec![],
            overlay_suppressed: false,
        };

        let result = composite_contact_sheet(&mut timeline);
        assert!(result.is_err());
    }

    #[test]
    fn composite_contact_sheet_max_width_resize() {
        let img = RgbImage::from_pixel(100, 100, Rgb([100, 100, 100]));
        let (tx, _rx) = oneshot::channel();
        let mut timeline = ActiveTimeline {
            response_tx: Some(tx),
            max_width: Some(50),
            columns: 3,
            debug_cleanup: None,
            schedule: VecDeque::new(),
            total_captures: 1,
            next_capture_index: 0,
            collected: vec![(0, img)],
            overlay_suppressed: false,
        };

        let result = composite_contact_sheet(&mut timeline).unwrap();
        assert_eq!(result["width"], 50);
    }

    #[test]
    fn hide_ui_nodes_empty_world() {
        let mut world = World::new();
        let result = hide_ui_nodes(&mut world);
        assert!(result.is_empty());
    }

    #[test]
    fn hide_ui_nodes_hides_nodes() {
        let mut world = World::new();
        let entity = world.spawn((Visibility::Visible, Node::default())).id();

        let hidden = hide_ui_nodes(&mut world);
        assert_eq!(hidden.len(), 1);
        assert_eq!(hidden[0].0, entity);
        assert_eq!(hidden[0].1, Visibility::Visible);

        let vis = world.get::<Visibility>(entity).unwrap();
        assert_eq!(*vis, Visibility::Hidden);
    }

    #[test]
    fn suppress_internal_overlay_counts_without_entities() {
        let mut world = World::new();
        suppress_internal_overlay(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
        release_internal_overlay(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
        // Releasing below zero saturates instead of underflowing
        release_internal_overlay(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
    }

    #[test]
    fn suppress_internal_overlay_force_hides_and_refcounts() {
        let mut world = World::new();
        let entity = world.spawn((Visibility::Visible, InternalOverlayUi)).id();

        // Two overlapping captures compose via the refcount
        suppress_internal_overlay(&mut world);
        suppress_internal_overlay(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 2);
        assert_eq!(
            *world.get::<Visibility>(entity).unwrap(),
            Visibility::Hidden
        );

        release_internal_overlay(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
        release_internal_overlay(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
    }

    /// The internal overlay must be suppressed when a timeline capture fires.
    /// Previously only capture_screenshot hid it; capture_timeline burned
    /// the debug overlay into every contact-sheet frame.
    #[test]
    fn timeline_capture_suppresses_internal_overlay() {
        let mut world = World::new();
        let overlay = world.spawn((Visibility::Visible, InternalOverlayUi)).id();

        let (tx, _rx) = oneshot::channel();
        let mut pending = PendingTimelines::default();
        pending.active.insert(
            0,
            ActiveTimeline {
                response_tx: Some(tx),
                max_width: None,
                columns: 2,
                debug_cleanup: None,
                schedule: VecDeque::from([0]), // capture on this frame
                total_captures: 1,
                next_capture_index: 0,
                collected: vec![],
                overlay_suppressed: false,
            },
        );
        world.insert_resource(pending);

        process_pending_timelines(&mut world);

        assert_eq!(
            *world.get::<Visibility>(overlay).unwrap(),
            Visibility::Hidden,
            "overlay must be hidden while the timeline captures"
        );
        assert_eq!(
            world.resource::<OverlaySuppression>().0,
            1,
            "timeline must hold one suppression refcount"
        );
        assert!(
            world.resource::<PendingTimelines>().active[&0].overlay_suppressed,
            "timeline must record its hold so completion releases exactly once"
        );

        // A second tick of the same timeline must not double-count
        process_pending_timelines(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
    }

    #[test]
    fn suppress_internal_overlay_ignores_non_overlay() {
        let mut world = World::new();
        let plain = world.spawn(Visibility::Visible).id();

        suppress_internal_overlay(&mut world);
        assert_eq!(
            *world.get::<Visibility>(plain).unwrap(),
            Visibility::Visible
        );
    }

    #[test]
    fn process_pending_timelines_no_resource() {
        let mut world = World::new();
        // Should not panic when PendingTimelines resource is absent
        process_pending_timelines(&mut world);
    }

    #[test]
    fn process_pending_timelines_empty() {
        let mut world = World::new();
        world.insert_resource(PendingTimelines::default());

        process_pending_timelines(&mut world);

        let timelines = world.resource::<PendingTimelines>();
        assert!(timelines.active.is_empty());
    }

    #[test]
    fn process_pending_timelines_countdown() {
        let mut world = World::new();
        let (tx, _rx) = oneshot::channel();
        let mut timelines = PendingTimelines::default();
        timelines.active.insert(
            0,
            ActiveTimeline {
                response_tx: Some(tx),
                max_width: None,
                columns: 3,
                debug_cleanup: None,
                schedule: VecDeque::from([2]),
                total_captures: 1,
                next_capture_index: 0,
                collected: vec![],
                overlay_suppressed: false,
            },
        );
        timelines.next_id = 1;
        world.insert_resource(timelines);

        process_pending_timelines(&mut world);

        let timelines = world.resource::<PendingTimelines>();
        let timeline = timelines.active.get(&0).unwrap();
        // Front should be decremented from 2 to 1
        assert_eq!(*timeline.schedule.front().unwrap(), 1);
        // No screenshot should have been spawned
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 0);
    }

    fn make_test_image(width: u32, height: u32) -> Image {
        let size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let data = vec![255u8; (width * height * 4) as usize];
        Image::new(
            size,
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }

    #[test]
    fn encode_screenshot_basic() {
        let img = make_test_image(100, 80);
        let result = encode_screenshot(img, None).unwrap();
        assert_eq!(result["width"], 100);
        assert_eq!(result["height"], 80);
        assert_eq!(result["format"], "png");
        assert_eq!(result["encoding"], "base64");
        assert!(result["image"].as_str().unwrap().len() > 0);
    }

    #[test]
    fn encode_screenshot_max_width_no_resize_needed() {
        let img = make_test_image(100, 80);
        let result = encode_screenshot(img, Some(200)).unwrap();
        // Image is 100px wide, max is 200, so no resize
        assert_eq!(result["width"], 100);
        assert_eq!(result["height"], 80);
    }

    #[test]
    fn encode_screenshot_max_width_resize() {
        let img = make_test_image(200, 100);
        let result = encode_screenshot(img, Some(100)).unwrap();
        assert_eq!(result["width"], 100);
        assert_eq!(result["height"], 50); // proportional
    }

    #[test]
    fn encode_screenshot_produces_valid_base64() {
        let img = make_test_image(10, 10);
        let result = encode_screenshot(img, None).unwrap();
        let b64 = result["image"].as_str().unwrap();
        let decoded = base64::engine::general_purpose::STANDARD.decode(b64);
        assert!(decoded.is_ok());
        // Should be a valid PNG
        let bytes = decoded.unwrap();
        assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn encode_screenshot_small_1x1() {
        let img = make_test_image(1, 1);
        let result = encode_screenshot(img, None).unwrap();
        assert_eq!(result["width"], 1);
        assert_eq!(result["height"], 1);
    }

    #[test]
    fn merge_extra_response_none_passes_through() {
        let result = Ok(serde_json::json!({"image": "abc", "width": 10, "height": 20}));
        let merged = merge_extra_response(result, None).unwrap();
        assert_eq!(merged["image"], "abc");
        assert_eq!(merged["width"], 10);
    }

    #[test]
    fn merge_extra_response_ok_merges_fields() {
        let result = Ok(serde_json::json!({"image": "abc", "width": 10, "height": 20}));
        let extra = Some(serde_json::json!({"depth_samples": {"hit_count": 3}, "custom": true}));
        let merged = merge_extra_response(result, extra).unwrap();
        assert_eq!(merged["screenshot"], "abc");
        assert_eq!(merged["screenshot_width"], 10);
        assert_eq!(merged["screenshot_height"], 20);
        assert_eq!(merged["depth_samples"]["hit_count"], 3);
        assert_eq!(merged["custom"], true);
    }

    #[test]
    fn merge_extra_response_err_returns_extra_with_null_screenshot() {
        let result = Err(ControlError::not_found("fail"));
        let extra = Some(serde_json::json!({"reload": "ok", "entity_count": 42}));
        let merged = merge_extra_response(result, extra).unwrap();
        assert!(merged["screenshot"].is_null());
        assert_eq!(merged["reload"], "ok");
        assert_eq!(merged["entity_count"], 42);
    }

    #[test]
    fn process_pending_timelines_captures_at_zero_delay() {
        let mut world = World::new();
        world.spawn(PrimaryWindow);
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let mut timelines = PendingTimelines::default();
        timelines.active.insert(
            0,
            ActiveTimeline {
                response_tx: Some(tx),
                max_width: None,
                columns: 3,
                debug_cleanup: None,
                schedule: VecDeque::from([0]), // ready immediately
                total_captures: 1,
                next_capture_index: 0,
                collected: vec![],
                overlay_suppressed: false,
            },
        );
        timelines.next_id = 1;
        world.insert_resource(timelines);

        process_pending_timelines(&mut world);

        // A Screenshot entity should have been spawned
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 1);

        // next_capture_index should have advanced
        let tl = world.resource::<PendingTimelines>();
        let timeline = tl.active.get(&0).unwrap();
        assert_eq!(timeline.next_capture_index, 1);
        assert!(timeline.schedule.is_empty());
    }

    #[test]
    fn process_pending_timelines_multiple_delays() {
        let mut world = World::new();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let mut timelines = PendingTimelines::default();
        timelines.active.insert(
            0,
            ActiveTimeline {
                response_tx: Some(tx),
                max_width: None,
                columns: 3,
                debug_cleanup: None,
                schedule: VecDeque::from([0, 10, 10]), // first ready, then 2 more
                total_captures: 3,
                next_capture_index: 0,
                collected: vec![],
                overlay_suppressed: false,
            },
        );
        timelines.next_id = 1;
        world.insert_resource(timelines);

        process_pending_timelines(&mut world);

        // First capture should be spawned
        let tl = world.resource::<PendingTimelines>();
        let timeline = tl.active.get(&0).unwrap();
        assert_eq!(timeline.next_capture_index, 1);
        // schedule should now be [10, 10] (first 0 popped)
        assert_eq!(timeline.schedule.len(), 2);
        assert_eq!(timeline.schedule[0], 10);
    }

    #[test]
    fn compute_schedule_large_frame_count() {
        let schedule = compute_schedule(1000, 11);
        assert_eq!(schedule.len(), 11);
        let total: u32 = schedule.iter().sum();
        assert_eq!(total, 1000);
    }

    #[test]
    fn compute_schedule_zero_captures_treated_as_one() {
        // capture_count of 0 would cause issues, but the function
        // handles capture_count <= 1 by returning [0]
        let schedule = compute_schedule(100, 0);
        assert_eq!(schedule.len(), 1);
        assert_eq!(schedule[0], 0);
    }

    #[test]
    fn headless_frame_buffer_default_is_none() {
        let buffer = HeadlessFrameBuffer::default();
        assert!(buffer.latest.is_none());
    }

    #[test]
    fn encode_rgb_screenshot_produces_valid_png() {
        let mut rgb = RgbImage::new(4, 4);
        for x in 0..4 {
            for y in 0..4 {
                rgb.put_pixel(x, y, Rgb([100, 150, 200]));
            }
        }
        let result = encode_rgb_screenshot(rgb, None).unwrap();
        assert_eq!(result["format"], "png");
        assert_eq!(result["width"], 4);
        assert_eq!(result["height"], 4);
        assert!(result["image"].as_str().unwrap().len() > 10);
    }

    #[test]
    fn encode_rgb_screenshot_resizes() {
        let rgb = RgbImage::new(100, 50);
        let result = encode_rgb_screenshot(rgb, Some(20)).unwrap();
        assert_eq!(result["width"], 20);
        assert_eq!(result["height"], 10);
    }

    #[test]
    fn capture_headless_frame_no_buffer() {
        let mut world = World::new();
        let result = capture_headless_frame(&mut world, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("HeadlessFrameBuffer"));
    }

    #[test]
    fn capture_headless_frame_no_frame_available() {
        let mut world = World::new();
        world.insert_resource(HeadlessFrameBuffer::default());
        let result = capture_headless_frame(&mut world, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("no frame"));
    }

    #[test]
    fn capture_headless_frame_success() {
        let mut world = World::new();
        let rgba = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ];
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((rgba, 2, 2)),
        });
        let result = capture_headless_frame(&mut world, None).unwrap();
        assert_eq!(result["width"], 2);
        assert_eq!(result["height"], 2);
        assert_eq!(result["format"], "png");
    }

    #[test]
    fn read_headless_frame_converts_rgba_to_rgb() {
        let mut world = World::new();
        // 1x1 pixel: R=100, G=200, B=50, A=255
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![100, 200, 50, 255], 1, 1)),
        });
        let rgb = read_headless_frame(&world).unwrap();
        assert_eq!(rgb.width(), 1);
        assert_eq!(rgb.height(), 1);
        let pixel = rgb.get_pixel(0, 0);
        assert_eq!(pixel.0, [100, 200, 50]);
    }
}
