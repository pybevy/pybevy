use bevy::{
    ecs::world::World,
    prelude::Resource,
    time::{Time, Virtual},
};
use pybevy_core::{PendingReloadRequest, ReloadRequestMode, ReloadResult};
use tokio::sync::oneshot;

use crate::bridge::{
    ControlError, DebugCameraRequest, PendingReloadResponses, PendingScreenshots, ReloadMode,
};

/// State machine for reload-and-capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadAndCaptureState {
    WaitingForReload,
    WaitingForScreenshot,
}

/// A pending reload-and-capture request.
pub struct PendingReloadAndCapture {
    pub response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    pub mode: ReloadMode,
    pub error_timestamp_before: f64,
    pub reload_frames_remaining: u32,
    pub screenshot_delay_frames: u32,
    pub max_width: Option<u32>,
    pub position: Option<[f32; 3]>,
    pub look_at: Option<[f32; 3]>,
    pub hide_ui: bool,
    pub state: ReloadAndCaptureState,
    pub reload_response: Option<serde_json::Value>,
}

/// Bevy resource for pending reload-and-capture responses.
#[derive(Resource, Default)]
pub struct PendingReloadAndCaptures {
    pub pending: Vec<PendingReloadAndCapture>,
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
        crate::handlers::time_control::validate_time_scale(scale)?;
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
    }

    Ok(serde_json::Value::Object(result))
}

/// Get the last Python system error
pub fn get_last_error(world: &mut World) -> Result<serde_json::Value, ControlError> {
    match world.get_resource::<pybevy_core::LastSystemError>() {
        Some(last_error) => match &last_error.error {
            Some(error) => Ok(serde_json::json!({
                "error": error,
                "traceback": last_error.traceback,
                "timestamp_secs": last_error.timestamp_secs,
            })),
            None => Ok(serde_json::json!({ "error": null })),
        },
        None => Ok(serde_json::json!({ "error": null })),
    }
}

/// Process pending reload responses (called each frame in Last schedule).
/// Counts down frames, then checks for errors/escalation and sends deferred response.
pub fn process_pending_reloads(world: &mut World) {
    let Some(mut pending) = world.remove_resource::<PendingReloadResponses>() else {
        return;
    };

    if pending.pending.is_empty() {
        world.insert_resource(pending);
        return;
    }

    let mut still_waiting = Vec::new();

    for mut reload in pending.pending.drain(..) {
        if reload.frames_remaining > 0 {
            reload.frames_remaining -= 1;
            still_waiting.push(reload);
        } else {
            // Reload has had time to execute — build response with error/escalation info
            let time = world.resource::<Time<Virtual>>();
            let mut response = serde_json::json!({
                "status": "reload_completed",
                "mode": reload.mode,
                "paused": time.is_paused(),
                "relative_speed": time.relative_speed(),
            });

            // Check for new errors since reload
            if let Some(last_error) = world.get_resource::<pybevy_core::LastSystemError>()
                && last_error.timestamp_secs > reload.error_timestamp_before
                && let Some(ref error_msg) = last_error.error
            {
                response["error"] = serde_json::json!(error_msg);
                response["traceback"] = serde_json::json!(last_error.traceback);
            }

            // Surface registration/Startup failures flagged on ReloadResult.
            if let Some(reload_result) = world.get_resource::<ReloadResult>() {
                if reload_result.failed {
                    response["status"] = serde_json::json!("reload_failed");
                    if response.get("error").map(|v| v.is_null()).unwrap_or(true) {
                        response["error"] = serde_json::json!(
                            reload_result
                                .failure_reason
                                .as_deref()
                                .unwrap_or("unknown registration failure")
                        );
                    }
                    if let Some(ref reason) = reload_result.failure_reason {
                        response["failure_reason"] = serde_json::json!(reason);
                    }
                }
                response["escalated"] = serde_json::json!(reload_result.escalated);
                if let Some(ref reason) = reload_result.escalation_reason {
                    response["escalation_reason"] = serde_json::json!(reason);
                }
            }

            if response.get("error").is_none() {
                response["error"] = serde_json::json!(null);
            }

            let _ = reload.response_tx.send(Ok(response));
        }
    }

    pending.pending = still_waiting;
    world.insert_resource(pending);
}

