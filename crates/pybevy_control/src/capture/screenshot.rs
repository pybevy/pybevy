use std::{collections::HashMap, io::Cursor, mem, sync::Arc};

use base64::Engine;
use bevy::{
    camera::RenderTarget,
    ecs::world::World,
    prelude::*,
    render::view::window::screenshot::{Screenshot, ScreenshotCaptured},
    window::PrimaryWindow,
};
use image::{ImageFormat, RgbImage};
use pybevy_ecs::shared::system_runtime::HotReloadGeneration;
use tokio::sync::oneshot;

use super::{
    camera::{
        selected_camera_screenshot_target, setup_debug_camera_for_entity, validate_capture_camera,
    },
    compose::composite_contact_sheet,
    headless::{capture_headless_frame, headless_frame_sequence},
    hold::{CaptureHold, hide_ui_nodes, prepare_pending_screenshot_gizmos, set_gizmos_enabled},
    isolation::{ENTITY_ISOLATION_SETTLE_FRAMES, begin_entity_capture_isolation},
    lease::capture_in_progress,
    readiness::RenderFrameReadiness,
    timeline::{PendingTimelines, TimelineCaptures},
};
use crate::{
    capture::{
        analysis::{CapturedFrameMetadata, CapturedFrames, analyze_frame, resize_rgb_image_linear},
        depth::{
            CaptureViewport, PendingDepthAnalysis, prepare_depth_capture,
            validate_depth_capture_camera,
        },
        queue::{CaptureResponseKind, PendingScreenshots},
    },
    protocol::ControlError,
};

/// Frames a queued capture may wait for its readback before it is failed and
/// its visibility/camera state restored, so a capture that never completes
/// cannot wedge the capture lease or hang its request forever.
pub(crate) const MAX_CAPTURE_WAIT_FRAMES: u32 = 600;

pub(crate) const CAPTURE_DEADLINE_ERROR: &str =
    "capture did not complete within the deadline (render readback never arrived)";
pub(crate) const CAPTURE_CANCELLED_ERROR: &str = "capture cancelled: the requester is gone";
pub(crate) const STALE_HEADLESS_FRAME_ERROR: &str = "capture did not receive a fresh headless frame before the deadline; the renderer may have stopped";

/// Per-screenshot responder stored until the observer fires.
pub struct ScreenshotResponder {
    pub response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    pub max_width: Option<u32>,
    pub hold: CaptureHold,
    /// Extra JSON fields to merge into the screenshot response.
    pub extra_response: Option<serde_json::Value>,
    pub(crate) capture_viewport: Option<CaptureViewport>,
    pub response_kind: CaptureResponseKind,
    pub frames_waited: u32,
}

/// Resource mapping screenshot Entity → responder info.
#[derive(Resource, Default)]
pub struct PendingScreenshotResponders {
    pub map: HashMap<Entity, ScreenshotResponder>,
}

/// Staged debug screenshots waiting for the debug camera to render before capture.
#[derive(Resource, Default)]
pub(super) struct StagedScreenshots {
    pub(super) pending: Vec<StagedScreenshot>,
}

pub(super) struct StagedScreenshot {
    pub(super) response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    pub(super) frames_remaining: u32,
    pub(super) with_gizmos: bool,
    pub(super) max_width: Option<u32>,
    pub(super) hold: CaptureHold,
    pub(super) depth_analysis: Option<PendingDepthAnalysis>,
    pub(super) extra_response: Option<serde_json::Value>,
    pub(super) response_kind: CaptureResponseKind,
    pub(super) baseline_headless_sequence: Option<u64>,
    pub(super) frames_waited: u32,
}

/// Upper bound on `delay_frames` for every capture tool.
pub const MAX_CAPTURE_DELAY_FRAMES: u32 = 600;
pub(crate) fn validate_capture_delay(delay_frames: u32) -> Result<(), ControlError> {
    if delay_frames > MAX_CAPTURE_DELAY_FRAMES {
        return Err(ControlError::invalid_params(format!(
            "delay_frames must be at most {MAX_CAPTURE_DELAY_FRAMES}"
        )));
    }
    Ok(())
}

