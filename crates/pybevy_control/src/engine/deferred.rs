use std::sync::{Arc, Mutex};

use bevy::{
    camera::Camera,
    ecs::world::World,
    time::{Time, Virtual},
};
use pybevy_core::public_error::capture_no_camera;
use tokio::sync::oneshot;

use crate::{
    capture::{
        analysis::{FrameStatsOptions, validate_frame_stats_options},
        camera::select_capture_camera_3d,
        depth::{
            PendingDepthAnalysis, PendingDepthSample, PendingDepthSamples, validate_depth_sampling,
        },
        hold::prepare_pending_screenshot_gizmos,
        queue::{
            CaptureResponseKind, DEFAULT_SCREENSHOT_MAX_WIDTH, DEFAULT_TIMELINE_MAX_WIDTH,
            DebugCameraRequest, PendingScreenshot, PendingScreenshots,
        },
        screenshot::{validate_capture_delay, validate_capture_pose},
        timeline::{
            ActiveTimeline, MAX_TIMELINE_CAPTURES, MAX_TIMELINE_FRAMES, PendingTimelines,
            compute_schedule,
        },
        turnaround::{
            ActiveTurnaround, MAX_TURNAROUND_VIEWS, PendingTurnarounds, compute_viewpoints,
            turnaround_framing,
        },
    },
    engine::channel::ControlRequest,
    protocol::{
        CaptureDepthParams, CaptureScreenshotParams, CaptureStatsParams, CaptureTimelineParams,
        CaptureTurnaroundParams, ControlError, ControlOperation, ReloadAndCaptureParams,
        ReloadParams,
    },
    reload::{self, PendingReload, PendingReloads, ReloadCapture, ReloadCompletion},
    schedule::{
        ActiveSchedule, ActiveSchedules, ScheduleMode, ScheduleRequest,
        SharedScheduleRegistryResource, SharedScheduleState,
    },
};

/// Deferred requests collected in one frame, stored together by [`Self::commit`].
pub(crate) struct DeferredBatch {
    render_epoch: Option<u64>,
    screenshots: Vec<PendingScreenshot>,
}

impl DeferredBatch {
    /// `render_epoch` makes queued screenshots wait for that render epoch
    /// before their capture delay starts.
    pub(crate) fn new(render_epoch: Option<u64>) -> Self {
        Self {
            render_epoch,
            screenshots: Vec::new(),
        }
    }

    /// Queue a deferred operation. A request whose operation is not deferred
    /// is handed back unchanged.
    pub(crate) fn enqueue(
        &mut self,
        world: &mut World,
        request: ControlRequest,
    ) -> Option<ControlRequest> {
        let ControlRequest {
            operation,
            response_tx,
        } = request;
        let screenshots = &mut self.screenshots;
        match operation {
            ControlOperation::CaptureScreenshot(p) => {
                let with_gizmos = p.gizmos;
                push_pending_screenshot(p, with_gizmos, response_tx, screenshots);
            }
            ControlOperation::CaptureWithGizmos(p) => {
                push_pending_screenshot(p, true, response_tx, screenshots);
            }
            ControlOperation::CaptureStats(p) => push_pending_stats(p, response_tx, screenshots),
            ControlOperation::CaptureTimeline(p) => push_pending_timeline(p, response_tx, world),
            ControlOperation::CaptureTurnaround(p) => {
                push_pending_turnaround(p, response_tx, world);
            }
            ControlOperation::CaptureDepth(p) => {
                push_pending_depth(p, response_tx, screenshots, world);
            }
            ControlOperation::Reload(p) => push_pending_reload(p, response_tx, world),
            ControlOperation::ReloadAndCapture(p) => {
                push_pending_reload_and_capture(p, response_tx, world);
            }
            operation => {
                return Some(ControlRequest {
                    operation,
                    response_tx,
                });
            }
        }
        None
    }