/// Process pending reload-and-capture requests.
/// State machine: WaitingForReload → check errors → queue screenshot → WaitingForScreenshot → send combined response.
pub fn process_pending_reload_and_capture(world: &mut World) {
    let Some(mut pending) = world.remove_resource::<PendingReloadAndCaptures>() else {
        return;
    };

    if pending.pending.is_empty() {
        world.insert_resource(pending);
        return;
    }

    let mut still_waiting = Vec::new();

    for mut rac in pending.pending.drain(..) {
        match rac.state {
            ReloadAndCaptureState::WaitingForReload => {
                if rac.reload_frames_remaining > 0 {
                    rac.reload_frames_remaining -= 1;
                    still_waiting.push(rac);
                } else {
                    // Reload has completed — check for errors
                    let time = world.resource::<Time<Virtual>>();
                    let mut reload_response = serde_json::json!({
                        "mode": rac.mode,
                        "status": "success",
                        "paused": time.is_paused(),
                        "relative_speed": time.relative_speed(),
                    });

                    let mut has_error = false;
                    if let Some(last_error) = world.get_resource::<pybevy_core::LastSystemError>()
                        && last_error.timestamp_secs > rac.error_timestamp_before
                        && let Some(ref error_msg) = last_error.error
                    {
                        reload_response["status"] = serde_json::json!("error");
                        reload_response["error"] = serde_json::json!(error_msg);
                        reload_response["traceback"] = serde_json::json!(last_error.traceback);

                        // Pattern-match error for hints
                        if let Some(h) = generate_error_hint(error_msg) {
                            reload_response["hint"] = serde_json::json!(h);
                        }
                        has_error = true;
                    }

                    if let Some(reload_result) = world.get_resource::<ReloadResult>() {
                        if reload_result.failed {
                            reload_response["status"] = serde_json::json!("reload_failed");
                            let reason_str = reload_result
                                .failure_reason
                                .as_deref()
                                .unwrap_or("unknown registration failure");
                            if reload_response
                                .get("error")
                                .map(|v| v.is_null())
                                .unwrap_or(true)
                            {
                                reload_response["error"] = serde_json::json!(reason_str);
                            }
                            if let Some(ref reason) = reload_result.failure_reason {
                                reload_response["failure_reason"] = serde_json::json!(reason);
                            }
                            has_error = true;
                        }
                        reload_response["escalated"] = serde_json::json!(reload_result.escalated);
                        if let Some(ref reason) = reload_result.escalation_reason {
                            reload_response["escalation_reason"] = serde_json::json!(reason);
                        }
                    }

                    if has_error {
                        // Error during reload — respond immediately without screenshot
                        let entity_count =
                            crate::handlers::entity_count::scene_entity_count(world) as usize;
                        let response = serde_json::json!({
                            "reload": reload_response,
                            "errors": reload_response.get("error"),
                            "screenshot": null,
                            "entity_count": entity_count,
                        });
                        let _ = rac.response_tx.send(Ok(response));
                    } else {
                        // Success — queue screenshot with extra reload data
                        let entity_count =
                            crate::handlers::entity_count::scene_entity_count(world) as usize;

                        let debug_camera = rac.position.map(|pos| DebugCameraRequest {
                            position: pos,
                            look_at: rac.look_at.unwrap_or([0.0, 0.0, 0.0]),
                        });

                        let screenshot = crate::bridge::PendingScreenshot {
                            response_tx: rac.response_tx,
                            frames_remaining: rac.screenshot_delay_frames,
                            with_gizmos: false,
                            max_width: rac.max_width.or(Some(768)),
                            debug_camera,
                            hide_ui: rac.hide_ui,
                            extra_response: Some(serde_json::json!({
                                "reload": reload_response,
                                "errors": null,
                                "entity_count": entity_count,
                            })),
                        };

                        let mut screenshots =
                            world.get_resource_or_insert_with(PendingScreenshots::default);
                        screenshots.pending.push(screenshot);
                    }
                }
            }
            ReloadAndCaptureState::WaitingForScreenshot => {
                // This state is no longer used since we use the forwarder pattern
                // but keep for safety
            }
        }
    }

    pending.pending = still_waiting;
    world.insert_resource(pending);
}

