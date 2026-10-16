use bevy::{
    ecs::world::World,
    prelude::Resource,
    time::{Time, Virtual},
};
use pybevy_core::{
    PendingReloadRequest, ReloadRequestMode, ReloadResult,
    gpu_validation::{WgpuValidationEntry, WgpuValidationErrorState},
    public_error::{WGPU_VALIDATION_FRAME_HINT, reload_error_hint},
};
use pybevy_ecs::shared::system_runtime::HotReloadGeneration;
use tokio::sync::oneshot;

use crate::{
    capture::{
        hold::{GizmoEnabledRestore, prepare_pending_screenshot_gizmos},
        queue::{
            CaptureResponseKind, DEFAULT_SCREENSHOT_MAX_WIDTH, DebugCameraRequest,
            PendingScreenshot, PendingScreenshots,
        },
        readiness::RenderFrameReadiness,
    },
    protocol::{ControlError, ReloadMode},
    scene::entity_count::scene_entity_count,
};

/// Frames a requested reload runs before its response is built.
const RELOAD_SETTLE_FRAMES: u32 = 5;

/// Bevy resource for reloads whose response waits until the reload completes.
#[derive(Resource, Default)]
pub struct PendingReloads {
    pub pending: Vec<PendingReload>,
}

pub struct PendingReload {
    pub response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    pub mode: ReloadMode,
    pub frames_remaining: u32,
    /// The countdown expired while a definition fetch was still running; a
    /// fresh grace countdown starts when the fetch finishes.
    pub awaiting_fetch: bool,
    /// Backstop for a fetch flag that never clears.
    pub fetch_deadline: Option<std::time::Instant>,
    pub on_complete: ReloadCompletion,
}

impl PendingReload {
    pub fn new(
        response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
        mode: ReloadMode,
        on_complete: ReloadCompletion,
    ) -> Self {
        Self {
            response_tx,
            mode,
            frames_remaining: RELOAD_SETTLE_FRAMES,
            awaiting_fetch: false,
            fetch_deadline: None,
            on_complete,
        }
    }
}

/// What a completed reload sends back.
pub enum ReloadCompletion {
    /// The reload report.
    Respond,
    /// A screenshot whose response carries the reload report.
    Capture(ReloadCapture),
}

pub struct ReloadCapture {
    pub delay_frames: u32,
    pub max_width: Option<u32>,
    pub position: Option<[f32; 3]>,
    pub look_at: Option<[f32; 3]>,
    pub hide_ui: bool,
}

/// Trigger a hot reload (full or partial), optionally setting time control atomically
pub fn trigger_reload(
    world: &mut World,
    mode: ReloadMode,
    pause: bool,
    time_scale: Option<f32>,
) -> Result<serde_json::Value, ControlError> {
    let reload_mode = match mode {
        ReloadMode::Partial => ReloadRequestMode::Partial,
        ReloadMode::Full => ReloadRequestMode::Full,
    };

    // Apply time control BEFORE queuing reload so no frames run at wrong speed.
    // Validate first: set_relative_speed panics on <= 0.0 / non-finite and
    // spirals the fixed-timestep loop on very large values.
    if let Some(scale) = time_scale {
        crate::scene::time::validate_time_scale(scale)?;
        let mut time = world.resource_mut::<Time<Virtual>>();
        time.set_relative_speed(scale);
    }
    if pause {
        let mut time = world.resource_mut::<Time<Virtual>>();
        time.pause();
    }

    // Clear stale errors so post-reload checks don't see pre-reload errors
    if let Some(mut last_error) = world.get_resource_mut::<pybevy_core::LastSystemError>() {
        last_error.error = None;
        last_error.traceback = None;
    }

    let mut request = world.get_resource_or_insert_with(PendingReloadRequest::default);
    request.mode = Some(reload_mode);

    let time = world.resource::<Time<Virtual>>();
    let mut response = serde_json::json!({
        "status": "reload_requested",
        "mode": mode,
        "paused": time.is_paused(),
        "relative_speed": time.relative_speed(),
    });

    // Note: if partial was requested, it may auto-escalate to full during execution.
    // Check get_reload_status after reload completes to see if escalation occurred.
    if reload_mode == ReloadRequestMode::Partial {
        response["note"] = serde_json::json!(
            "Partial reload requested. May auto-escalate to Full if structural changes are detected \
             (new/removed components, resources, plugins, or Startup systems). \
             Check get_reload_status for escalation details including the specific reason."
        );
    }

    Ok(response)
}