/// Process pending screenshot requests (called each frame in Last schedule).
///
/// Flow:
/// 1. Count down `frames_remaining` on normal pending screenshots
/// 2. When ready: if `debug_camera` is set, set up the debug camera and stage with extra delay
/// 3. Count down staged debug screenshots
/// 4. When staged screenshot is ready, spawn the Screenshot entity for capture
pub fn process_pending_screenshots(world: &mut World) {
    expire_stuck_screenshot_responders(world);

    let Some(mut pending) = world.remove_resource::<PendingScreenshots>() else {
        return;
    };

    if pending.pending.is_empty()
        && world
            .get_resource::<StagedScreenshots>()
            .is_none_or(|s| s.pending.is_empty())
    {
        world.insert_resource(pending);
        return;
    }

    let mut remaining = Vec::new();
    let mut ready = Vec::new();

    for mut screenshot in pending.pending.drain(..) {
        if screenshot.response_tx.is_closed() {
            CaptureHold::with_gizmo_restore(screenshot.gizmo_restore).release(world);
            continue;
        }
        prepare_pending_screenshot_gizmos(world, &mut screenshot);
        if screenshot.required_render_epoch.is_some_and(|epoch| {
            world
                .get_resource::<RenderFrameReadiness>()
                .is_some_and(|readiness| !readiness.is_complete(epoch))
        }) {
            remaining.push(screenshot);
            continue;
        }
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
        if capture_in_progress(world) {
            world
                .resource_mut::<PendingScreenshots>()
                .pending
                .push(screenshot);
            continue;
        }

        let mut hold = CaptureHold::with_gizmo_restore(mem::take(&mut screenshot.gizmo_restore));
        let selected_depth_camera = screenshot
            .depth_analysis
            .as_ref()
            .map(|analysis| analysis.camera_entity);
        if let Some(camera_entity) = selected_depth_camera
            && let Err(error) = validate_depth_capture_camera(world, camera_entity)
        {
            let _ = screenshot.response_tx.send(Err(error));
            hold.release(world);
            continue;
        }
        if selected_depth_camera.is_none()
            && screenshot.debug_camera.is_none()
            && let Err(error) = validate_capture_camera(world, &screenshot.response_kind)
        {
            let _ = screenshot.response_tx.send(Err(error));
            hold.release(world);
            continue;
        }

        if let Some(debug_req) = screenshot.debug_camera.take() {
            match setup_debug_camera_for_entity(world, &debug_req, selected_depth_camera) {
                Ok(cleanup) => hold.debug_cleanup = Some(cleanup),
                Err(error) => {
                    let _ = screenshot.response_tx.send(Err(error));
                    hold.release(world);
                    continue;
                }
            }
        }

        if let Some(entity_ref) = screenshot.entity.take() {
            match begin_entity_capture_isolation(world, &entity_ref, screenshot.with_gizmos) {
                Ok(isolation) => hold.entity_isolation = Some(isolation),
                Err(error) => {
                    let _ = screenshot.response_tx.send(Err(error));
                    hold.release(world);
                    continue;
                }
            }
        }

        hold.begin_capture(world);
        if screenshot.hide_ui {
            let keep_visible = hold
                .entity_isolation
                .as_ref()
                .map(|isolation| &isolation.scope);
            hold.ui_restore = Some(hide_ui_nodes(world, keep_visible));
        }
        let isolated = hold.entity_isolation.is_some();
        let settle_frames = if isolated {
            ENTITY_ISOLATION_SETTLE_FRAMES
        } else {
            2
        };

        let has_window = world
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .iter(world)
            .next()
            .is_some();
        // An isolated capture always stages: capturing in this same run
        // would grab a frame laid out before the isolation.
        if hold.debug_cleanup.is_some() || !has_window || isolated {
            let baseline_headless_sequence = headless_frame_sequence(world);
            let mut staged = world.get_resource_or_insert_with(StagedScreenshots::default);
            staged.pending.push(StagedScreenshot {
                response_tx: screenshot.response_tx,
                frames_remaining: settle_frames,
                with_gizmos: screenshot.with_gizmos,
                max_width: screenshot.max_width,
                hold,
                depth_analysis: screenshot.depth_analysis.take(),
                extra_response: screenshot.extra_response,
                response_kind: screenshot.response_kind,
                baseline_headless_sequence,
                frames_waited: 0,
            });
            continue;
        }

        if hold.gizmo_restore.is_empty() && !screenshot.with_gizmos {
            hold.gizmo_restore = set_gizmos_enabled(world, false);
        }
        let (target, capture_viewport) = if let Some(depth_analysis) =
            screenshot.depth_analysis.as_ref()
        {
            match prepare_depth_capture(world, depth_analysis) {
                Ok((target, viewport, depth)) => {
                    screenshot.extra_response = Some(serde_json::json!({ "depth_samples": depth }));
                    (Screenshot(target), viewport)
                }
                Err(error) => {
                    let _ = screenshot.response_tx.send(Err(error));
                    hold.release(world);
                    continue;
                }
            }
        } else {
            (Screenshot::primary_window(), None)
        };
        let entity = world.spawn(target).id();

        let mut responders =
            world.get_resource_or_insert_with(PendingScreenshotResponders::default);
        responders.map.insert(
            entity,
            ScreenshotResponder {
                response_tx: screenshot.response_tx,
                max_width: screenshot.max_width,
                hold,
                extra_response: screenshot.extra_response,
                capture_viewport,
                response_kind: screenshot.response_kind,
                frames_waited: 0,
            },
        );
    }

    // Process staged debug screenshots
    if let Some(mut staged) = world.remove_resource::<StagedScreenshots>() {
        let mut still_waiting = Vec::new();

        for mut s in staged.pending.drain(..) {
            if s.response_tx.is_closed() {
                fail_staged_screenshot(world, s, CAPTURE_CANCELLED_ERROR);
                continue;
            }
            if s.frames_remaining > 0 {
                s.frames_remaining -= 1;
                still_waiting.push(s);
            } else {
                // Debug camera has rendered - capture now
                if s.depth_analysis.is_none()
                    && let Err(error) = validate_capture_camera(world, &s.response_kind)
                {
                    fail_staged_screenshot_with_error(world, s, error);
                    continue;
                }
                let has_window = world
                    .query_filtered::<Entity, With<PrimaryWindow>>()
                    .iter(world)
                    .next()
                    .is_some();

                let (capture_target, capture_viewport) = if let Some(depth_analysis) =
                    s.depth_analysis.as_ref()
                {
                    match prepare_depth_capture(world, depth_analysis) {
                        Ok((target, viewport, depth)) => {
                            s.extra_response = Some(serde_json::json!({ "depth_samples": depth }));
                            (Some(target), viewport)
                        }
                        Err(error) => {
                            fail_staged_screenshot_with_error(world, s, error);
                            continue;
                        }
                    }
                } else {
                    match selected_camera_screenshot_target(world, &s.response_kind) {
                        Ok(Some((target, viewport))) => (Some(target), viewport),
                        Ok(None) => (None, None),
                        Err(error) => {
                            fail_staged_screenshot_with_error(world, s, error);
                            continue;
                        }
                    }
                };

                let screenshot_target = if let Some(target) = capture_target.as_ref() {
                    Some(Screenshot(target.clone()))
                } else if has_window {
                    Some(Screenshot::primary_window())
                } else {
                    s.hold.debug_cleanup.as_ref().and_then(|cleanup| {
                        match world.get::<RenderTarget>(cleanup.debug_entity) {
                            Some(target @ RenderTarget::Image(_)) => {
                                Some(Screenshot(target.clone()))
                            }
                            _ => None,
                        }
                    })
                };

                if screenshot_target.is_none()
                    && s.baseline_headless_sequence.is_some_and(|baseline| {
                        headless_frame_sequence(world).is_none_or(|current| current <= baseline)
                    })
                {
                    s.frames_waited += 1;
                    if s.frames_waited > MAX_CAPTURE_WAIT_FRAMES {
                        fail_staged_screenshot(world, s, STALE_HEADLESS_FRAME_ERROR);
                    } else {
                        still_waiting.push(s);
                    }
                    continue;
                }

                if let Some(screenshot) = screenshot_target {
                    if s.hold.gizmo_restore.is_empty() && !s.with_gizmos {
                        s.hold.gizmo_restore = set_gizmos_enabled(world, false);
                    }

                    // Capture a newly rendered target, not readback from before view culling.
                    let entity = world.spawn(screenshot).id();

                    let mut responders =
                        world.get_resource_or_insert_with(PendingScreenshotResponders::default);
                    responders.map.insert(
                        entity,
                        ScreenshotResponder {
                            response_tx: s.response_tx,
                            max_width: s.max_width,
                            hold: s.hold,
                            extra_response: s.extra_response,
                            capture_viewport,
                            response_kind: s.response_kind,
                            frames_waited: 0,
                        },
                    );
                } else {
                    // Headless fallback
                    let result = capture_headless_frame(world, s.max_width, s.response_kind);
                    let result = merge_extra_response(result, s.extra_response);
                    let _ = s.response_tx.send(result);
                    s.hold.release(world);
                }
            }
        }

        staged.pending = still_waiting;
        world.insert_resource(staged);
    }
}