    /// Suppress gizmos for the queued screenshots and hand them to the
    /// capture systems.
    pub(crate) fn commit(mut self, world: &mut World) {
        if self.screenshots.is_empty() {
            return;
        }
        for screenshot in &mut self.screenshots {
            if let Some(epoch) = self.render_epoch {
                screenshot.required_render_epoch = Some(epoch);
            }
            prepare_pending_screenshot_gizmos(world, screenshot);
        }
        world
            .get_resource_or_insert_with(PendingScreenshots::default)
            .pending
            .extend(self.screenshots);
    }
}

pub fn push_pending_screenshot(
    params: CaptureScreenshotParams,
    with_gizmos: bool,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    deferred: &mut Vec<PendingScreenshot>,
) {
    if let Err(error) = validate_capture_delay(params.delay_frames)
        .and_then(|()| validate_capture_pose(params.position, params.look_at))
    {
        let _ = response_tx.send(Err(error));
        return;
    }
    let defaults = PendingScreenshot::new(response_tx, CaptureResponseKind::Screenshot);
    deferred.push(PendingScreenshot {
        frames_remaining: params.delay_frames,
        with_gizmos,
        max_width: params.max_width.or(defaults.max_width),
        debug_camera: DebugCameraRequest::from_pose(params.position, params.look_at),
        hide_ui: params.hide_ui,
        entity: params.entity,
        ..defaults
    });
}

pub fn push_pending_stats(
    params: CaptureStatsParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    deferred: &mut Vec<PendingScreenshot>,
) {
    if params.max_width == Some(0) {
        let _ = response_tx.send(Err(ControlError::invalid_params(
            "max_width must be a positive integer",
        )));
        return;
    }
    let options = FrameStatsOptions {
        grid: params.grid,
        region: params.region,
        sample_points: params.sample_points,
    };
    if let Err(error) = validate_frame_stats_options(&options)
        .and_then(|()| validate_capture_delay(params.delay_frames))
        .and_then(|()| validate_capture_pose(params.position, params.look_at))
    {
        let _ = response_tx.send(Err(error));
        return;
    }
    let defaults = PendingScreenshot::new(response_tx, CaptureResponseKind::Stats(options));
    deferred.push(PendingScreenshot {
        frames_remaining: params.delay_frames,
        with_gizmos: params.gizmos,
        max_width: params.max_width.or(defaults.max_width),
        debug_camera: DebugCameraRequest::from_pose(params.position, params.look_at),
        hide_ui: params.hide_ui,
        entity: params.entity,
        ..defaults
    });
}

pub fn push_pending_timeline(
    params: CaptureTimelineParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    // capture_count == 0 makes compute_schedule emit a single frame while
    // total_captures stays 0, so the completion check never matches and the
    // request hangs until timeout; above the max the contact sheet is unusable.
    if params.capture_count < 1 || params.capture_count > MAX_TIMELINE_CAPTURES {
        let _ = response_tx.send(Err(ControlError::invalid_params(format!(
            "capture_count must be between 1 and {MAX_TIMELINE_CAPTURES}"
        ))));
        return;
    }
    if !(1..=MAX_TIMELINE_CAPTURES).contains(&params.columns) {
        let _ = response_tx.send(Err(ControlError::invalid_params(format!(
            "columns must be between 1 and {MAX_TIMELINE_CAPTURES}"
        ))));
        return;
    }
    if params.total_frames > MAX_TIMELINE_FRAMES {
        let _ = response_tx.send(Err(ControlError::invalid_params(format!(
            "total_frames must be at most {MAX_TIMELINE_FRAMES}"
        ))));
        return;
    }

    if let Err(error) = validate_capture_pose(params.position, params.look_at) {
        let _ = response_tx.send(Err(error));
        return;
    }

    let mut schedule = compute_schedule(params.total_frames, params.capture_count);
    // Let one post-suppression frame reach both window and headless targets.
    if let Some(first) = schedule.front_mut() {
        *first += 1;
    }

    let debug_camera = DebugCameraRequest::from_pose(params.position, params.look_at);
    if debug_camera.is_some()
        && let Some(first) = schedule.front_mut()
    {
        *first += 2;
    }

    let mut pending = world.get_resource_or_insert_with(PendingTimelines::default);
    let id = pending.next_id;
    pending.next_id += 1;
    pending.active.insert(
        id,
        ActiveTimeline {
            response_tx: Some(response_tx),
            // A contact sheet composes capture_count frames at this width, so
            // an unclamped default produces a payload many times the size of a
            // single screenshot. capture_turnaround already defaults the same
            // way; capture_screenshot clamps to DEFAULT_SCREENSHOT_MAX_WIDTH.
            max_width: Some(params.max_width.unwrap_or(DEFAULT_TIMELINE_MAX_WIDTH)),
            columns: params.columns,
            debug_camera,
            schedule,
            total_captures: params.capture_count,
            next_capture_index: 0,
            collected: Vec::new(),
            hide_ui: params.hide_ui,
            with_gizmos: params.gizmos,
            hold: None,
            headless_sequence: None,
            stall_frames: 0,
        },
    );
}