/// Get current reload status
pub fn get_reload_status(world: &mut World) -> Result<serde_json::Value, ControlError> {
    let mut result = serde_json::Map::new();

    // Check if a reload request is pending
    let pending = world
        .get_resource::<PendingReloadRequest>()
        .and_then(|r| r.mode)
        .is_some();
    result.insert("pending_request".into(), serde_json::json!(pending));

    if let Some(generation) = world.get_resource::<HotReloadGeneration>() {
        result.insert("generation".into(), serde_json::json!(generation.current));
    }

    if world
        .get_resource::<bevy::ecs::schedule::Schedules>()
        .is_some()
    {
        result.insert("app_running".into(), serde_json::json!(true));
    }

    // Include last reload result (escalation info)
    if let Some(reload_result) = world.get_resource::<ReloadResult>() {
        if reload_result.escalated {
            result.insert("escalated".into(), serde_json::json!(true));
            if let Some(ref reason) = reload_result.escalation_reason {
                result.insert("escalation_reason".into(), serde_json::json!(reason));
            }
        }
        if let Some(actual_mode) = reload_result.actual_mode {
            let mode_str = match actual_mode {
                ReloadRequestMode::Full => "full",
                ReloadRequestMode::Partial => "partial",
            };
            result.insert("actual_mode".into(), serde_json::json!(mode_str));
        }
        if reload_result.failed {
            result.insert("failed".into(), serde_json::json!(true));
            if let Some(ref reason) = reload_result.failure_reason {
                result.insert("failure_reason".into(), serde_json::json!(reason));
            }
            if let Some(ref traceback) = reload_result.failure_traceback {
                result.insert("traceback".into(), serde_json::json!(traceback));
            }
            result.insert(
                "running_previous_generation".into(),
                serde_json::json!(reload_result.running_previous_generation),
            );
        }
        if let Some(ref added) = reload_result.plugins_added {
            result.insert("plugins_added".into(), serde_json::json!(added));
        }
        if let Some(ref removed) = reload_result.plugins_removed {
            result.insert("plugins_removed".into(), serde_json::json!(removed));
        }
        if let Some(ref removed) = reload_result.systems_removed {
            result.insert("systems_removed".into(), serde_json::json!(removed));
        }
        if reload_result.definition_fetch_in_progress {
            result.insert(
                "definition_fetch_in_progress".into(),
                serde_json::json!(true),
            );
        }
    }

    Ok(serde_json::Value::Object(result))
}

/// Get the last Python system error and any recorded wgpu validation errors.
pub fn get_last_error(world: &mut World) -> Result<serde_json::Value, ControlError> {
    let mut response = last_python_error(world);
    response["wgpu_validation"] = wgpu_validation_json(world);
    Ok(response)
}

fn wgpu_validation_entry_json(entry: &WgpuValidationEntry) -> serde_json::Value {
    serde_json::json!({
        "message": entry.message,
        "label": entry.label,
        "is_pipeline": entry.is_pipeline,
    })
}

fn wgpu_validation_json(world: &World) -> serde_json::Value {
    let Some(summary) = world
        .get_resource::<WgpuValidationErrorState>()
        .and_then(WgpuValidationErrorState::summary)
    else {
        return serde_json::Value::Null;
    };
    serde_json::json!({
        "count": summary.count,
        "first": wgpu_validation_entry_json(&summary.first),
        "last": wgpu_validation_entry_json(&summary.last),
        "last_seen_secs_ago": summary.last_seen_ago.as_secs_f64(),
        "hint": WGPU_VALIDATION_FRAME_HINT,
    })
}

fn last_python_error(world: &World) -> serde_json::Value {
    let reload_failure = world
        .get_resource::<ReloadResult>()
        .filter(|result| result.failed && result.failure_reason.is_some());
    let live_error = world
        .get_resource::<pybevy_core::LastSystemError>()
        .filter(|error| error.error.is_some());

    if let Some(last_error) = live_error
        && reload_failure.is_none_or(|reload| last_error.sequence > reload.failure_sequence)
    {
        return serde_json::json!({
            "error": last_error.error,
            "traceback": last_error.traceback,
            "timestamp_secs": last_error.timestamp_secs,
        });
    }

    if let Some(reload_result) = reload_failure {
        return serde_json::json!({
            "error": reload_result.failure_reason,
            "traceback": reload_result.failure_traceback,
            "reload_failed": true,
            "running_previous_generation": reload_result.running_previous_generation,
        });
    }

    serde_json::json!({ "error": null })
}

/// Post-fetch frames granted so the resumed generation's first error can land
/// in `LastSystemError` before the response reads it.
const POST_FETCH_GRACE_FRAMES: u32 = 5;

/// Backstop for a stuck `definition_fetch_in_progress` flag. The fetch session
/// caps its own retries well below this; hitting it means the flag leaked.
const FETCH_RESPONSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

fn definition_fetch_in_progress(world: &World) -> bool {
    world
        .get_resource::<ReloadResult>()
        .is_some_and(|result| result.definition_fetch_in_progress)
}

fn add_error_hint(response: &mut serde_json::Value) {
    let Some(hint) = response
        .get("error")
        .and_then(serde_json::Value::as_str)
        .and_then(reload_error_hint)
    else {
        return;
    };
    response["hint"] = serde_json::json!(hint);
}

/// Adds the completed reload's system error, registration failure and
/// escalation to `response`, and returns whether it reported an error.
/// `system_error_status` replaces the status when a system error is present.
fn add_reload_outcome(
    world: &World,
    response: &mut serde_json::Value,
    system_error_status: Option<&str>,
) -> bool {
    let mut has_error = false;
    if let Some(last_error) = world.get_resource::<pybevy_core::LastSystemError>()
        && let Some(ref error_msg) = last_error.error
    {
        if let Some(status) = system_error_status {
            response["status"] = serde_json::json!(status);
        }
        response["error"] = serde_json::json!(error_msg);
        response["traceback"] = serde_json::json!(last_error.traceback);
        has_error = true;
    }

    // Registration and Startup failures flagged on ReloadResult take precedence.
    if let Some(reload_result) = world.get_resource::<ReloadResult>() {
        if reload_result.failed {
            response["status"] = serde_json::json!("reload_failed");
            response["error"] = serde_json::json!(
                reload_result
                    .failure_reason
                    .as_deref()
                    .unwrap_or("unknown registration failure")
            );
            response["traceback"] = serde_json::json!(reload_result.failure_traceback);
            if let Some(ref reason) = reload_result.failure_reason {
                response["failure_reason"] = serde_json::json!(reason);
            }
            has_error = true;
        }
        response["escalated"] = serde_json::json!(reload_result.escalated);
        if let Some(ref reason) = reload_result.escalation_reason {
            response["escalation_reason"] = serde_json::json!(reason);
        }
    }
    has_error
}

