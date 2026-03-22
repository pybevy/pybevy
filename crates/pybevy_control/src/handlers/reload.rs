use bevy::{
    ecs::world::World,
    prelude::Resource,
    time::{Time, Virtual},
};
use pybevy_core::{PendingReloadRequest, ReloadRequestMode, ReloadResult};
use tokio::sync::oneshot;

use crate::bridge::{ControlError, DebugCameraRequest, PendingReloadResponses, PendingScreenshots};

/// State machine for reload-and-capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadAndCaptureState {
    WaitingForReload,
    WaitingForScreenshot,
}

/// A pending reload-and-capture request.
pub struct PendingReloadAndCapture {
    pub response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    pub mode: String,
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
    mode: String,
    pause: bool,
    time_scale: Option<f32>,
) -> Result<serde_json::Value, ControlError> {
    let reload_mode = match mode.as_str() {
        "partial" => ReloadRequestMode::Partial,
        _ => ReloadRequestMode::Full,
    };

    // Apply time control BEFORE queuing reload — no frames run at wrong speed
    if let Some(scale) = time_scale {
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
            if response.get("error").is_none() {
                response["error"] = serde_json::json!(null);
            }

            // Check for escalation
            if let Some(reload_result) = world.get_resource::<ReloadResult>() {
                response["escalated"] = serde_json::json!(reload_result.escalated);
                if let Some(ref reason) = reload_result.escalation_reason {
                    response["escalation_reason"] = serde_json::json!(reason);
                }
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
                        reload_response["escalated"] = serde_json::json!(reload_result.escalated);
                        if let Some(ref reason) = reload_result.escalation_reason {
                            reload_response["escalation_reason"] = serde_json::json!(reason);
                        }
                    }

                    if has_error {
                        // Error during reload — respond immediately without screenshot
                        let entity_count = count_entities(world);
                        let response = serde_json::json!({
                            "reload": reload_response,
                            "errors": reload_response.get("error"),
                            "screenshot": null,
                            "entity_count": entity_count,
                        });
                        let _ = rac.response_tx.send(Ok(response));
                    } else {
                        // Success — queue screenshot via forwarder
                        let entity_count = count_entities(world);

                        let debug_camera = rac.position.map(|pos| DebugCameraRequest {
                            position: pos,
                            look_at: rac.look_at.unwrap_or([0.0, 0.0, 0.0]),
                        });

                        // Create a forwarder channel that wraps the screenshot
                        // result into the combined response
                        let (forward_tx, forward_rx) =
                            oneshot::channel::<Result<serde_json::Value, ControlError>>();
                        let original_tx = rac.response_tx;
                        let reload_resp = reload_response.clone();

                        std::thread::spawn(move || {
                            let screenshot_result = forward_rx.blocking_recv();
                            let response = match screenshot_result {
                                Ok(Ok(screenshot_json)) => Ok(serde_json::json!({
                                    "reload": reload_resp,
                                    "errors": null,
                                    "screenshot": screenshot_json.get("image"),
                                    "screenshot_width": screenshot_json.get("width"),
                                    "screenshot_height": screenshot_json.get("height"),
                                    "entity_count": entity_count,
                                })),
                                Ok(Err(e)) => Ok(serde_json::json!({
                                    "reload": reload_resp,
                                    "errors": null,
                                    "screenshot_error": e.message,
                                    "entity_count": entity_count,
                                })),
                                Err(_) => Ok(serde_json::json!({
                                    "reload": reload_resp,
                                    "errors": null,
                                    "screenshot": null,
                                    "entity_count": entity_count,
                                })),
                            };
                            let _ = original_tx.send(response);
                        });

                        // Queue the screenshot with the forwarder's sender
                        let screenshot = crate::bridge::PendingScreenshot {
                            response_tx: forward_tx,
                            frames_remaining: rac.screenshot_delay_frames,
                            with_gizmos: false,
                            max_width: rac.max_width.or(Some(768)),
                            debug_camera,
                            hide_ui: rac.hide_ui,
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

/// Count total entities in the world.
fn count_entities(world: &World) -> usize {
    world.entities().len() as usize
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