pub fn push_pending_reload(
    params: ReloadParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    // Surface validation errors (e.g. an out-of-range time_scale) directly.
    // Otherwise the caller would silently drop the bad param and still receive
    // a false "reload_completed" from the deferred response waiter below.
    if let Err(e) = reload::trigger_reload(world, params.mode, params.pause, params.time_scale) {
        let _ = response_tx.send(Err(e));
        return;
    }

    let mut pending = world.get_resource_or_insert_with(PendingReloads::default);
    pending.pending.push(PendingReload::new(
        response_tx,
        params.mode,
        ReloadCompletion::Respond,
    ));
}

pub fn push_pending_reload_and_capture(
    params: ReloadAndCaptureParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    // Surface validation errors (e.g. an out-of-range time_scale) directly
    // instead of dropping the bad param and reporting a false success later.
    if let Err(e) = validate_capture_delay(params.delay_frames.unwrap_or(30))
        .and_then(|()| validate_capture_pose(params.position, params.look_at))
        .and_then(|()| reload::trigger_reload(world, params.mode, params.pause, params.time_scale))
    {
        let _ = response_tx.send(Err(e));
        return;
    }

    let capture = ReloadCapture {
        delay_frames: params.delay_frames.unwrap_or(30),
        max_width: params.max_width,
        position: params.position,
        look_at: params.look_at,
        hide_ui: params.hide_ui.unwrap_or(true),
    };
    let mut pending = world.get_resource_or_insert_with(PendingReloads::default);
    pending.pending.push(PendingReload::new(
        response_tx,
        params.mode,
        ReloadCompletion::Capture(capture),
    ));
}

pub fn push_pending_turnaround(
    params: CaptureTurnaroundParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    if params
        .distance
        .is_some_and(|distance| !distance.is_finite() || distance <= 0.0)
    {
        let _ = response_tx.send(Err(ControlError::invalid_params(
            "distance must be > 0 and finite",
        )));
        return;
    }

    let vc = params.view_count.unwrap_or(6);
    if !(1..=MAX_TURNAROUND_VIEWS).contains(&vc) {
        let _ = response_tx.send(Err(ControlError::invalid_params(format!(
            "view_count must be between 1 and {MAX_TURNAROUND_VIEWS}"
        ))));
        return;
    }

    let elev = params.elevation.unwrap_or(25.0);
    let top = params.include_top.unwrap_or(true);

    let (final_look_at, final_distance) =
        turnaround_framing(world, params.look_at, params.distance);
    let viewpoints = compute_viewpoints(final_look_at, final_distance, elev, vc, top);
    // Also catches a non-finite elevation, center or auto-fit distance.
    if viewpoints
        .iter()
        .flat_map(|view| view.position)
        .any(|coordinate| !coordinate.is_finite())
    {
        let _ = response_tx.send(Err(ControlError::invalid_params(
            "elevation, look_at and distance must give finite camera positions",
        )));
        return;
    }

    let mut pending = world.get_resource_or_insert_with(PendingTurnarounds::default);
    pending.active.push(ActiveTurnaround {
        response_tx: Some(response_tx),
        viewpoints,
        current_index: 0,
        captures: Vec::new(),
        columns: params.columns.unwrap_or(3),
        max_width: Some(params.max_width.unwrap_or(1200)),
        frames_remaining: 0,
        view_staged: false,
        hide_ui: params.hide_ui.unwrap_or(true),
        hold: None,
        debug_cleanup: None,
        look_at: final_look_at,
        pending_screenshot_entity: None,
        headless_sequence: None,
        stall_frames: 0,
    });
}