/// Process pending reloads (called each frame in the Last schedule).
/// Counts down frames, holds while a definition fetch is still running, then
/// responds with the reload report or queues the requested capture.
pub fn process_pending_reloads(world: &mut World) {
    let Some(mut pending) = world.remove_resource::<PendingReloads>() else {
        return;
    };

    if pending.pending.is_empty() {
        world.insert_resource(pending);
        return;
    }

    let fetching = definition_fetch_in_progress(world);
    let mut still_waiting = Vec::new();

    for mut reload in pending.pending.drain(..) {
        if reload.response_tx.is_closed() {
            continue;
        }
        if reload.frames_remaining > 0 {
            reload.frames_remaining -= 1;
            still_waiting.push(reload);
        } else if fetching {
            let deadline = *reload
                .fetch_deadline
                .get_or_insert_with(|| std::time::Instant::now() + FETCH_RESPONSE_TIMEOUT);
            if std::time::Instant::now() < deadline {
                reload.awaiting_fetch = true;
                still_waiting.push(reload);
            } else {
                respond_fetch_timeout(world, reload);
            }
        } else if reload.awaiting_fetch {
            // The fetch just finished; let the resumed attempt's generation
            // run before reading its error state.
            reload.awaiting_fetch = false;
            reload.frames_remaining = POST_FETCH_GRACE_FRAMES;
            still_waiting.push(reload);
        } else {
            complete_reload(world, reload);
        }
    }

    pending.pending = still_waiting;
    world.insert_resource(pending);
}

fn respond_fetch_timeout(world: &mut World, reload: PendingReload) {
    let reason = "definition-load fetch did not finish within the response window";
    let report = serde_json::json!({
        "status": "reload_failed",
        "mode": reload.mode,
        "error": reason,
        "failure_reason": reason,
    });
    let response = match reload.on_complete {
        ReloadCompletion::Respond => report,
        ReloadCompletion::Capture(_) => serde_json::json!({
            "reload": report,
            "errors": reason,
            "screenshot": null,
            "entity_count": scene_entity_count(world) as usize,
        }),
    };
    let _ = reload.response_tx.send(Ok(response));
}