fn fail_staged_screenshot(world: &mut World, staged: StagedScreenshot, message: &str) {
    fail_staged_screenshot_with_error(world, staged, ControlError::internal(message.to_string()));
}

fn fail_staged_screenshot_with_error(
    world: &mut World,
    staged: StagedScreenshot,
    error: ControlError,
) {
    let _ = staged.response_tx.send(Err(error));
    staged.hold.release(world);
}

/// Fail responders whose capture readback never arrived, restoring the
/// visibility and camera state they held.
fn expire_stuck_screenshot_responders(world: &mut World) {
    let expired: Vec<(Entity, ScreenshotResponder)> = {
        let Some(mut responders) = world.get_resource_mut::<PendingScreenshotResponders>() else {
            return;
        };
        let stuck: Vec<Entity> = responders
            .map
            .iter_mut()
            .filter_map(|(entity, responder)| {
                responder.frames_waited += 1;
                (responder.frames_waited > MAX_CAPTURE_WAIT_FRAMES
                    || responder.response_tx.is_closed())
                .then_some(*entity)
            })
            .collect();
        stuck
            .into_iter()
            .filter_map(|entity| {
                responders
                    .map
                    .remove(&entity)
                    .map(|responder| (entity, responder))
            })
            .collect()
    };

    for (entity, responder) in expired {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
        let _ = responder
            .response_tx
            .send(Err(ControlError::internal(CAPTURE_DEADLINE_ERROR)));
        responder.hold.release(world);
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
    mut captured_frames: ResMut<CapturedFrames>,
    hot_reload_generation: Option<Res<HotReloadGeneration>>,
    mut commands: Commands,
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
            timeline.stall_frames = 0;

            // Check if all captures collected
            if timeline.collected.len() as u32 == timeline.total_captures {
                let result = composite_contact_sheet(timeline);
                if let Some(tx) = timeline.response_tx.take() {
                    let _ = tx.send(result);
                }
                if let Some(hold) = timeline.hold.take() {
                    hold.queue_release(&mut commands);
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
    let result = complete_screenshot_capture(
        img,
        responder.max_width,
        responder.capture_viewport,
        responder.response_kind,
        &mut captured_frames,
        hot_reload_generation.map(|generation| generation.current),
    );
    let result = merge_extra_response(result, responder.extra_response);
    let _ = responder.response_tx.send(result);
    responder.hold.queue_release(&mut commands);
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
        Ok(screenshot) => {
            let mut response = serde_json::json!({
                "screenshot": screenshot.get("image"),
                "screenshot_width": screenshot.get("width"),
                "screenshot_height": screenshot.get("height"),
            });
            if let Some(response) = response.as_object_mut() {
                for key in ["frame_id", "retained", "retention_reason"] {
                    if let Some(value) = screenshot.get(key) {
                        response.insert(key.to_string(), value.clone());
                    }
                }
            }
            response
        }
        Err(error) => serde_json::json!({
            "screenshot": null,
            "screenshot_error": error.message,
        }),
    };
    if let (Some(base), serde_json::Value::Object(extra_map)) = (merged.as_object_mut(), extra) {
        for (k, v) in extra_map {
            base.insert(k, v);
        }
    }
    Ok(merged)
}

#[cfg(test)]
fn encode_screenshot(
    img: bevy::image::Image,
    max_width: Option<u32>,
) -> Result<serde_json::Value, ControlError> {
    let dyn_img = img.try_into_dynamic().map_err(|e| {
        ControlError::internal(format!("Failed to convert screenshot image: {e:?}"))
    })?;

    // Discard alpha (stores HDR brightness) to get a clean RGB image
    let rgb = resize_rgb_image_linear(dyn_img.to_rgb8(), max_width);
    encode_rgb_screenshot(&rgb)
}

fn complete_screenshot_capture(
    img: bevy::image::Image,
    max_width: Option<u32>,
    capture_viewport: Option<CaptureViewport>,
    response_kind: CaptureResponseKind,
    captured_frames: &mut CapturedFrames,
    hot_reload_generation: Option<u32>,
) -> Result<serde_json::Value, ControlError> {
    let dyn_img = img.try_into_dynamic().map_err(|error| {
        ControlError::internal(format!("Failed to convert screenshot image: {error:?}"))
    })?;
    let rgb = crop_capture_viewport(dyn_img.to_rgb8(), capture_viewport)?;
    complete_rgb_capture(
        rgb,
        max_width,
        response_kind,
        captured_frames,
        hot_reload_generation,
    )
}

fn crop_capture_viewport(
    rgb: RgbImage,
    capture_viewport: Option<CaptureViewport>,
) -> Result<RgbImage, ControlError> {
    let Some(viewport) = capture_viewport else {
        return Ok(rgb);
    };
    let end_x = viewport
        .physical_position
        .x
        .checked_add(viewport.physical_size.x)
        .ok_or_else(|| ControlError::internal("Selected camera viewport exceeds its target"))?;
    let end_y = viewport
        .physical_position
        .y
        .checked_add(viewport.physical_size.y)
        .ok_or_else(|| ControlError::internal("Selected camera viewport exceeds its target"))?;
    if viewport.physical_size.x == 0
        || viewport.physical_size.y == 0
        || end_x > rgb.width()
        || end_y > rgb.height()
    {
        return Err(ControlError::internal(format!(
            "Selected camera viewport [{}, {}] + [{}, {}] exceeds captured target {}x{}",
            viewport.physical_position.x,
            viewport.physical_position.y,
            viewport.physical_size.x,
            viewport.physical_size.y,
            rgb.width(),
            rgb.height(),
        )));
    }
    Ok(image::imageops::crop_imm(
        &rgb,
        viewport.physical_position.x,
        viewport.physical_position.y,
        viewport.physical_size.x,
        viewport.physical_size.y,
    )
    .to_image())
}

pub(super) fn complete_rgb_capture(
    rgb: RgbImage,
    max_width: Option<u32>,
    response_kind: CaptureResponseKind,
    captured_frames: &mut CapturedFrames,
    hot_reload_generation: Option<u32>,
) -> Result<serde_json::Value, ControlError> {
    let rgb = Arc::new(resize_rgb_image_linear(rgb, max_width));
    let (mut result, kind) = match response_kind {
        CaptureResponseKind::Screenshot | CaptureResponseKind::ReloadAndCapture => {
            (encode_rgb_screenshot(&rgb)?, "screenshot")
        }
        CaptureResponseKind::UnretainedScreenshot => return encode_rgb_screenshot(&rgb),
        CaptureResponseKind::Stats(options) => (analyze_frame(&rgb, &options)?, "stats"),
    };
    let retention = captured_frames.retain(
        rgb,
        CapturedFrameMetadata {
            kind,
            max_width,
            hot_reload_generation,
        },
    );
    retention.insert_into(
        result
            .as_object_mut()
            .ok_or_else(|| ControlError::internal("Capture response was not an object"))?,
    );
    Ok(result)
}

/// Encode an RgbImage as a base64 PNG string.
fn encode_rgb_screenshot(rgb: &RgbImage) -> Result<serde_json::Value, ControlError> {
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

#[cfg(test)]
mod tests {

    use bevy::{
        camera::{RenderTarget, Viewport},
        image::Image,
        render::view::window::screenshot::Screenshot,
        ui::Node,
        window::{PrimaryWindow, WindowRef},
    };
    use image::{Rgb, RgbImage};
    use tokio::sync::oneshot;

    use super::*;
    use crate::{
        capture::{
            analysis::FrameStatsOptions,
            camera::{DebugCameraCleanup, setup_debug_camera},
            headless::HeadlessFrameBuffer,
            hold::{GizmoEnabledRestore, OverlaySuppression, suppress_internal_overlay},
            queue::{
                CaptureResponseKind, DebugCameraRequest, PendingScreenshot, PendingScreenshots,
            },
            test_support::{
                default_group_restore, gizmos_enabled, make_test_image,
                spawn_structurally_valid_headless_camera, world_with_gizmos,
            },
        },
        engine::deferred::push_pending_depth,
        protocol::{CaptureDepthParams, EntityRef},
    };

    /// Set up a World with gizmos + PendingScreenshots + a PrimaryWindow entity.
    fn world_with_pending_screenshot(
        with_gizmos: bool,
    ) -> (
        World,
        oneshot::Receiver<Result<serde_json::Value, ControlError>>,
    ) {
        let mut world = world_with_gizmos();
        // Spawn a PrimaryWindow entity so the windowed screenshot path is taken
        world.spawn((Window::default(), PrimaryWindow));
        world.spawn((Camera::default(), RenderTarget::Window(WindowRef::Primary)));
        let (tx, rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                required_render_epoch: None,
                with_gizmos,
                gizmo_restore: GizmoEnabledRestore::new(),
                max_width: None,
                debug_camera: None,
                hide_ui: false,
                entity: None,
                depth_analysis: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });
        (world, rx)
    }

    fn queued_screenshot(
        frames_remaining: u32,
    ) -> (
        PendingScreenshot,
        oneshot::Receiver<Result<serde_json::Value, ControlError>>,
    ) {
        let (tx, rx) = oneshot::channel();
        let screenshot = PendingScreenshot {
            response_tx: tx,
            frames_remaining,
            required_render_epoch: None,
            with_gizmos: false,
            gizmo_restore: GizmoEnabledRestore::new(),
            max_width: None,
            debug_camera: None,
            hide_ui: true,
            entity: None,
            depth_analysis: None,
            extra_response: None,
            response_kind: CaptureResponseKind::Screenshot,
        };
        (screenshot, rx)
    }

    #[test]
    fn delayed_depth_capture_does_not_reselect_another_active_camera() {
        let mut world = world_with_gizmos();
        let mut images = Assets::<Image>::default();
        let selected_target = images.add(make_test_image(3, 2));
        world.insert_resource(images);
        world.spawn((
            Camera3d::default(),
            Camera::default(),
            Projection::default(),
            GlobalTransform::default(),
            Name::new("fallback"),
        ));
        let selected = world
            .spawn((
                Camera3d::default(),
                Camera {
                    order: 10,
                    ..default()
                },
                Projection::default(),
                Transform::default(),
                GlobalTransform::default(),
                RenderTarget::Image(selected_target.into()),
                Name::new("selected"),
            ))
            .id();
        let params: CaptureDepthParams = serde_json::from_str(r#"{"delay_frames": 0}"#).unwrap();
        let (tx, mut rx) = oneshot::channel();
        let mut deferred = Vec::new();
        push_pending_depth(params, tx, &mut deferred, &mut world);
        assert_eq!(
            deferred[0].depth_analysis.as_ref().unwrap().camera_entity,
            selected
        );
        world.insert_resource(PendingScreenshots { pending: deferred });

        process_pending_screenshots(&mut world);
        assert_eq!(world.resource::<StagedScreenshots>().pending.len(), 1);
        world.get_mut::<Camera>(selected).unwrap().is_active = false;
        process_pending_screenshots(&mut world);
        process_pending_screenshots(&mut world);

        let error = rx.try_recv().unwrap().unwrap_err();
        assert_eq!(
            error.message,
            "Selected capture_depth camera became inactive before capture"
        );
        assert!(world.query::<&Screenshot>().iter(&world).next().is_none());
    }

    #[test]
    fn depth_capture_spawns_screenshot_for_the_selected_camera_target() {
        let mut world = world_with_gizmos();
        let mut images = Assets::<Image>::default();
        let first_target = images.add(make_test_image(2, 1));
        let selected_target = images.add(make_test_image(3, 2));
        world.spawn((
            Camera3d::default(),
            Camera::default(),
            Projection::default(),
            GlobalTransform::default(),
            RenderTarget::Image(bevy::camera::ImageRenderTarget::from(first_target)),
            Name::new("first"),
        ));
        let selected = world
            .spawn((
                Camera3d::default(),
                Camera {
                    order: 10,
                    ..default()
                },
                Projection::default(),
                Transform::default(),
                GlobalTransform::default(),
                RenderTarget::Image(bevy::camera::ImageRenderTarget::from(
                    selected_target.clone(),
                )),
                Name::new("selected"),
            ))
            .id();
        world.insert_resource(images);
        let params: CaptureDepthParams = serde_json::from_str(r#"{"delay_frames": 0}"#).unwrap();
        let (tx, mut rx) = oneshot::channel();
        let mut deferred = Vec::new();
        push_pending_depth(params, tx, &mut deferred, &mut world);
        assert_eq!(
            deferred[0].depth_analysis.as_ref().unwrap().camera_entity,
            selected
        );
        world.insert_resource(PendingScreenshots { pending: deferred });

        process_pending_screenshots(&mut world);
        process_pending_screenshots(&mut world);
        process_pending_screenshots(&mut world);

        assert!(
            rx.try_recv().is_err(),
            "response waits for ScreenshotCaptured"
        );
        let mut screenshots = world.query::<&Screenshot>();
        let screenshot = screenshots.single(&world).unwrap();
        let RenderTarget::Image(target) = &screenshot.0 else {
            panic!("depth RGB must capture the selected image target");
        };
        assert_eq!(target.handle, selected_target);
        assert_eq!(world.resource::<PendingScreenshotResponders>().map.len(), 1);
    }

    #[test]
    fn staged_depth_capture_uses_camera_context_at_screenshot_spawn() {
        let mut world = world_with_gizmos();
        let mut images = Assets::<Image>::default();
        let initial_target = images.add(make_test_image(4, 3));
        let ready_target = images.add(make_test_image(8, 6));
        world.insert_resource(images);
        world.spawn((
            bevy::camera::primitives::Aabb::from_min_max(
                Vec3::new(-100.0, -100.0, -1.0),
                Vec3::new(100.0, 100.0, 1.0),
            ),
            GlobalTransform::default(),
            Name::new("depth-plane"),
        ));
        let initial_transform = Transform::from_xyz(0.0, 0.0, 10.0);
        let selected = world
            .spawn((
                Camera3d::default(),
                Camera {
                    order: 10,
                    ..default()
                },
                Projection::Perspective(PerspectiveProjection {
                    fov: 0.2,
                    aspect_ratio: 1.0,
                    ..default()
                }),
                initial_transform,
                GlobalTransform::from(initial_transform),
                RenderTarget::Image(initial_target.into()),
                Name::new("selected"),
            ))
            .id();
        let params: CaptureDepthParams =
            serde_json::from_str(r#"{"delay_frames":0,"sample_points":[[600,400]]}"#).unwrap();
        let (tx, _rx) = oneshot::channel();
        let mut deferred = Vec::new();
        push_pending_depth(params, tx, &mut deferred, &mut world);
        world.insert_resource(PendingScreenshots { pending: deferred });

        process_pending_screenshots(&mut world);
        assert_eq!(world.resource::<StagedScreenshots>().pending.len(), 1);

        let ready_transform = Transform::from_xyz(0.0, 0.0, 20.0);
        world.entity_mut(selected).insert((
            Camera {
                viewport: Some(Viewport {
                    physical_position: UVec2::new(1, 1),
                    physical_size: UVec2::new(6, 4),
                    ..default()
                }),
                order: 12,
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                fov: 2.0,
                aspect_ratio: 1.0,
                ..default()
            }),
            ready_transform,
            GlobalTransform::from(ready_transform),
            RenderTarget::Image(ready_target.clone().into()),
        ));

        process_pending_screenshots(&mut world);
        process_pending_screenshots(&mut world);

        let mut screenshots = world.query::<(Entity, &Screenshot)>();
        let (screenshot_entity, screenshot) = screenshots.single(&world).unwrap();
        let RenderTarget::Image(target) = &screenshot.0 else {
            panic!("depth RGB must capture the ready-time image target");
        };
        assert_eq!(target.handle, ready_target);
        let responders = world.resource::<PendingScreenshotResponders>();
        let responder = responders.map.get(&screenshot_entity).unwrap();
        assert_eq!(
            responder.capture_viewport.unwrap().physical_size,
            UVec2::new(6, 4)
        );
        let depth = &responder.extra_response.as_ref().unwrap()["depth_samples"];
        assert_eq!(
            depth["camera_position"],
            serde_json::json!([0.0, 0.0, 20.0])
        );
        assert_eq!(depth["camera_order"], 12);
        assert_eq!(
            depth["viewport"],
            serde_json::json!({
                "physical_position": [1, 1],
                "physical_size": [6, 4],
            })
        );
        assert!(
            depth["samples"][0]["world_position"][0].as_f64().unwrap() > 10.0,
            "the ready-time wide projection must determine the depth ray"
        );
    }

    #[test]
    fn rejected_camera2d_override_restores_capture_state() {
        let mut world = world_with_gizmos();
        let camera = world.spawn(Camera2d).id();
        let ui = world.spawn((Visibility::Visible, Node::default())).id();
        let (response_tx, mut response_rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![crate::capture::queue::PendingScreenshot {
                response_tx,
                frames_remaining: 0,
                required_render_epoch: None,
                with_gizmos: false,
                gizmo_restore: GizmoEnabledRestore::new(),
                max_width: None,
                debug_camera: Some(DebugCameraRequest {
                    position: [0.0, 0.0, 500.0],
                    look_at: [0.0, 0.0, 0.0],
                }),
                hide_ui: true,
                entity: None,
                depth_analysis: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });

        process_pending_screenshots(&mut world);

        let error = response_rx.try_recv().unwrap().unwrap_err();
        assert!(error.message.contains("require a Camera3d"));
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Visible);
        assert!(world.get::<Camera>(camera).unwrap().is_active);
        assert!(
            world
                .get_resource::<OverlaySuppression>()
                .is_none_or(|suppression| suppression.0 == 0)
        );
    }

    #[test]
    fn headless_debug_capture_waits_for_its_image_and_restores_on_timeout() {
        let mut world = world_with_gizmos();
        let original = Transform::from_xyz(0.0, 0.0, 5.0);
        let image = Handle::<Image>::default();
        let target = RenderTarget::Image(image.clone().into());
        let camera = world
            .spawn((
                Camera3d::default(),
                original,
                GlobalTransform::from(original),
                target.clone(),
            ))
            .id();
        let cleanup = setup_debug_camera(
            &mut world,
            &DebugCameraRequest {
                position: [10.0, 0.0, 5.0],
                look_at: [10.0, 0.0, 0.0],
            },
        )
        .unwrap();
        let (response_tx, mut response_rx) = oneshot::channel();
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
                baseline_headless_sequence: Some(5),
                frames_waited: 0,
            }],
        });
        world.insert_resource(PendingScreenshots::default());
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![0; 4], 1, 1)),
            sequence: 6,
        });
        suppress_internal_overlay(&mut world);

        process_pending_screenshots(&mut world);

        assert!(response_rx.try_recv().is_err());
        assert!(world.resource::<StagedScreenshots>().pending.is_empty());
        let mut screenshots = world.query::<(Entity, &Screenshot)>();
        let (screenshot_entity, screenshot) = screenshots.single(&world).unwrap();
        let RenderTarget::Image(captured_target) = &screenshot.0 else {
            panic!("temporary image view must capture its render target");
        };
        assert_eq!(captured_target.handle, image);
        assert_eq!(
            world.get::<Transform>(camera).unwrap().translation,
            Vec3::new(10.0, 0.0, 5.0)
        );
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
        let responders = &mut world.resource_mut::<PendingScreenshotResponders>().map;
        assert_eq!(responders.len(), 1);
        responders
            .get_mut(&screenshot_entity)
            .unwrap()
            .frames_waited = MAX_CAPTURE_WAIT_FRAMES;

        process_pending_screenshots(&mut world);

        let error = response_rx.try_recv().unwrap().unwrap_err();
        assert_eq!(error.message, CAPTURE_DEADLINE_ERROR);
        assert!(world.get_entity(screenshot_entity).is_err());
        assert_eq!(*world.get::<Transform>(camera).unwrap(), original);
        assert_eq!(
            *world.get::<GlobalTransform>(camera).unwrap(),
            GlobalTransform::from(original)
        );
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
        assert!(
            world
                .resource::<PendingScreenshotResponders>()
                .map
                .is_empty()
        );
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
    fn windowed_entity_capture_stages_before_spawning_the_screenshot() {
        let mut world = world_with_gizmos();
        world.spawn((Window::default(), PrimaryWindow));
        world.spawn((Camera::default(), RenderTarget::Window(WindowRef::Primary)));
        world.spawn(Name::new("Target"));
        let (tx, _rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                required_render_epoch: None,
                with_gizmos: false,
                gizmo_restore: GizmoEnabledRestore::new(),
                max_width: None,
                debug_camera: None,
                hide_ui: false,
                entity: Some(EntityRef::Name("Target".to_string())),
                depth_analysis: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });

        process_pending_screenshots(&mut world);

        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 0);
        let staged = world.resource::<StagedScreenshots>();
        assert_eq!(staged.pending.len(), 1);
        assert!(staged.pending[0].hold.entity_isolation.is_some());
        assert_eq!(
            staged.pending[0].frames_remaining,
            ENTITY_ISOLATION_SETTLE_FRAMES - 1
        );
    }

    #[test]
    fn process_screenshot_stores_gizmo_restore_in_responder() {
        let (mut world, _rx) = world_with_pending_screenshot(false);

        process_pending_screenshots(&mut world);

        let responders = world.resource::<PendingScreenshotResponders>();
        assert_eq!(responders.map.len(), 1);
        let responder = responders.map.values().next().unwrap();
        // Should store the original enabled state for restoration
        assert_eq!(responder.hold.gizmo_restore, default_group_restore(true));
    }

    #[test]
    fn process_screenshot_gizmo_capture_has_no_restore() {
        let (mut world, _rx) = world_with_pending_screenshot(true);

        process_pending_screenshots(&mut world);

        let responders = world.resource::<PendingScreenshotResponders>();
        let responder = responders.map.values().next().unwrap();
        // Gizmo screenshot should not need restoration
        assert!(responder.hold.gizmo_restore.is_empty());
    }

    #[test]
    fn process_screenshot_with_delay_does_not_capture_yet() {
        let mut world = world_with_gizmos();
        let (tx, _rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx: tx,
                frames_remaining: 2,
                required_render_epoch: None,
                with_gizmos: false,
                gizmo_restore: GizmoEnabledRestore::new(),
                max_width: None,
                debug_camera: None,
                hide_ui: false,
                entity: None,
                depth_analysis: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });

        process_pending_screenshots(&mut world);

        // Should not have spawned a Screenshot yet
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 0);
        // Suppression begins while the capture is still delayed, before the
        // next Update systems can submit global gizmos.
        assert!(!gizmos_enabled(&world));
        // Request should still be pending with decremented count
        let pending = world.resource::<PendingScreenshots>();
        assert_eq!(pending.pending.len(), 1);
        assert_eq!(pending.pending[0].frames_remaining, 1);
        assert_eq!(
            pending.pending[0].gizmo_restore,
            default_group_restore(true)
        );
    }

    #[test]
    fn pending_capture_waits_for_its_required_render_epoch() {
        let mut world = world_with_gizmos();
        let readiness = RenderFrameReadiness::default();
        let required_render_epoch = readiness.request_frame();
        world.insert_resource(readiness.clone());
        let (tx, _rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx: tx,
                frames_remaining: 2,
                required_render_epoch: Some(required_render_epoch),
                with_gizmos: false,
                gizmo_restore: GizmoEnabledRestore::new(),
                max_width: None,
                debug_camera: None,
                hide_ui: false,
                entity: None,
                depth_analysis: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });

        process_pending_screenshots(&mut world);
        assert_eq!(
            world.resource::<PendingScreenshots>().pending[0].frames_remaining,
            2
        );

        readiness.complete_requested_frame();
        process_pending_screenshots(&mut world);
        assert_eq!(
            world.resource::<PendingScreenshots>().pending[0].frames_remaining,
            1
        );
    }

    #[test]
    fn headless_ui_capture_waits_for_hidden_frame() {
        let mut world = world_with_gizmos();
        spawn_structurally_valid_headless_camera(&mut world);
        let ui = world.spawn((Visibility::Visible, Node::default())).id();
        let (response_tx, _response_rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx,
                frames_remaining: 0,
                required_render_epoch: None,
                with_gizmos: false,
                gizmo_restore: GizmoEnabledRestore::new(),
                max_width: None,
                debug_camera: None,
                hide_ui: true,
                entity: None,
                depth_analysis: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });

        process_pending_screenshots(&mut world);

        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Hidden);
        let staged = world.resource::<StagedScreenshots>();
        assert_eq!(staged.pending.len(), 1);
        assert_eq!(staged.pending[0].frames_remaining, 1);
        assert!(staged.pending[0].hold.debug_cleanup.is_none());
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
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
                required_render_epoch: None,
                with_gizmos: true,
                gizmo_restore: GizmoEnabledRestore::new(),
                max_width: Some(512),
                debug_camera: Some(DebugCameraRequest {
                    position: [10.0, 5.0, 0.0],
                    look_at: [0.0, 0.0, 0.0],
                }),
                hide_ui: false,
                entity: None,
                depth_analysis: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });

        process_pending_screenshots(&mut world);

        // Debug camera path stages the screenshot (extra 2 frame delay)
        // so no Screenshot entity yet, and gizmos untouched
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 0);
        assert!(gizmos_enabled(&world));

        // Staged screenshot should exist with with_gizmos preserved
        let staged = world.resource::<StagedScreenshots>();
        assert_eq!(staged.pending.len(), 1);
        assert!(staged.pending[0].with_gizmos);
        assert_eq!(staged.pending[0].max_width, Some(512));
    }

    #[test]
    fn process_staged_debug_screenshot_toggles_gizmos_on_capture() {
        let mut world = world_with_gizmos();
        // Spawn a PrimaryWindow entity so the windowed screenshot path is taken
        world.spawn((Window::default(), PrimaryWindow));

        // Directly insert a staged screenshot (simulating debug camera already set up)
        let (tx, _rx) = oneshot::channel();
        let cleanup = DebugCameraCleanup {
            debug_entity: world
                .spawn((Camera3d::default(), Transform::default()))
                .id(),
            reused_state: None,
            original_cameras: vec![],
        };
        world.insert_resource(StagedScreenshots {
            pending: vec![StagedScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                // Normal screenshot via debug camera
                with_gizmos: false,
                max_width: None,
                hold: CaptureHold {
                    leased: true,
                    debug_cleanup: Some(cleanup),
                    ..Default::default()
                },
                depth_analysis: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
                baseline_headless_sequence: None,
                frames_waited: 0,
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
    fn ready_captures_start_one_at_a_time() {
        let (mut world, _first_rx) = world_with_pending_screenshot(false);
        let (second, _second_rx) = queued_screenshot(0);
        world
            .resource_mut::<PendingScreenshots>()
            .pending
            .push(second);

        process_pending_screenshots(&mut world);

        assert_eq!(world.resource::<PendingScreenshotResponders>().map.len(), 1);
        assert_eq!(world.resource::<PendingScreenshots>().pending.len(), 1);
        assert!(capture_in_progress(&world));
    }

    #[test]
    fn abandoned_queued_screenshot_restores_gizmos() {
        let mut world = world_with_gizmos();
        let (screenshot, rx) = queued_screenshot(5);
        world.insert_resource(PendingScreenshots {
            pending: vec![screenshot],
        });
        process_pending_screenshots(&mut world);
        assert!(!gizmos_enabled(&world));

        drop(rx);
        process_pending_screenshots(&mut world);

        assert!(gizmos_enabled(&world));
        assert!(world.resource::<PendingScreenshots>().pending.is_empty());
    }

    #[test]
    fn waiting_responder_below_deadline_is_kept() {
        let mut world = world_with_gizmos();
        let screenshot_entity = world.spawn_empty().id();
        let (tx, mut rx) = oneshot::channel();
        let mut responders = PendingScreenshotResponders::default();
        responders.map.insert(
            screenshot_entity,
            ScreenshotResponder {
                response_tx: tx,
                max_width: None,
                hold: CaptureHold {
                    leased: true,
                    ..Default::default()
                },
                extra_response: None,
                capture_viewport: None,
                response_kind: CaptureResponseKind::Screenshot,
                frames_waited: 0,
            },
        );
        world.insert_resource(responders);
        world.insert_resource(PendingScreenshots::default());

        process_pending_screenshots(&mut world);

        assert!(rx.try_recv().is_err(), "response must remain deferred");
        let responders = world.resource::<PendingScreenshotResponders>();
        assert_eq!(responders.map[&screenshot_entity].frames_waited, 1);
    }

    #[test]
    fn stale_headless_frame_fails_instead_of_becoming_a_capture() {
        let mut world = world_with_gizmos();
        spawn_structurally_valid_headless_camera(&mut world);
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![255, 0, 0, 255], 1, 1)),
            sequence: 5,
        });
        world.insert_resource(OverlaySuppression(1));
        world.insert_resource(PendingScreenshots::default());
        let (tx, mut rx) = oneshot::channel();
        world.insert_resource(StagedScreenshots {
            pending: vec![StagedScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                with_gizmos: false,
                hold: CaptureHold {
                    leased: true,
                    ..Default::default()
                },
                max_width: None,
                depth_analysis: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
                baseline_headless_sequence: Some(5),
                frames_waited: MAX_CAPTURE_WAIT_FRAMES,
            }],
        });

        process_pending_screenshots(&mut world);

        let error = rx.try_recv().unwrap().unwrap_err();
        assert_eq!(error.message, STALE_HEADLESS_FRAME_ERROR);
        assert!(world.resource::<StagedScreenshots>().pending.is_empty());
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
    }

    #[test]
    fn newer_headless_frame_completes_the_staged_capture() {
        let mut world = world_with_gizmos();
        spawn_structurally_valid_headless_camera(&mut world);
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![0, 255, 0, 255], 1, 1)),
            sequence: 6,
        });
        world.insert_resource(OverlaySuppression(1));
        world.insert_resource(PendingScreenshots::default());
        let (tx, mut rx) = oneshot::channel();
        world.insert_resource(StagedScreenshots {
            pending: vec![StagedScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                with_gizmos: false,
                hold: CaptureHold {
                    leased: true,
                    ..Default::default()
                },
                max_width: None,
                depth_analysis: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
                baseline_headless_sequence: Some(5),
                frames_waited: 0,
            }],
        });

        process_pending_screenshots(&mut world);

        let capture = rx.try_recv().unwrap().unwrap();
        assert_eq!(capture["width"], 1);
        assert!(world.resource::<StagedScreenshots>().pending.is_empty());
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
    }

    #[test]
    fn staged_capture_waits_instead_of_using_a_despawned_cameras_frame() {
        let mut world = world_with_gizmos();
        spawn_structurally_valid_headless_camera(&mut world);
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![255, 0, 0, 255], 1, 1)),
            sequence: 5,
        });
        world.insert_resource(OverlaySuppression(1));
        world.insert_resource(PendingScreenshots::default());
        let (tx, mut rx) = oneshot::channel();
        world.insert_resource(StagedScreenshots {
            pending: vec![StagedScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                with_gizmos: false,
                hold: CaptureHold {
                    leased: true,
                    ..Default::default()
                },
                max_width: None,
                depth_analysis: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
                baseline_headless_sequence: Some(5),
                frames_waited: 0,
            }],
        });

        world.resource_mut::<HeadlessFrameBuffer>().sync(None);
        process_pending_screenshots(&mut world);

        assert!(rx.try_recv().is_err(), "capture must wait for a new frame");
        assert_eq!(world.resource::<StagedScreenshots>().pending.len(), 1);

        world
            .resource_mut::<HeadlessFrameBuffer>()
            .sync(Some((vec![0, 255, 0, 255], 1, 1, 6)));
        process_pending_screenshots(&mut world);

        let capture = rx.try_recv().unwrap().unwrap();
        assert_eq!(capture["width"], 1);
        assert!(world.resource::<StagedScreenshots>().pending.is_empty());
    }

    #[test]
    fn encode_screenshot_basic() {
        let img = make_test_image(100, 80);
        let result = encode_screenshot(img, None).unwrap();
        assert_eq!(result["width"], 100);
        assert_eq!(result["height"], 80);
        assert_eq!(result["format"], "png");
        assert_eq!(result["encoding"], "base64");
        assert!(!result["image"].as_str().unwrap().is_empty());
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
    fn window_image_completion_uses_the_numeric_analysis_path() {
        let image = make_test_image(2, 1);
        let mut frames = CapturedFrames::default();
        let result = complete_screenshot_capture(
            image,
            None,
            None,
            CaptureResponseKind::Stats(FrameStatsOptions {
                grid: 1,
                region: None,
                sample_points: Some(vec![[1, 0]]),
            }),
            &mut frames,
            Some(7),
        )
        .unwrap();

        assert_eq!(result["overall"]["luma_mean"], 1.0);
        assert_eq!(
            result["samples"][0]["rgb"],
            serde_json::json!([1.0, 1.0, 1.0])
        );
        assert!(result.get("image").is_none());
        assert_eq!(result["retained"], true);
    }

    #[test]
    fn depth_rgb_completion_does_not_enter_frame_retention() {
        let image = make_test_image(1, 1);
        let mut frames = CapturedFrames::default();
        let result = complete_screenshot_capture(
            image,
            None,
            None,
            CaptureResponseKind::UnretainedScreenshot,
            &mut frames,
            None,
        )
        .unwrap();

        assert!(result.get("frame_id").is_none());
        assert!(
            frames
                .compare("f_0000000000000000", "f_0000000000000000", 0.0)
                .is_err()
        );
    }

    #[test]
    fn reload_and_capture_completion_still_enters_frame_retention() {
        let image = make_test_image(1, 1);
        let mut frames = CapturedFrames::default();
        let result = complete_screenshot_capture(
            image,
            None,
            None,
            CaptureResponseKind::ReloadAndCapture,
            &mut frames,
            None,
        )
        .unwrap();

        assert_eq!(result["retained"], true);
        assert!(result["frame_id"].as_str().is_some());
    }

    #[test]
    fn depth_rgb_completion_crops_to_the_selected_camera_viewport() {
        let image = make_test_image(4, 3);
        let mut frames = CapturedFrames::default();
        let result = complete_screenshot_capture(
            image,
            None,
            Some(CaptureViewport {
                physical_position: UVec2::new(1, 1),
                physical_size: UVec2::new(2, 1),
            }),
            CaptureResponseKind::UnretainedScreenshot,
            &mut frames,
            None,
        )
        .unwrap();

        assert_eq!(result["width"], 2);
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
        let result = Ok(serde_json::json!({
            "image": "abc",
            "width": 10,
            "height": 20,
            "frame_id": "f_0001",
            "retained": true,
        }));
        let extra = Some(serde_json::json!({"depth_samples": {"hit_count": 3}, "custom": true}));
        let merged = merge_extra_response(result, extra).unwrap();
        assert_eq!(merged["screenshot"], "abc");
        assert_eq!(merged["screenshot_width"], 10);
        assert_eq!(merged["screenshot_height"], 20);
        assert_eq!(merged["frame_id"], "f_0001");
        assert_eq!(merged["retained"], true);
        assert!(merged.get("retention_reason").is_none());
        assert_eq!(merged["depth_samples"]["hit_count"], 3);
        assert_eq!(merged["custom"], true);
    }

    #[test]
    fn merge_extra_response_err_returns_extra_with_null_screenshot() {
        let result = Err(ControlError::not_found("fail"));
        let extra = Some(serde_json::json!({"reload": "ok", "entity_count": 42}));
        let merged = merge_extra_response(result, extra).unwrap();
        assert!(merged["screenshot"].is_null());
        assert_eq!(merged["screenshot_error"], "fail");
        assert_eq!(merged["reload"], "ok");
        assert_eq!(merged["entity_count"], 42);
    }

    #[test]
    fn merge_extra_response_ok_has_no_screenshot_error() {
        let result = Ok(serde_json::json!({"image": "abc", "width": 10, "height": 20}));
        let merged = merge_extra_response(result, Some(serde_json::json!({}))).unwrap();
        assert!(merged.get("screenshot_error").is_none());
    }

    #[test]
    fn encode_rgb_screenshot_produces_valid_png() {
        let mut rgb = RgbImage::new(4, 4);
        for x in 0..4 {
            for y in 0..4 {
                rgb.put_pixel(x, y, Rgb([100, 150, 200]));
            }
        }
        let result = encode_rgb_screenshot(&rgb).unwrap();
        assert_eq!(result["format"], "png");
        assert_eq!(result["width"], 4);
        assert_eq!(result["height"], 4);
        assert!(result["image"].as_str().unwrap().len() > 10);
    }

    #[test]
    fn encode_rgb_screenshot_resizes() {
        let rgb = RgbImage::new(100, 50);
        let resized = resize_rgb_image_linear(rgb, Some(20));
        let result = encode_rgb_screenshot(&resized).unwrap();
        assert_eq!(result["width"], 20);
        assert_eq!(result["height"], 10);
    }
}