pub fn push_pending_depth(
    params: CaptureDepthParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    deferred: &mut Vec<PendingScreenshot>,
    world: &mut World,
) {
    if let Err(error) = validate_depth_sampling(&params.sample_points, &params.grid_density)
        .and_then(|()| validate_capture_delay(params.delay_frames.unwrap_or(2)))
        .and_then(|()| validate_capture_pose(params.position, params.look_at))
    {
        let _ = response_tx.send(Err(error));
        return;
    }
    let Some(camera_entity) = select_capture_camera_3d(world) else {
        let error = if world.query::<&Camera>().iter(world).next().is_none() {
            ControlError::invalid_params(capture_no_camera("capture_depth"))
        } else {
            ControlError::not_found("No active Camera3d found for capture_depth projection")
        };
        let _ = response_tx.send(Err(error));
        return;
    };
    let CaptureDepthParams {
        position,
        look_at,
        sample_points,
        grid_density,
        include_rgb,
        delay_frames,
        hide_ui,
        max_width,
    } = params;
    let want_rgb = include_rgb.unwrap_or(true);
    let df = delay_frames.unwrap_or(2);
    let mw = Some(max_width.unwrap_or(DEFAULT_SCREENSHOT_MAX_WIDTH));
    let hu = hide_ui.unwrap_or(true);
    let dc = DebugCameraRequest::from_pose(position, look_at);
    let depth_analysis = PendingDepthAnalysis {
        camera_entity,
        position,
        look_at,
        sample_points,
        grid_density,
    };

    if want_rgb {
        deferred.push(PendingScreenshot {
            frames_remaining: df,
            max_width: mw,
            debug_camera: dc,
            hide_ui: hu,
            depth_analysis: Some(depth_analysis),
            ..PendingScreenshot::new(response_tx, CaptureResponseKind::UnretainedScreenshot)
        });
    } else {
        world
            .get_resource_or_insert_with(PendingDepthSamples::default)
            .pending
            .push(PendingDepthSample {
                response_tx,
                frames_remaining: df,
                analysis: depth_analysis,
            });
    }
}

/// Handle ScheduleActions requests (no GIL needed).
pub(crate) fn handle_submit_schedule(
    sched_req: ScheduleRequest,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    let mut active = world.get_resource_or_insert_with(ActiveSchedules::default);
    let schedule_id = format!("schedule-{}", active.next_id);
    active.next_id += 1;

    let t0 = world
        .get_resource::<Time<Virtual>>()
        .map(|t| t.elapsed_secs_f64())
        .unwrap_or(0.0);

    if sched_req.mode == ScheduleMode::Async {
        let shared = Arc::new(Mutex::new(SharedScheduleState::new(
            &schedule_id,
            sched_req.actions.len(),
        )));

        if let Some(registry_res) = world.get_resource::<SharedScheduleRegistryResource>() {
            registry_res.0.insert(schedule_id.clone(), shared.clone());
        }

        let _ = response_tx.send(Ok(serde_json::json!({
            "schedule_id": schedule_id,
            "status": "running",
        })));

        let mut active = world.get_resource_or_insert_with(ActiveSchedules::default);
        active.schedules.push(ActiveSchedule::new_async(
            schedule_id,
            sched_req,
            t0,
            shared,
        ));
    } else {
        let mut active = world.get_resource_or_insert_with(ActiveSchedules::default);
        active.schedules.push(ActiveSchedule::new_sync(
            schedule_id,
            sched_req,
            t0,
            response_tx,
        ));
    }
}