/// Generate error hints from common error patterns.
fn generate_error_hint(error: &str) -> Option<String> {
    if error.contains("NameError") {
        Some("NameError: A variable or import is missing. Check if you need to add an import (e.g., `from pybevy.prelude import *`) or if a variable name is misspelled.".to_string())
    } else if error.contains("AttributeError") {
        Some("AttributeError: A method or property doesn't exist on the object. Check the type definition with get_type_definition() or search_api().".to_string())
    } else if error.contains("TypeError") {
        Some("TypeError: Wrong argument types or count. Check the constructor signature with get_type_definition().".to_string())
    } else if error.contains("ImportError") || error.contains("ModuleNotFoundError") {
        Some("ImportError: A module or name couldn't be imported. Check if the import path is correct.".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use pybevy_core::{LastSystemError, PendingReloadRequest, ReloadRequestMode, ReloadResult};

    use super::*;
    use crate::bridge::PendingReloadResponse;

    #[test]
    fn generate_error_hint_name_error() {
        let hint = generate_error_hint("NameError: name 'foo' is not defined");
        assert!(hint.is_some());
        assert!(hint.unwrap().contains("NameError"));
    }

    #[test]
    fn generate_error_hint_attribute_error() {
        let hint = generate_error_hint("AttributeError: object has no attribute 'bar'");
        assert!(hint.is_some());
        assert!(hint.unwrap().contains("AttributeError"));
    }

    #[test]
    fn generate_error_hint_type_error() {
        let hint = generate_error_hint("TypeError: expected 2 arguments, got 3");
        assert!(hint.is_some());
        assert!(hint.unwrap().contains("TypeError"));
    }

    #[test]
    fn generate_error_hint_import_error() {
        let hint = generate_error_hint("ImportError: cannot import name 'Xyz'");
        assert!(hint.is_some());
        assert!(hint.unwrap().contains("ImportError"));
    }

    #[test]
    fn generate_error_hint_module_not_found() {
        let hint = generate_error_hint("ModuleNotFoundError: No module named 'nonexistent'");
        assert!(hint.is_some());
        assert!(hint.unwrap().contains("ImportError"));
    }

    #[test]
    fn generate_error_hint_no_match() {
        let hint = generate_error_hint("something random");
        assert!(hint.is_none());
    }

    #[test]
    fn generate_error_hint_empty_string() {
        let hint = generate_error_hint("");
        assert!(hint.is_none());
    }

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
    fn get_reload_status_with_escalation() {
        let mut world = World::new();
        world.insert_resource(ReloadResult {
            escalated: true,
            escalation_reason: Some("structural changes detected".to_string()),
            actual_mode: Some(ReloadRequestMode::Full),
            failed: false,
            failure_reason: None,
            running_previous_generation: false,
            plugins_added: None,
            plugins_removed: None,
            systems_removed: None,
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
            running_previous_generation: false,
            plugins_added: None,
            plugins_removed: None,
            systems_removed: Some(vec!["update_scoreboard".to_string()]),
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
        });
        let result = get_last_error(&mut world).unwrap();
        assert_eq!(result["error"], "boom");
        assert_eq!(result["traceback"], "at line 1");
        assert_eq!(result["timestamp_secs"], 42.0);
    }

    #[test]
    fn reload_and_capture_state_variants() {
        // Test Debug
        let state = ReloadAndCaptureState::WaitingForReload;
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("WaitingForReload"));

        // Test Clone and Copy
        let cloned = state.clone();
        let copied = state;
        assert_eq!(cloned, copied);

        // Test PartialEq: equal variants
        assert_eq!(
            ReloadAndCaptureState::WaitingForReload,
            ReloadAndCaptureState::WaitingForReload
        );

        // Test PartialEq: different variants
        assert_ne!(
            ReloadAndCaptureState::WaitingForReload,
            ReloadAndCaptureState::WaitingForScreenshot
        );
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
    fn trigger_reload_clears_stale_errors() {
        let mut world = world_with_reload_deps();
        world.insert_resource(pybevy_core::LastSystemError {
            error: Some("old error".into()),
            traceback: Some("old trace".into()),
            timestamp_secs: 1.0,
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
        world.insert_resource(PendingReloadResponses::default());
        process_pending_reloads(&mut world);
        // Should still have the resource
        assert!(world.get_resource::<PendingReloadResponses>().is_some());
    }

    #[test]
    fn process_pending_reloads_countdown() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloadResponses {
            pending: vec![PendingReloadResponse {
                response_tx: tx,
                frames_remaining: 3,
                mode: ReloadMode::Full,
                error_timestamp_before: 0.0,
            }],
        });
        process_pending_reloads(&mut world);
        let pending = world.get_resource::<PendingReloadResponses>().unwrap();
        assert_eq!(pending.pending.len(), 1);
        assert_eq!(pending.pending[0].frames_remaining, 2);
    }

    #[test]
    fn process_pending_reloads_sends_response_when_ready() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloadResponses {
            pending: vec![PendingReloadResponse {
                response_tx: tx,
                frames_remaining: 0,
                mode: ReloadMode::Full,
                error_timestamp_before: 0.0,
            }],
        });
        process_pending_reloads(&mut world);
        // Response should have been sent
        let result = rx.try_recv().unwrap().unwrap();
        assert_eq!(result["status"], "reload_completed");
        assert_eq!(result["mode"], "full");
        // Pending should be empty now
        let pending = world.get_resource::<PendingReloadResponses>().unwrap();
        assert!(pending.pending.is_empty());
    }

    #[test]
    fn process_pending_reloads_includes_error_if_new() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        // Insert an error that happened AFTER the reload was triggered
        world.insert_resource(pybevy_core::LastSystemError {
            error: Some("runtime crash".into()),
            traceback: Some("line 42".into()),
            timestamp_secs: 5.0,
        });
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloadResponses {
            pending: vec![PendingReloadResponse {
                response_tx: tx,
                frames_remaining: 0,
                mode: ReloadMode::Full,
                error_timestamp_before: 1.0, // error at 5.0 > 1.0
            }],
        });
        process_pending_reloads(&mut world);
        let result = rx.try_recv().unwrap().unwrap();
        assert_eq!(result["error"], "runtime crash");
        assert!(result.get("traceback").is_some());
    }

    #[test]
    fn process_pending_reloads_surfaces_reload_result_failed() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world.insert_resource(ReloadResult {
            failed: true,
            failure_reason: Some("AttributeError: bogus".into()),
            running_previous_generation: true,
            ..ReloadResult::default()
        });
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloadResponses {
            pending: vec![PendingReloadResponse {
                response_tx: tx,
                frames_remaining: 0,
                mode: ReloadMode::Full,
                error_timestamp_before: 0.0,
            }],
        });
        process_pending_reloads(&mut world);
        let result = rx.try_recv().unwrap().unwrap();
        assert_eq!(result["status"], "reload_failed");
        assert_eq!(result["error"], "AttributeError: bogus");
        assert_eq!(result["failure_reason"], "AttributeError: bogus");
    }

    #[test]
    fn process_pending_reloads_no_error_if_old() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        // Error happened BEFORE reload was triggered
        world.insert_resource(pybevy_core::LastSystemError {
            error: Some("old error".into()),
            traceback: None,
            timestamp_secs: 0.5,
        });
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        world.insert_resource(PendingReloadResponses {
            pending: vec![PendingReloadResponse {
                response_tx: tx,
                frames_remaining: 0,
                mode: ReloadMode::Full,
                error_timestamp_before: 1.0, // error at 0.5 < 1.0
            }],
        });
        process_pending_reloads(&mut world);
        let result = rx.try_recv().unwrap().unwrap();
        assert!(result["error"].is_null());
    }
}