fn complete_reload(world: &mut World, reload: PendingReload) {
    let time = world.resource::<Time<Virtual>>();
    let (paused, relative_speed) = (time.is_paused(), time.relative_speed());
    let capture = match reload.on_complete {
        ReloadCompletion::Respond => {
            let mut response = serde_json::json!({
                "status": "reload_completed",
                "mode": reload.mode,
                "paused": paused,
                "relative_speed": relative_speed,
            });
            add_reload_outcome(world, &mut response, None);
            if response.get("error").is_none() {
                response["error"] = serde_json::json!(null);
            }
            add_error_hint(&mut response);
            let _ = reload.response_tx.send(Ok(response));
            return;
        }
        ReloadCompletion::Capture(capture) => capture,
    };

    let mut reload_response = serde_json::json!({
        "mode": reload.mode,
        "status": "success",
        "paused": paused,
        "relative_speed": relative_speed,
    });
    let has_error = add_reload_outcome(world, &mut reload_response, Some("error"));
    add_error_hint(&mut reload_response);
    let entity_count = scene_entity_count(world) as usize;

    if has_error {
        // Error during reload - respond immediately without screenshot
        let response = serde_json::json!({
            "reload": reload_response,
            "errors": reload_response.get("error"),
            "screenshot": null,
            "entity_count": entity_count,
        });
        let _ = reload.response_tx.send(Ok(response));
        return;
    }

    let debug_camera = capture.position.map(|pos| DebugCameraRequest {
        position: pos,
        look_at: capture.look_at.unwrap_or([0.0, 0.0, 0.0]),
    });
    let mut screenshot = PendingScreenshot {
        response_tx: reload.response_tx,
        frames_remaining: capture.delay_frames,
        required_render_epoch: world
            .get_resource::<RenderFrameReadiness>()
            .map(RenderFrameReadiness::request_frame),
        with_gizmos: false,
        gizmo_restore: GizmoEnabledRestore::new(),
        max_width: capture.max_width.or(Some(DEFAULT_SCREENSHOT_MAX_WIDTH)),
        debug_camera,
        hide_ui: capture.hide_ui,
        entity: None,
        depth_analysis: None,
        response_kind: CaptureResponseKind::ReloadAndCapture,
        extra_response: Some(serde_json::json!({
            "reload": reload_response,
            "errors": null,
            "entity_count": entity_count,
        })),
    };
    prepare_pending_screenshot_gizmos(world, &mut screenshot);
    let mut screenshots = world.get_resource_or_insert_with(PendingScreenshots::default);
    screenshots.pending.push(screenshot);
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicU32};

    use pybevy_core::{
        LastSystemError, PendingReloadRequest, ReloadRequestMode, ReloadResult,
        public_error::{
            RELOAD_ATTRIBUTE_ERROR_HINT, RELOAD_CONSTRUCTOR_TYPE_ERROR_HINT, RELOAD_QUERY_ROW_HINT,
        },
    };

    use super::*;
    use crate::capture::screenshot::process_pending_screenshots;

    #[test]
    fn get_reload_status_empty_world() {
        let mut world = World::new();
        let result = get_reload_status(&mut world).unwrap();
        assert_eq!(result["pending_request"], false);
    }

    #[test]
    fn get_reload_status_with_pending() {
        let mut world = World::new();
        world.insert_resource(PendingReloadRequest {
            mode: Some(ReloadRequestMode::Full),
        });
        let result = get_reload_status(&mut world).unwrap();
        assert_eq!(result["pending_request"], true);
    }

    #[test]
    fn get_reload_status_reports_current_generation() {
        let mut world = World::new();
        world.insert_resource(HotReloadGeneration::new(Arc::new(AtomicU32::new(7))));

        let result = get_reload_status(&mut world).unwrap();

        assert_eq!(result["generation"], 7);
    }

    #[test]
    fn get_reload_status_with_escalation() {
        let mut world = World::new();
        world.insert_resource(ReloadResult {
            escalated: true,
            escalation_reason: Some("structural changes detected".to_string()),
            actual_mode: Some(ReloadRequestMode::Full),
            failed: false,
            failure_reason: None,
            failure_traceback: None,
            failure_sequence: 0,
            running_previous_generation: false,
            plugins_added: None,
            plugins_removed: None,
            systems_removed: None,
            definition_fetch_in_progress: false,
        });
        let result = get_reload_status(&mut world).unwrap();
        assert_eq!(result["escalated"], true);
        assert_eq!(result["escalation_reason"], "structural changes detected");
        assert_eq!(result["actual_mode"], "full");
    }

    #[test]
    fn get_reload_status_with_systems_removed() {
        let mut world = World::new();
        world.insert_resource(ReloadResult {
            escalated: false,
            escalation_reason: None,
            actual_mode: Some(ReloadRequestMode::Full),
            failed: false,
            failure_reason: None,
            failure_traceback: None,
            failure_sequence: 0,
            running_previous_generation: false,
            plugins_added: None,
            plugins_removed: None,
            systems_removed: Some(vec!["update_scoreboard".to_string()]),
            definition_fetch_in_progress: false,
        });
        let result = get_reload_status(&mut world).unwrap();
        let removed = result["systems_removed"].as_array().unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], "update_scoreboard");
    }

    #[test]
    fn get_reload_status_systems_removed_absent_when_none() {
        let mut world = World::new();
        world.insert_resource(ReloadResult {
            systems_removed: None,
            definition_fetch_in_progress: false,
            ..ReloadResult::default()
        });
        let result = get_reload_status(&mut world).unwrap();
        assert!(result.get("systems_removed").is_none());
    }

    #[test]
    fn get_last_error_no_resource() {
        let mut world = World::new();
        let result = get_last_error(&mut world).unwrap();
        assert!(result["error"].is_null());
    }

    #[test]
    fn get_last_error_no_error() {
        let mut world = World::new();
        world.insert_resource(LastSystemError {
            error: None,
            traceback: None,
            timestamp_secs: 0.0,
            ..LastSystemError::default()
        });
        let result = get_last_error(&mut world).unwrap();
        assert!(result["error"].is_null());
    }

    #[test]
    fn get_last_error_with_error() {
        let mut world = World::new();
        world.insert_resource(LastSystemError {
            error: Some("boom".to_string()),
            traceback: Some("at line 1".to_string()),
            timestamp_secs: 42.0,
            ..LastSystemError::default()
        });
        let result = get_last_error(&mut world).unwrap();
        assert_eq!(result["error"], "boom");
        assert_eq!(result["traceback"], "at line 1");
        assert_eq!(result["timestamp_secs"], 42.0);
    }

    #[test]
    fn get_last_error_prioritizes_reload_failure() {
        let mut world = World::new();
        pybevy_core::publish_last_system_error(
            &mut world,
            "downstream error from the previous generation".to_string(),
            Some("stale traceback".to_string()),
            42.0,
        );
        pybevy_core::publish_reload_failure(
            &mut world,
            "conflicting component access".to_string(),
            Some("  File \"scene.py\", line 17, in broken_system".to_string()),
            true,
        );

        let result = get_last_error(&mut world).unwrap();

        assert_eq!(result["error"], "conflicting component access");
        assert_eq!(
            result["traceback"],
            "  File \"scene.py\", line 17, in broken_system"
        );
        assert_eq!(result["reload_failed"], true);
        assert_eq!(result["running_previous_generation"], true);
    }

    #[test]
    fn get_last_error_prioritizes_newer_live_system_error() {
        let mut world = World::new();
        pybevy_core::publish_reload_failure(
            &mut world,
            "Startup failed".to_string(),
            Some("startup traceback".to_string()),
            false,
        );
        pybevy_core::publish_last_system_error(
            &mut world,
            "runtime failed".to_string(),
            Some("runtime traceback".to_string()),
            0.0,
        );

        let result = get_last_error(&mut world).unwrap();

        assert_eq!(result["error"], "runtime failed");
        assert_eq!(result["traceback"], "runtime traceback");
        assert!(result.get("reload_failed").is_none());
    }

    #[test]
    fn get_last_error_uses_system_error_after_successful_reload() {
        let mut world = World::new();
        world.insert_resource(LastSystemError {
            error: Some("current system error".to_string()),
            traceback: None,
            timestamp_secs: 1.0,
            ..LastSystemError::default()
        });
        world.insert_resource(ReloadResult {
            failed: false,
            failure_reason: Some("obsolete failure".to_string()),
            ..ReloadResult::default()
        });

        let result = get_last_error(&mut world).unwrap();

        assert_eq!(result["error"], "current system error");
        assert!(result.get("reload_failed").is_none());
    }

    #[test]
    fn get_last_error_reports_wgpu_validation_beside_python_error() {
        let mut world = World::new();
        let state = WgpuValidationErrorState::default();
        world.insert_resource(state.clone());
        assert!(get_last_error(&mut world).unwrap()["wgpu_validation"].is_null());

        state.record(&WgpuValidationEntry::from_message(
            "In Device::create_render_pipeline, label = 'pbr_prepass_pipeline'".to_string(),
        ));
        state.record(&WgpuValidationEntry::from_message(
            "In a set_pipeline command\n  RenderPipeline with 'pbr_prepass_pipeline' label is invalid"
                .to_string(),
        ));
        let result = get_last_error(&mut world).unwrap();

        assert!(result["error"].is_null());
        let wgpu = &result["wgpu_validation"];
        assert_eq!(wgpu["count"], 2);
        assert_eq!(wgpu["first"]["label"], "pbr_prepass_pipeline");
        assert_eq!(wgpu["first"]["is_pipeline"], true);
        assert!(
            wgpu["last"]["message"]
                .as_str()
                .unwrap()
                .contains("label is invalid")
        );
        assert_eq!(wgpu["hint"], WGPU_VALIDATION_FRAME_HINT);
        assert!(wgpu["last_seen_secs_ago"].as_f64().unwrap() >= 0.0);

        state.clear();
        assert!(get_last_error(&mut world).unwrap()["wgpu_validation"].is_null());
    }

    fn world_with_reload_deps() -> World {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world
    }

    #[test]
    fn trigger_reload_full_mode() {
        let mut world = world_with_reload_deps();
        let result = trigger_reload(&mut world, ReloadMode::Full, false, None).unwrap();
        assert_eq!(result["status"], "reload_requested");
        assert_eq!(result["mode"], "full");
        assert_eq!(result["paused"], false);
        // PendingReloadRequest should be inserted
        let req = world.get_resource::<PendingReloadRequest>().unwrap();
        assert!(req.mode.is_some());
    }

    #[test]
    fn trigger_reload_partial_mode_has_note() {
        let mut world = world_with_reload_deps();
        let result = trigger_reload(&mut world, ReloadMode::Partial, false, None).unwrap();
        assert_eq!(result["mode"], "partial");
        let note = result["note"].as_str().unwrap();
        assert!(note.contains("auto-escalate"));
        assert!(note.contains("components"));
        assert!(note.contains("plugins"));
        assert!(note.contains("escalation details"));
    }

    #[test]
    fn trigger_reload_with_pause() {
        let mut world = world_with_reload_deps();
        let result = trigger_reload(&mut world, ReloadMode::Full, true, None).unwrap();
        assert_eq!(result["paused"], true);
        let time = world.resource::<Time<Virtual>>();
        assert!(time.is_paused());
    }

    #[test]
    fn trigger_reload_with_time_scale() {
        let mut world = world_with_reload_deps();
        let result = trigger_reload(&mut world, ReloadMode::Full, false, Some(0.5)).unwrap();
        assert_eq!(result["relative_speed"], 0.5);
        let time = world.resource::<Time<Virtual>>();
        assert!((time.relative_speed() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn trigger_reload_with_pause_and_scale() {
        let mut world = world_with_reload_deps();
        let result = trigger_reload(&mut world, ReloadMode::Full, true, Some(2.0)).unwrap();
        assert_eq!(result["paused"], true);
        assert_eq!(result["relative_speed"], 2.0);
    }

    #[test]
    fn trigger_reload_rejects_out_of_range_time_scale() {
        // Regression: the reload time_scale param reached set_relative_speed with
        // no validation at all. A negative/non-finite value panicked the
        // subprocess and a huge value spiraled the fixed-timestep loop. Both are
        // now rejected before any state changes and before the reload is queued.
        for bad in [-1.0f32, f32::INFINITY, 1.0e6] {
            let mut world = world_with_reload_deps();
            let err = trigger_reload(&mut world, ReloadMode::Full, false, Some(bad)).unwrap_err();
            assert!(!err.message.is_empty());
            // Speed untouched and no reload queued.
            let speed = world.resource::<Time<Virtual>>().relative_speed();
            assert!((speed - 1.0).abs() < 1e-6);
            assert!(world.get_resource::<PendingReloadRequest>().is_none());
        }
    }

    #[test]
    fn trigger_reload_clears_stale_errors() {
        let mut world = world_with_reload_deps();
        world.insert_resource(pybevy_core::LastSystemError {
            error: Some("old error".into()),
            traceback: Some("old trace".into()),
            timestamp_secs: 1.0,
            ..Default::default()
        });
        trigger_reload(&mut world, ReloadMode::Full, false, None).unwrap();
        let err = world
            .get_resource::<pybevy_core::LastSystemError>()
            .unwrap();
        assert!(err.error.is_none());
        assert!(err.traceback.is_none());
    }

    #[test]
    fn reload_mode_deserialize_rejects_unknown() {
        let result: Result<ReloadMode, _> =
            serde_json::from_value(serde_json::json!("unknown_mode"));
        assert!(result.is_err());
    }

    #[test]
    fn process_pending_reloads_no_resource() {
        let mut world = World::new();
        // Should not panic
        super::process_pending_reloads(&mut world);
    }

    #[test]
    fn process_pending_reloads_empty() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world.insert_resource(PendingReloads::default());
        process_pending_reloads(&mut world);
        // Should still have the resource
        assert!(world.get_resource::<PendingReloads>().is_some());
    }

    #[test]
    fn process_pending_reloads_countdown() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloads {
            pending: vec![PendingReload {
                response_tx: tx,
                frames_remaining: 3,
                mode: ReloadMode::Full,
                awaiting_fetch: false,
                fetch_deadline: None,
                on_complete: ReloadCompletion::Respond,
            }],
        });
        process_pending_reloads(&mut world);
        let pending = world.get_resource::<PendingReloads>().unwrap();
        assert_eq!(pending.pending.len(), 1);
        assert_eq!(pending.pending[0].frames_remaining, 2);
    }

    #[test]
    fn process_pending_reloads_sends_response_when_ready() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloads {
            pending: vec![PendingReload {
                response_tx: tx,
                frames_remaining: 0,
                mode: ReloadMode::Full,
                awaiting_fetch: false,
                fetch_deadline: None,
                on_complete: ReloadCompletion::Respond,
            }],
        });
        process_pending_reloads(&mut world);
        // Response should have been sent
        let result = rx.try_recv().unwrap().unwrap();
        assert_eq!(result["status"], "reload_completed");
        assert_eq!(result["mode"], "full");
        // Pending should be empty now
        let pending = world.get_resource::<PendingReloads>().unwrap();
        assert!(pending.pending.is_empty());
    }

    #[test]
    fn process_pending_reloads_holds_while_definition_fetch_runs() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world.insert_resource(ReloadResult {
            definition_fetch_in_progress: true,
            ..Default::default()
        });
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloads {
            pending: vec![PendingReload {
                response_tx: tx,
                frames_remaining: 0,
                mode: ReloadMode::Full,
                awaiting_fetch: false,
                fetch_deadline: None,
                on_complete: ReloadCompletion::Respond,
            }],
        });

        process_pending_reloads(&mut world);
        assert!(rx.try_recv().is_err(), "response sent while fetch pending");
        {
            let pending = world.get_resource::<PendingReloads>().unwrap();
            assert_eq!(pending.pending.len(), 1);
            assert!(pending.pending[0].awaiting_fetch);
        }

        // Fetch finishes: a fresh grace countdown starts before the response.
        world
            .resource_mut::<ReloadResult>()
            .definition_fetch_in_progress = false;
        process_pending_reloads(&mut world);
        assert!(rx.try_recv().is_err(), "response sent without grace frames");
        {
            let pending = world.get_resource::<PendingReloads>().unwrap();
            assert_eq!(pending.pending[0].frames_remaining, POST_FETCH_GRACE_FRAMES);
        }

        for _ in 0..=POST_FETCH_GRACE_FRAMES {
            process_pending_reloads(&mut world);
        }
        let result = rx.try_recv().unwrap().unwrap();
        assert_eq!(result["status"], "reload_completed");
    }

    #[test]
    fn process_pending_reloads_times_out_a_stuck_fetch() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world.insert_resource(ReloadResult {
            definition_fetch_in_progress: true,
            ..Default::default()
        });
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloads {
            pending: vec![PendingReload {
                response_tx: tx,
                frames_remaining: 0,
                mode: ReloadMode::Full,
                awaiting_fetch: true,
                fetch_deadline: Some(std::time::Instant::now() - FETCH_RESPONSE_TIMEOUT),
                on_complete: ReloadCompletion::Respond,
            }],
        });

        process_pending_reloads(&mut world);
        let result = rx.try_recv().unwrap().unwrap();
        assert_eq!(result["status"], "reload_failed");
        assert!(
            result["error"]
                .as_str()
                .unwrap()
                .contains("did not finish within the response window"),
            "{result}"
        );
    }

    #[test]
    fn process_pending_reloads_includes_error_if_new() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        // Insert an error that happened AFTER the reload was triggered
        world.insert_resource(pybevy_core::LastSystemError {
            error: Some(
                "TypeError: cannot unpack non-iterable pybevy.transform.Transform object".into(),
            ),
            traceback: Some("line 42".into()),
            timestamp_secs: 5.0,
            ..Default::default()
        });
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloads {
            pending: vec![PendingReload {
                response_tx: tx,
                frames_remaining: 0,
                mode: ReloadMode::Full,
                awaiting_fetch: false,
                fetch_deadline: None,
                on_complete: ReloadCompletion::Respond,
            }],
        });
        process_pending_reloads(&mut world);
        let result = rx.try_recv().unwrap().unwrap();
        assert_eq!(
            result["error"],
            "TypeError: cannot unpack non-iterable pybevy.transform.Transform object"
        );
        assert_eq!(result["traceback"], "line 42");
        assert_eq!(result["hint"], RELOAD_QUERY_ROW_HINT);
    }

    #[test]
    fn process_pending_reloads_surfaces_reload_result_failed() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world.insert_resource(pybevy_core::LastSystemError {
            error: Some(
                "TypeError: cannot unpack non-iterable pybevy.transform.Transform object".into(),
            ),
            traceback: Some("stale traceback".into()),
            timestamp_secs: 5.0,
            ..Default::default()
        });
        world.insert_resource(ReloadResult {
            failed: true,
            failure_reason: Some("AttributeError: 'Foo' object has no attribute 'bar'".into()),
            failure_traceback: Some("Traceback: scene.py:42".into()),
            running_previous_generation: true,
            ..ReloadResult::default()
        });
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloads {
            pending: vec![PendingReload {
                response_tx: tx,
                frames_remaining: 0,
                mode: ReloadMode::Full,
                awaiting_fetch: false,
                fetch_deadline: None,
                on_complete: ReloadCompletion::Respond,
            }],
        });
        process_pending_reloads(&mut world);
        let result = rx.try_recv().unwrap().unwrap();
        assert_eq!(result["status"], "reload_failed");
        assert_eq!(
            result["error"],
            "AttributeError: 'Foo' object has no attribute 'bar'"
        );
        assert_eq!(result["traceback"], "Traceback: scene.py:42");
        assert_eq!(
            result["failure_reason"],
            "AttributeError: 'Foo' object has no attribute 'bar'"
        );
        assert_eq!(result["hint"], RELOAD_ATTRIBUTE_ERROR_HINT);
    }

    #[test]
    fn reload_capture_requests_a_fresh_render_epoch_before_counting_capture_frames() {
        for mode in [ReloadMode::Full, ReloadMode::Partial] {
            let mut world = World::new();
            world.init_resource::<Time<Virtual>>();
            let readiness = RenderFrameReadiness::default();
            let previous_epoch = readiness.request_frame();
            world.insert_resource(readiness.clone());
            let (tx, mut rx) = oneshot::channel();
            world.insert_resource(PendingReloads {
                pending: vec![PendingReload {
                    response_tx: tx,
                    mode,
                    frames_remaining: 1,
                    awaiting_fetch: false,
                    fetch_deadline: None,
                    on_complete: ReloadCompletion::Capture(ReloadCapture {
                        delay_frames: 2,
                        max_width: None,
                        position: None,
                        look_at: None,
                        hide_ui: false,
                    }),
                }],
            });

            process_pending_reloads(&mut world);
            assert_eq!(readiness.requested_epoch(), previous_epoch);
            assert!(!world.contains_resource::<PendingScreenshots>());

            process_pending_reloads(&mut world);
            assert_eq!(readiness.requested_epoch(), previous_epoch + 1);
            assert!(world.resource::<PendingReloads>().pending.is_empty());
            let pending = world.resource::<PendingScreenshots>();
            assert_eq!(pending.pending.len(), 1);
            assert_eq!(
                pending.pending[0].required_render_epoch,
                Some(previous_epoch + 1)
            );

            for _ in 0..10 {
                process_pending_screenshots(&mut world);
                assert!(matches!(
                    rx.try_recv(),
                    Err(oneshot::error::TryRecvError::Empty)
                ));
                let pending = world.resource::<PendingScreenshots>();
                assert_eq!(pending.pending.len(), 1);
                assert_eq!(pending.pending[0].frames_remaining, 2);
            }
        }
    }

    #[test]
    fn reload_capture_without_a_renderer_keeps_the_capture_countdown() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        let (tx, _rx) = oneshot::channel();
        world.insert_resource(PendingReloads {
            pending: vec![PendingReload {
                response_tx: tx,
                mode: ReloadMode::Full,
                frames_remaining: 0,
                awaiting_fetch: false,
                fetch_deadline: None,
                on_complete: ReloadCompletion::Capture(ReloadCapture {
                    delay_frames: 2,
                    max_width: None,
                    position: None,
                    look_at: None,
                    hide_ui: false,
                }),
            }],
        });

        process_pending_reloads(&mut world);
        assert!(!world.contains_resource::<RenderFrameReadiness>());
        let pending = world.resource::<PendingScreenshots>();
        assert_eq!(pending.pending.len(), 1);
        assert_eq!(pending.pending[0].required_render_epoch, None);
        assert!(matches!(
            &pending.pending[0].response_kind,
            CaptureResponseKind::ReloadAndCapture
        ));
        process_pending_screenshots(&mut world);
        assert_eq!(
            world.resource::<PendingScreenshots>().pending[0].frames_remaining,
            1
        );
    }

    #[test]
    fn reload_and_capture_prioritizes_reload_failure() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world.insert_resource(pybevy_core::LastSystemError {
            error: Some(
                "TypeError: cannot unpack non-iterable pybevy.transform.Transform object".into(),
            ),
            traceback: Some("stale traceback".into()),
            timestamp_secs: 5.0,
            ..Default::default()
        });
        world.insert_resource(ReloadResult {
            failed: true,
            failure_reason: Some(
                "TypeError: Transform.__new__() takes 0 positional arguments but 1 was given"
                    .into(),
            ),
            failure_traceback: Some("registration traceback".into()),
            running_previous_generation: true,
            ..ReloadResult::default()
        });
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloads {
            pending: vec![PendingReload {
                response_tx: tx,
                mode: ReloadMode::Full,
                frames_remaining: 0,
                awaiting_fetch: false,
                fetch_deadline: None,
                on_complete: ReloadCompletion::Capture(ReloadCapture {
                    delay_frames: 0,
                    max_width: None,
                    position: None,
                    look_at: None,
                    hide_ui: true,
                }),
            }],
        });

        process_pending_reloads(&mut world);
        let result = rx.try_recv().unwrap().unwrap();
        assert_eq!(result["reload"]["status"], "reload_failed");
        assert_eq!(
            result["reload"]["error"],
            "TypeError: Transform.__new__() takes 0 positional arguments but 1 was given"
        );
        assert_eq!(result["reload"]["traceback"], "registration traceback");
        assert_eq!(result["reload"]["hint"], RELOAD_CONSTRUCTOR_TYPE_ERROR_HINT);
        assert_eq!(
            result["errors"],
            "TypeError: Transform.__new__() takes 0 positional arguments but 1 was given"
        );
        assert!(result["screenshot"].is_null());
    }

    #[test]
    fn trigger_reload_clears_stale_errors_so_they_are_not_reported() {
        // Staleness is prevented by clearing the slot when the reload is
        // requested, not by comparing timestamps: the clock resets to 0 on a
        // full reload, so a pre-reload timestamp is not comparable afterwards.
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world.insert_resource(pybevy_core::LastSystemError {
            error: Some("old error".into()),
            traceback: None,
            timestamp_secs: 0.5,
            ..Default::default()
        });

        trigger_reload(&mut world, ReloadMode::Full, false, None).unwrap();
        assert!(
            world
                .resource::<pybevy_core::LastSystemError>()
                .error
                .is_none(),
            "reload request must clear the error slot"
        );

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloads {
            pending: vec![PendingReload {
                response_tx: tx,
                frames_remaining: 0,
                mode: ReloadMode::Full,
                awaiting_fetch: false,
                fetch_deadline: None,
                on_complete: ReloadCompletion::Respond,
            }],
        });
        process_pending_reloads(&mut world);
        let result = rx.try_recv().unwrap().unwrap();
        assert!(result["error"].is_null());
    }

    #[test]
    fn error_after_reload_is_reported_regardless_of_timestamp() {
        // The post-reload clock restarts near zero, so a new error can carry a
        // smaller timestamp than the pre-reload one it replaced.
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world.insert_resource(pybevy_core::LastSystemError {
            error: Some("old error".into()),
            traceback: None,
            timestamp_secs: 9.0,
            ..Default::default()
        });
        trigger_reload(&mut world, ReloadMode::Full, false, None).unwrap();

        let mut last = world.resource_mut::<pybevy_core::LastSystemError>();
        last.error = Some("RuntimeError: boom".into());
        last.timestamp_secs = 0.1;

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloads {
            pending: vec![PendingReload {
                response_tx: tx,
                frames_remaining: 0,
                mode: ReloadMode::Full,
                awaiting_fetch: false,
                fetch_deadline: None,
                on_complete: ReloadCompletion::Respond,
            }],
        });
        process_pending_reloads(&mut world);
        let result = rx.try_recv().unwrap().unwrap();
        assert_eq!(result["error"], "RuntimeError: boom");
    }

    fn capture_reload(
        response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    ) -> PendingReload {
        let mut reload = PendingReload::new(
            response_tx,
            ReloadMode::Full,
            ReloadCompletion::Capture(ReloadCapture {
                delay_frames: 0,
                max_width: None,
                position: None,
                look_at: None,
                hide_ui: true,
            }),
        );
        reload.frames_remaining = 0;
        reload
    }

    #[test]
    fn reload_and_capture_holds_a_running_fetch_under_a_deadline() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world.insert_resource(ReloadResult {
            definition_fetch_in_progress: true,
            ..Default::default()
        });
        let (tx, mut rx) = oneshot::channel();
        world.insert_resource(PendingReloads {
            pending: vec![capture_reload(tx)],
        });

        process_pending_reloads(&mut world);
        assert!(rx.try_recv().is_err(), "response sent while fetch pending");
        let pending = world.resource::<PendingReloads>();
        assert!(pending.pending[0].awaiting_fetch);
        assert!(pending.pending[0].fetch_deadline.is_some());
    }

    #[test]
    fn reload_and_capture_times_out_a_stuck_fetch() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world.insert_resource(ReloadResult {
            definition_fetch_in_progress: true,
            ..Default::default()
        });
        let (tx, mut rx) = oneshot::channel();
        let mut reload = capture_reload(tx);
        reload.awaiting_fetch = true;
        reload.fetch_deadline = Some(std::time::Instant::now() - FETCH_RESPONSE_TIMEOUT);
        world.insert_resource(PendingReloads {
            pending: vec![reload],
        });

        process_pending_reloads(&mut world);
        let result = rx.try_recv().unwrap().unwrap();
        let reason = "definition-load fetch did not finish within the response window";
        assert_eq!(result["reload"]["status"], "reload_failed");
        assert_eq!(result["reload"]["failure_reason"], reason);
        assert_eq!(result["errors"], reason);
        assert!(result["screenshot"].is_null());
        assert_eq!(result["entity_count"], 0);
        assert!(!world.contains_resource::<PendingScreenshots>());
        assert!(world.resource::<PendingReloads>().pending.is_empty());
    }

    #[test]
    fn closed_reload_responders_are_dropped() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        let (respond_tx, respond_rx) = oneshot::channel();
        let (capture_tx, capture_rx) = oneshot::channel();
        world.insert_resource(PendingReloads {
            pending: vec![
                PendingReload::new(respond_tx, ReloadMode::Full, ReloadCompletion::Respond),
                capture_reload(capture_tx),
            ],
        });
        drop((respond_rx, capture_rx));

        process_pending_reloads(&mut world);
        assert!(world.resource::<PendingReloads>().pending.is_empty());
        assert!(!world.contains_resource::<PendingScreenshots>());
    }
}