#[cfg(test)]
mod tests {
    use bevy::{asset::Assets, camera::RenderTarget, image::Image, prelude::Camera2d};

    use super::*;
    use crate::{
        capture::{
            hold::OverlaySuppression,
            test_support::{make_test_image, spawn_structurally_valid_headless_camera},
            timeline::process_pending_timelines,
        },
        protocol::{ErrorCode, ReloadMode},
    };

    #[test]
    fn enqueue_hands_back_requests_that_are_not_deferred() {
        let mut world = World::new();
        let (response_tx, _rx) = oneshot::channel();
        let mut batch = DeferredBatch::new(Some(3));
        let request = ControlRequest {
            operation: ControlOperation::PauseTime,
            response_tx,
        };

        let returned = batch
            .enqueue(&mut world, request)
            .expect("pause_time is not deferred");
        assert!(matches!(returned.operation, ControlOperation::PauseTime));
        batch.commit(&mut world);
        assert!(!world.contains_resource::<PendingScreenshots>());
    }

    #[test]
    fn commit_applies_the_batch_render_epoch() {
        for epoch in [None, Some(7)] {
            let mut world = World::new();
            let (response_tx, _rx) = oneshot::channel();
            let mut batch = DeferredBatch::new(epoch);
            let request = ControlRequest {
                operation: ControlOperation::CaptureScreenshot(serde_json::from_str("{}").unwrap()),
                response_tx,
            };

            assert!(batch.enqueue(&mut world, request).is_none());
            batch.commit(&mut world);
            let pending = world.resource::<PendingScreenshots>();
            assert_eq!(pending.pending.len(), 1);
            assert_eq!(pending.pending[0].required_render_epoch, epoch);
        }
    }

    #[test]
    fn push_pending_screenshot_applies_default_max_width() {
        let (tx, _rx) = oneshot::channel();
        let params: CaptureScreenshotParams = serde_json::from_str("{}").unwrap();
        let mut deferred = Vec::new();
        push_pending_screenshot(params, false, tx, &mut deferred);
        assert_eq!(deferred[0].max_width, Some(DEFAULT_SCREENSHOT_MAX_WIDTH));
    }

    #[test]
    fn push_pending_screenshot_keeps_explicit_max_width() {
        let (tx, _rx) = oneshot::channel();
        let params: CaptureScreenshotParams =
            serde_json::from_str(r#"{"max_width": 1280}"#).unwrap();
        let mut deferred = Vec::new();
        push_pending_screenshot(params, false, tx, &mut deferred);
        assert_eq!(deferred[0].max_width, Some(1280));
    }
    #[test]
    fn push_pending_stats_preserves_analysis_options() {
        let (tx, _rx) = oneshot::channel();
        let params: CaptureStatsParams = serde_json::from_str(
            r#"{"grid": 3, "region": [1, 2, 9, 6], "sample_points": [[2, 3]]}"#,
        )
        .unwrap();
        let mut deferred = Vec::new();
        push_pending_stats(params, tx, &mut deferred);

        assert_eq!(deferred[0].max_width, Some(DEFAULT_SCREENSHOT_MAX_WIDTH));
        let CaptureResponseKind::Stats(options) = &deferred[0].response_kind else {
            panic!("capture_stats must queue a stats response");
        };
        assert_eq!(options.grid, 3);
        assert_eq!(options.region, Some([1, 2, 9, 6]));
        assert_eq!(options.sample_points, Some(vec![[2, 3]]));
    }

    #[test]
    fn push_pending_stats_rejects_invalid_options_before_capture() {
        let (tx, rx) = oneshot::channel();
        let params: CaptureStatsParams = serde_json::from_str(r#"{"grid": 0}"#).unwrap();
        let mut deferred = Vec::new();
        push_pending_stats(params, tx, &mut deferred);

        assert!(deferred.is_empty());
        let error = rx.blocking_recv().unwrap().unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(error.message.contains("grid"));
    }

    #[test]
    fn push_pending_depth_returns_structured_coordinate_error() {
        let (tx, rx) = oneshot::channel();
        let params: CaptureDepthParams = serde_json::from_str(
            r#"{"sample_points": [[99999, -50], [10, 10]], "include_rgb": false}"#,
        )
        .unwrap();
        let mut deferred = Vec::new();
        let mut world = World::new();

        push_pending_depth(params, tx, &mut deferred, &mut world);

        assert!(deferred.is_empty());
        let error = rx.blocking_recv().unwrap().unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert_eq!(
            error.message,
            "sample_points[0] must be within [0, 800) on both axes (got [99999, -50])"
        );
    }
    #[test]
    fn push_pending_reload_surfaces_time_scale_error() {
        // Regression: trigger_reload's Result was discarded with `let _ =`, so a
        // rejected time_scale silently dropped the param and the deferred waiter
        // still reported a false "reload_completed". The error must reach the
        // caller and no response waiter should be queued.
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        let (tx, mut rx) = oneshot::channel();
        push_pending_reload(
            ReloadParams {
                mode: ReloadMode::Full,
                pause: false,
                time_scale: Some(1.0e6),
            },
            tx,
            &mut world,
        );
        let result = rx.try_recv().expect("caller should receive a response");
        let err = result.expect_err("out-of-range time_scale must be an error");
        assert!(err.message.contains("1000"));
        // No deferred waiter queued, and the bad scale was not applied.
        let queued = world
            .get_resource::<PendingReloads>()
            .map(|p| p.pending.len())
            .unwrap_or(0);
        assert_eq!(queued, 0);
        let speed = world.resource::<Time<Virtual>>().relative_speed();
        assert!((speed - 1.0).abs() < 1e-6);
    }

    #[test]
    fn push_pending_reload_valid_time_scale_defers() {
        // A valid time_scale applies and queues the deferred waiter (no immediate
        // response), confirming the error path did not regress the happy path.
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        let (tx, mut rx) = oneshot::channel();
        push_pending_reload(
            ReloadParams {
                mode: ReloadMode::Full,
                pause: false,
                time_scale: Some(2.0),
            },
            tx,
            &mut world,
        );
        assert!(
            rx.try_recv().is_err(),
            "response is deferred, not immediate"
        );
        let queued = world
            .get_resource::<PendingReloads>()
            .map(|p| p.pending.len())
            .unwrap_or(0);
        assert_eq!(queued, 1);
        let speed = world.resource::<Time<Virtual>>().relative_speed();
        assert!((speed - 2.0).abs() < 1e-6);
    }

    fn timeline_params(capture_count: u32) -> CaptureTimelineParams {
        CaptureTimelineParams {
            total_frames: 6,
            capture_count,
            max_width: None,
            columns: 3,
            position: None,
            look_at: None,
            hide_ui: true,
            gizmos: false,
        }
    }

    #[test]
    fn push_pending_timeline_rejects_out_of_range_capture_count() {
        // Regression: capture_count == 0 made compute_schedule emit one frame
        // while total_captures stayed 0, so the completion check never matched
        // and the request hung until the 120s timeout. Out-of-range counts must
        // be rejected up front with no timeline queued.
        for bad in [0, MAX_TIMELINE_CAPTURES + 1] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            push_pending_timeline(timeline_params(bad), tx, &mut world);
            let result = rx.try_recv().expect("caller should receive a response");
            let err = result.expect_err("out-of-range capture_count must be an error");
            assert!(err.message.contains("between 1 and"));
            let queued = world
                .get_resource::<PendingTimelines>()
                .map(|p| p.active.len())
                .unwrap_or(0);
            assert_eq!(queued, 0);
        }
    }

    #[test]
    fn push_pending_timeline_rejects_out_of_range_columns() {
        for bad in [0, MAX_TIMELINE_CAPTURES + 1, u32::MAX] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            let mut params = timeline_params(6);
            params.columns = bad;
            push_pending_timeline(params, tx, &mut world);
            let err = rx
                .try_recv()
                .expect("caller should receive a response")
                .expect_err("out-of-range columns must be an error");
            assert_eq!(err.message, "columns must be between 1 and 20");
            assert!(world.get_resource::<PendingTimelines>().is_none());
        }
    }

    #[test]
    fn push_pending_timeline_accepts_valid_capture_count() {
        let mut world = World::new();
        spawn_structurally_valid_headless_camera(&mut world);
        let (tx, mut rx) = oneshot::channel();
        let mut params = timeline_params(6);
        params.hide_ui = false;
        params.gizmos = true;
        push_pending_timeline(params, tx, &mut world);
        assert!(
            rx.try_recv().is_err(),
            "response is deferred, not immediate"
        );
        let pending = world.resource::<PendingTimelines>();
        assert_eq!(pending.active.len(), 1);
        let timeline = pending.active.values().next().unwrap();
        assert!(!timeline.hide_ui);
        assert!(timeline.with_gizmos);
        assert!(timeline.hold.is_none(), "a queued timeline holds no state");
        assert_eq!(timeline.schedule.front(), Some(&1));
        assert!(!world.contains_resource::<OverlaySuppression>());

        process_pending_timelines(&mut world);
        let timeline = world
            .resource::<PendingTimelines>()
            .active
            .values()
            .next()
            .unwrap();
        assert!(timeline.hold.is_some());
        assert_eq!(timeline.schedule.front(), Some(&0));
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
    }

    #[test]
    fn push_pending_timeline_rejects_camera2d_position_override() {
        let mut world = World::new();
        let target = Assets::<Image>::default().add(make_test_image(1, 1));
        world.spawn((Camera2d, RenderTarget::Image(target.into())));
        let (tx, mut rx) = oneshot::channel();
        let mut params = timeline_params(6);
        params.position = Some([0.0, 0.0, 500.0]);

        push_pending_timeline(params, tx, &mut world);
        assert!(
            rx.try_recv().is_err(),
            "the pose is applied when the timeline starts"
        );
        process_pending_timelines(&mut world);

        let error = rx.try_recv().unwrap().unwrap_err();
        assert!(error.message.contains("require a Camera3d"));
        assert!(world.resource::<PendingTimelines>().active.is_empty());
        assert!(!world.contains_resource::<OverlaySuppression>());
    }

    #[test]
    fn push_pending_timeline_clamps_the_default_sheet_width() {
        // A contact sheet composes capture_count frames, so an unclamped
        // default produces a payload many times a single screenshot and can
        // exceed the transport's inline image bound. capture_turnaround
        // already defaults the same way.
        let mut world = World::new();
        let (tx, _rx) = oneshot::channel();
        push_pending_timeline(timeline_params(6), tx, &mut world);
        let width = world
            .get_resource::<PendingTimelines>()
            .and_then(|pending| pending.active.values().next().map(|t| t.max_width))
            .expect("timeline was queued");
        assert_eq!(width, Some(DEFAULT_TIMELINE_MAX_WIDTH));
    }

    #[test]
    fn push_pending_timeline_keeps_an_explicit_sheet_width() {
        let mut world = World::new();
        let (tx, _rx) = oneshot::channel();
        let mut params = timeline_params(6);
        params.max_width = Some(2400);
        push_pending_timeline(params, tx, &mut world);
        let width = world
            .get_resource::<PendingTimelines>()
            .and_then(|pending| pending.active.values().next().map(|t| t.max_width))
            .expect("timeline was queued");
        assert_eq!(width, Some(2400));
    }

    fn turnaround_params(distance: Option<f32>) -> CaptureTurnaroundParams {
        CaptureTurnaroundParams {
            look_at: None,
            distance,
            elevation: None,
            view_count: None,
            include_top: None,
            columns: None,
            max_width: None,
            hide_ui: None,
        }
    }

    #[test]
    fn push_pending_turnaround_rejects_invalid_distance() {
        for distance in [0.0, -1.0, f32::NEG_INFINITY, f32::INFINITY, f32::NAN] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            push_pending_turnaround(turnaround_params(Some(distance)), tx, &mut world);

            let error = rx
                .try_recv()
                .expect("caller should receive a response")
                .expect_err("invalid distance must be an error");
            assert_eq!(error.code, ErrorCode::InvalidParams);
            assert_eq!(error.message, "distance must be > 0 and finite");
            assert!(!world.contains_resource::<PendingTurnarounds>());
        }
    }

    #[test]
    fn push_pending_turnaround_accepts_automatic_and_positive_distance() {
        for distance in [None, Some(1.0)] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            push_pending_turnaround(turnaround_params(distance), tx, &mut world);

            assert!(rx.try_recv().is_err(), "response should remain deferred");
            assert_eq!(world.resource::<PendingTurnarounds>().active.len(), 1);
        }
    }

    #[test]
    fn push_pending_turnaround_rejects_nonfinite_camera_positions() {
        for (elevation, look_at, distance) in [
            (f32::NAN, None, 10.0),
            (f32::INFINITY, None, 10.0),
            (0.0, Some([f32::INFINITY, 0.0, 0.0]), 10.0),
            (0.0, Some([f32::MAX, 0.0, 0.0]), f32::MAX),
        ] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            let mut params = turnaround_params(Some(distance));
            params.elevation = Some(elevation);
            params.look_at = look_at;
            push_pending_turnaround(params, tx, &mut world);

            let error = rx
                .try_recv()
                .expect("caller should receive a response")
                .expect_err("non-finite camera position must be an error");
            assert_eq!(error.code, ErrorCode::InvalidParams);
            assert_eq!(
                error.message,
                "elevation, look_at and distance must give finite camera positions"
            );
            assert!(!world.contains_resource::<PendingTurnarounds>());
        }
    }

    #[test]
    fn push_pending_turnaround_accepts_finite_elevations_past_the_poles() {
        for elevation in [-270.0, 90.0, 450.0, 1e30] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            let mut params = turnaround_params(Some(10.0));
            params.elevation = Some(elevation);
            push_pending_turnaround(params, tx, &mut world);

            assert!(rx.try_recv().is_err(), "response should remain deferred");
            assert_eq!(world.resource::<PendingTurnarounds>().active.len(), 1);
        }
    }

    #[test]
    fn push_pending_turnaround_rejects_out_of_range_view_count() {
        for view_count in [0, MAX_TURNAROUND_VIEWS + 1] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            let mut params = turnaround_params(None);
            params.view_count = Some(view_count);
            push_pending_turnaround(params, tx, &mut world);

            let error = rx
                .try_recv()
                .expect("caller should receive a response")
                .expect_err("out-of-range view count must be an error");
            assert_eq!(error.code, ErrorCode::InvalidParams);
            assert_eq!(error.message, "view_count must be between 1 and 20");
            assert!(!world.contains_resource::<PendingTurnarounds>());
        }
    }

    #[test]
    fn push_pending_turnaround_accepts_boundary_view_counts() {
        for view_count in [1, MAX_TURNAROUND_VIEWS] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            let mut params = turnaround_params(None);
            params.view_count = Some(view_count);
            push_pending_turnaround(params, tx, &mut world);

            assert!(rx.try_recv().is_err(), "response should remain deferred");
            assert_eq!(world.resource::<PendingTurnarounds>().active.len(), 1);
        }
    }
}
