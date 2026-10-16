use std::collections::HashSet;

use pybevy_core::public_error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::protocol::{ControlOperation, OperationTag, json_object_schema};

/// sync (default): block until all actions complete. async: return schedule_id immediately.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleMode {
    /// Block until all actions complete
    #[default]
    Sync,
    /// Return schedule_id immediately
    Async,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScheduleRequest {
    /// Ordered list of tool calls to execute
    pub actions: Vec<ScheduleAction>,
    #[serde(default)]
    pub mode: ScheduleMode,
    /// Abort remaining actions on first error (default false)
    #[serde(default)]
    pub stop_on_error: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScheduleAction {
    /// Tool name to call
    pub tool: String,
    /// Tool arguments (default {})
    #[serde(default)]
    #[schemars(schema_with = "json_object_schema")]
    pub args: serde_json::Value,
    /// Time offset in **virtual** seconds from schedule start (default 0). Must be monotonically
    /// non-decreasing. Note: virtual seconds do not advance while time is paused, so a schedule
    /// that calls `pause_time` cannot use `at` for any later action - use `at_frame` instead, or
    /// call `resume_time` from outside the schedule. A `seek_time` action moves the schedule clock
    /// with virtual time, so later actions keep their `at` spacing from the seek.
    pub at: Option<f64>,
    /// Frame offset from schedule start (alternative to 'at'). Cannot mix with 'at'.
    pub at_frame: Option<i64>,
    /// Label for this action (used by skip_if_error)
    pub label: Option<String>,
    /// Skip this action if the action with the given label errored
    pub skip_if_error: Option<String>,
}

/// Bridge-local MCP tools that never reach the engine.
const BRIDGE_LOCAL_TOOLS: &[&str] = &[
    "get_schedule_result",
    "run_scene",
    "get_started",
    "get_logs",
    "search_api",
    "get_type_definition",
];

fn is_non_schedulable(tool: &str) -> bool {
    BRIDGE_LOCAL_TOOLS.contains(&tool)
        || OperationTag::from_name(tool).is_some_and(|tag| !tag.spec().schedulable)
}

pub fn validate_schedule(request: &ScheduleRequest) -> Result<(), String> {
    if request.actions.is_empty() {
        return Err("actions must not be empty".to_string());
    }
    if request.actions.len() > 256 {
        return Err("actions must have at most 256 entries".to_string());
    }
    let mut last_at: Option<f64> = None;
    let mut last_frame: Option<i64> = None;
    let mut uses_at = false;
    let mut uses_at_frame = false;
    let mut labels = HashSet::new();

    for (i, action) in request.actions.iter().enumerate() {
        if is_non_schedulable(&action.tool) {
            return Err(format!(
                "action[{}]: tool '{}' is not schedulable",
                i, action.tool
            ));
        }
        if !crate::catalog::tools::is_public_tool_name(&action.tool) {
            return Err(public_error::mcp_schedule_non_public_tool(i, &action.tool));
        }

        if let Some(skip_label) = action.skip_if_error.as_deref()
            && !labels.contains(skip_label)
        {
            return Err(format!(
                "action[{i}]: 'skip_if_error' must reference an earlier action label (got '{skip_label}')"
            ));
        }

        if let Some(label) = action.label.as_deref() {
            if label.is_empty() {
                return Err(format!("action[{i}]: 'label' must not be empty"));
            }
            if !labels.insert(label) {
                return Err(format!("action[{i}]: duplicate action label '{label}'"));
            }
        }

        match (action.at, action.at_frame) {
            (Some(_), Some(_)) => {
                return Err(format!(
                    "action[{}]: cannot specify both 'at' and 'at_frame'",
                    i
                ));
            }
            (Some(at), None) => {
                uses_at = true;
                if !at.is_finite() || at < 0.0 {
                    return Err(format!(
                        "action[{}]: 'at' must be a finite non-negative number",
                        i
                    ));
                }
                if let Some(prev) = last_at
                    && at < prev
                {
                    return Err(format!(
                        "action[{}]: 'at' values must be monotonically non-decreasing (got {} after {})",
                        i, at, prev
                    ));
                }
                last_at = Some(at);
            }
            (None, Some(frame)) => {
                uses_at_frame = true;
                if frame < 0 {
                    return Err(format!(
                        "action[{i}]: 'at_frame' must be a non-negative integer"
                    ));
                }
                if let Some(prev) = last_frame
                    && frame < prev
                {
                    return Err(format!(
                        "action[{}]: 'at_frame' values must be monotonically non-decreasing (got {} after {})",
                        i, frame, prev
                    ));
                }
                last_frame = Some(frame);
            }
            (None, None) => {
                // Default: at=0
            }
        }
    }

    if uses_at && uses_at_frame {
        return Err(
            "cannot mix 'at' (game-time) and 'at_frame' (frame offset) in the same schedule"
                .to_string(),
        );
    }

    Ok(())
}
pub fn tool_to_operation(tool: &str, args: &serde_json::Value) -> Result<ControlOperation, String> {
    let mut obj = if args.is_null() {
        serde_json::json!({})
    } else {
        args.clone()
    };
    let map = obj
        .as_object_mut()
        .ok_or_else(|| "args must be an object".to_string())?;

    // Route combined tools to their hidden sub-variants
    let effective_tool = match tool {
        "capture_screenshot" if map.get("gizmos").and_then(|v| v.as_bool()).unwrap_or(false) => {
            "capture_with_gizmos"
        }
        "query_spatial" if map.contains_key("radius") => "query_spatial_neighborhood",
        "check_overlaps" if map.get("entity").is_none_or(serde_json::Value::is_null) => {
            map.remove("entity");
            "check_all_overlaps"
        }
        other => other,
    };

    map.insert(
        "tool".to_string(),
        serde_json::Value::String(effective_tool.to_string()),
    );

    let call: ControlOperation =
        serde_json::from_value(obj).map_err(|e| format!("invalid tool call '{tool}': {e}"))?;
    Ok(call)
}
pub(super) fn resolve_at(action: &ScheduleAction) -> f64 {
    action.at.unwrap_or(0.0)
}

pub(super) fn resolve_at_frame(action: &ScheduleAction) -> Option<u64> {
    action.at_frame.and_then(|frame| u64::try_from(frame).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        CaptureScreenshotParams, CaptureStatsParams, CompareFramesParams, EntityRef,
        GetComponentParams, QueryEntitiesParams, ReloadMode, ReloadParams, SeekTimeParams,
        SetComponentParams,
    };

    #[test]
    fn test_validate_empty_actions() {
        let req = ScheduleRequest {
            actions: vec![],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        assert!(validate_schedule(&req).is_err());
    }

    #[test]
    fn test_validate_too_many_actions() {
        let actions: Vec<ScheduleAction> = (0..257)
            .map(|_| ScheduleAction {
                tool: "pause_time".to_string(),
                args: serde_json::Value::Null,
                at: Some(0.0),
                at_frame: None,
                label: None,
                skip_if_error: None,
            })
            .collect();
        let req = ScheduleRequest {
            actions,
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        assert!(validate_schedule(&req).is_err());
    }

    #[test]
    fn test_validate_non_schedulable_tool() {
        let req = ScheduleRequest {
            actions: vec![ScheduleAction {
                tool: "schedule_actions".to_string(),
                args: serde_json::Value::Null,
                at: Some(0.0),
                at_frame: None,
                label: None,
                skip_if_error: None,
            }],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        let err = validate_schedule(&req).unwrap_err();
        assert!(err.contains("not schedulable"));
    }

    #[test]
    fn test_validate_rejects_private_and_unknown_tools() {
        for tool in [
            "list_entities",
            "get_entity",
            "list_resources",
            "capture_with_gizmos",
            "get_config",
            "list_configs",
            "nonexistent_tool",
        ] {
            let req = ScheduleRequest {
                actions: vec![ScheduleAction {
                    tool: tool.to_string(),
                    args: serde_json::Value::Null,
                    at: Some(0.0),
                    at_frame: None,
                    label: None,
                    skip_if_error: None,
                }],
                mode: ScheduleMode::Sync,
                stop_on_error: false,
            };

            assert_eq!(
                validate_schedule(&req).unwrap_err(),
                format!("action[0]: tool '{tool}' is not a public schedulable tool")
            );
        }
    }

    #[test]
    fn test_public_tool_schema_matches_schedule_name_validation() {
        for tool in crate::catalog::tools::list_tools() {
            let name = tool["name"].as_str().unwrap();
            let req = ScheduleRequest {
                actions: vec![ScheduleAction {
                    tool: name.to_string(),
                    args: serde_json::Value::Null,
                    at: Some(0.0),
                    at_frame: None,
                    label: None,
                    skip_if_error: None,
                }],
                mode: ScheduleMode::Sync,
                stop_on_error: false,
            };
            let result = validate_schedule(&req);

            if is_non_schedulable(name) {
                assert_eq!(
                    result.unwrap_err(),
                    format!("action[0]: tool '{name}' is not schedulable")
                );
            } else {
                assert!(
                    result.is_ok(),
                    "public tool '{name}' was rejected: {result:?}"
                );
            }
        }
    }

    #[test]
    fn test_validate_non_monotonic_at() {
        let req = ScheduleRequest {
            actions: vec![
                ScheduleAction {
                    tool: "pause_time".to_string(),
                    args: serde_json::Value::Null,
                    at: Some(5.0),
                    at_frame: None,
                    label: None,
                    skip_if_error: None,
                },
                ScheduleAction {
                    tool: "resume_time".to_string(),
                    args: serde_json::Value::Null,
                    at: Some(3.0),
                    at_frame: None,
                    label: None,
                    skip_if_error: None,
                },
            ],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        let err = validate_schedule(&req).unwrap_err();
        assert!(err.contains("monotonically"));
    }

    #[test]
    fn test_validate_mixed_at_and_at_frame() {
        let req = ScheduleRequest {
            actions: vec![
                ScheduleAction {
                    tool: "pause_time".to_string(),
                    args: serde_json::Value::Null,
                    at: Some(0.0),
                    at_frame: None,
                    label: None,
                    skip_if_error: None,
                },
                ScheduleAction {
                    tool: "resume_time".to_string(),
                    args: serde_json::Value::Null,
                    at: None,
                    at_frame: Some(10),
                    label: None,
                    skip_if_error: None,
                },
            ],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        let err = validate_schedule(&req).unwrap_err();
        assert!(err.contains("cannot mix"));
    }

    #[test]
    fn test_validate_both_at_and_at_frame() {
        let req = ScheduleRequest {
            actions: vec![ScheduleAction {
                tool: "pause_time".to_string(),
                args: serde_json::Value::Null,
                at: Some(0.0),
                at_frame: Some(0),
                label: None,
                skip_if_error: None,
            }],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        let err = validate_schedule(&req).unwrap_err();
        assert!(err.contains("cannot specify both"));
    }

    #[test]
    fn test_validate_negative_at() {
        let req = ScheduleRequest {
            actions: vec![ScheduleAction {
                tool: "pause_time".to_string(),
                args: serde_json::Value::Null,
                at: Some(-1.0),
                at_frame: None,
                label: None,
                skip_if_error: None,
            }],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        let err = validate_schedule(&req).unwrap_err();
        assert!(err.contains("non-negative"));
    }

    #[test]
    fn test_validate_infinite_at() {
        let req = ScheduleRequest {
            actions: vec![ScheduleAction {
                tool: "pause_time".to_string(),
                args: serde_json::Value::Null,
                at: Some(f64::INFINITY),
                at_frame: None,
                label: None,
                skip_if_error: None,
            }],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        let err = validate_schedule(&req).unwrap_err();
        assert!(err.contains("finite"));
    }

    #[test]
    fn test_validate_valid_schedule() {
        let req = ScheduleRequest {
            actions: vec![
                ScheduleAction {
                    tool: "pause_time".to_string(),
                    args: serde_json::Value::Null,
                    at: Some(0.0),
                    at_frame: None,
                    label: Some("p".to_string()),
                    skip_if_error: None,
                },
                ScheduleAction {
                    tool: "resume_time".to_string(),
                    args: serde_json::Value::Null,
                    at: Some(5.0),
                    at_frame: None,
                    label: None,
                    skip_if_error: Some("p".to_string()),
                },
            ],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        assert!(validate_schedule(&req).is_ok());
    }

    #[test]
    fn test_validate_skip_if_error_rejects_unknown_or_forward_labels() {
        for json in [
            r#"{"actions":[{"tool":"resume_time","skip_if_error":"missing"}]}"#,
            r#"{"actions":[{"tool":"resume_time","skip_if_error":"later"},{"tool":"pause_time","label":"later"}]}"#,
        ] {
            let request: ScheduleRequest = serde_json::from_str(json).unwrap();
            assert!(
                validate_schedule(&request)
                    .unwrap_err()
                    .contains("must reference an earlier action label")
            );
        }
    }

    #[test]
    fn test_validate_rejects_duplicate_labels() {
        let request: ScheduleRequest = serde_json::from_str(
            r#"{"actions":[{"tool":"pause_time","label":"step"},{"tool":"resume_time","label":"step"}]}"#,
        )
        .unwrap();

        assert_eq!(
            validate_schedule(&request).unwrap_err(),
            "action[1]: duplicate action label 'step'"
        );
    }

    #[test]
    fn test_validate_default_at() {
        let req = ScheduleRequest {
            actions: vec![
                ScheduleAction {
                    tool: "pause_time".to_string(),
                    args: serde_json::Value::Null,
                    at: None,
                    at_frame: None,
                    label: None,
                    skip_if_error: None,
                },
                ScheduleAction {
                    tool: "resume_time".to_string(),
                    args: serde_json::Value::Null,
                    at: None,
                    at_frame: None,
                    label: None,
                    skip_if_error: None,
                },
            ],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        assert!(validate_schedule(&req).is_ok());
    }

    #[test]
    fn test_validate_at_frame_monotonic() {
        let req = ScheduleRequest {
            actions: vec![
                ScheduleAction {
                    tool: "pause_time".to_string(),
                    args: serde_json::Value::Null,
                    at: None,
                    at_frame: Some(0),
                    label: None,
                    skip_if_error: None,
                },
                ScheduleAction {
                    tool: "resume_time".to_string(),
                    args: serde_json::Value::Null,
                    at: None,
                    at_frame: Some(10),
                    label: None,
                    skip_if_error: None,
                },
            ],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        assert!(validate_schedule(&req).is_ok());
    }

    #[test]
    fn test_validate_negative_at_frame() {
        let req = ScheduleRequest {
            actions: vec![ScheduleAction {
                tool: "pause_time".to_string(),
                args: serde_json::Value::Null,
                at: None,
                at_frame: Some(-5),
                label: None,
                skip_if_error: None,
            }],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };

        assert_eq!(
            validate_schedule(&req).unwrap_err(),
            "action[0]: 'at_frame' must be a non-negative integer"
        );
    }

    #[test]
    fn test_tool_to_operation_pause() {
        let op = tool_to_operation("pause_time", &serde_json::Value::Null).unwrap();
        assert!(matches!(op, ControlOperation::PauseTime));
    }

    #[test]
    fn test_tool_to_operation_resume() {
        let op = tool_to_operation("resume_time", &serde_json::Value::Null).unwrap();
        assert!(matches!(op, ControlOperation::ResumeTime));
    }

    #[test]
    fn test_tool_to_operation_seek() {
        let op = tool_to_operation("seek_time", &serde_json::json!({"seconds": 5.0})).unwrap();
        assert!(
            matches!(op, ControlOperation::SeekTime(SeekTimeParams { seconds, pause }) if (seconds - 5.0).abs() < 0.001 && pause)
        );
    }

    #[test]
    fn test_tool_to_operation_set_time_scale() {
        let op = tool_to_operation("set_time_scale", &serde_json::json!({"scale": 2.0})).unwrap();
        assert!(
            matches!(op, ControlOperation::SetTimeScale { scale } if (scale - 2.0).abs() < 0.001)
        );
    }

    #[test]
    fn test_tool_to_operation_set_component() {
        let op = tool_to_operation(
            "set_component",
            &serde_json::json!({"entity": "MyEntity", "component": "Transform", "fields": {"translation": [0, 1, 0]}}),
        )
        .unwrap();
        assert!(
            matches!(op, ControlOperation::SetComponent(SetComponentParams { entity: EntityRef::Name(n), component, .. }) if n == "MyEntity" && component == "Transform")
        );
    }

    #[test]
    fn test_tool_to_operation_screenshot() {
        let op = tool_to_operation("capture_screenshot", &serde_json::json!({"max_width": 768}))
            .unwrap();
        assert!(matches!(
            op,
            ControlOperation::CaptureScreenshot(CaptureScreenshotParams {
                max_width: Some(768),
                ..
            })
        ));
    }

    #[test]
    fn test_tool_to_operation_stats_and_compare() {
        let stats = tool_to_operation("capture_stats", &serde_json::json!({"grid": 4})).unwrap();
        assert!(matches!(
            stats,
            ControlOperation::CaptureStats(CaptureStatsParams { grid: 4, .. })
        ));

        let compare = tool_to_operation(
            "compare_frames",
            &serde_json::json!({"a": "f_1", "b": "f_2"}),
        )
        .unwrap();
        assert!(matches!(
            compare,
            ControlOperation::CompareFrames(CompareFramesParams { a, b, .. })
                if a == "f_1" && b == "f_2"
        ));
    }

    #[test]
    fn test_tool_to_operation_unknown() {
        let result = tool_to_operation("nonexistent_tool", &serde_json::Value::Null);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_non_schedulable() {
        assert!(is_non_schedulable("schedule_actions"));
        assert!(is_non_schedulable("run_scene"));
        assert!(is_non_schedulable("get_started"));
        assert!(is_non_schedulable("get_logs"));
        assert!(is_non_schedulable("reload"));
        assert!(is_non_schedulable("reload_and_capture"));
        assert!(!is_non_schedulable("pause_time"));
        assert!(!is_non_schedulable("capture_screenshot"));
    }

    #[test]
    fn test_validate_rejects_reload() {
        let req = ScheduleRequest {
            actions: vec![ScheduleAction {
                tool: "reload".to_string(),
                args: serde_json::json!({"mode": "full"}),
                at: Some(0.0),
                at_frame: None,
                label: None,
                skip_if_error: None,
            }],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        let err = validate_schedule(&req).unwrap_err();
        assert!(err.contains("not schedulable"));
    }

    #[test]
    fn test_validate_rejects_reload_and_capture() {
        let req = ScheduleRequest {
            actions: vec![ScheduleAction {
                tool: "reload_and_capture".to_string(),
                args: serde_json::json!({"mode": "full"}),
                at: Some(0.0),
                at_frame: None,
                label: None,
                skip_if_error: None,
            }],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        let err = validate_schedule(&req).unwrap_err();
        assert!(err.contains("not schedulable"));
    }

    #[test]
    fn test_tool_to_operation_query_spatial_pairwise() {
        let op = tool_to_operation(
            "query_spatial",
            &serde_json::json!({"entity_a": "A", "entity_b": "B"}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::QuerySpatial(..)));
    }

    #[test]
    fn test_tool_to_operation_query_spatial_neighborhood() {
        let op = tool_to_operation(
            "query_spatial",
            &serde_json::json!({"entity": "A", "radius": 5.0}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::QuerySpatialNeighborhood(..)));
    }

    #[test]
    fn test_tool_to_operation_check_overlaps_single() {
        let op = tool_to_operation("check_overlaps", &serde_json::json!({"entity": "A"})).unwrap();
        assert!(matches!(op, ControlOperation::CheckOverlaps(..)));
    }

    #[test]
    fn test_tool_to_operation_check_overlaps_all() {
        let op = tool_to_operation("check_overlaps", &serde_json::json!({})).unwrap();
        assert!(matches!(op, ControlOperation::CheckAllOverlaps(..)));
    }

    #[test]
    fn test_tool_to_operation_get_time_status() {
        let op = tool_to_operation("get_time_status", &serde_json::Value::Null).unwrap();
        assert!(matches!(op, ControlOperation::GetTimeStatus));
    }

    #[test]
    fn test_tool_to_operation_get_performance() {
        let op = tool_to_operation("get_performance", &serde_json::Value::Null).unwrap();
        assert!(matches!(op, ControlOperation::GetPerformance));
    }

    #[test]
    fn test_tool_to_operation_get_scene_summary() {
        let op = tool_to_operation("get_scene_summary", &serde_json::Value::Null).unwrap();
        assert!(matches!(op, ControlOperation::GetSceneSummary));
    }

    #[test]
    fn test_tool_to_operation_get_system_list() {
        let op = tool_to_operation(
            "get_system_list",
            &serde_json::json!({"include_internal": true}),
        )
        .unwrap();
        assert!(matches!(
            op,
            ControlOperation::ListSystems {
                include_internal: true
            }
        ));
    }

    #[test]
    fn test_tool_to_operation_get_registry() {
        let op = tool_to_operation("get_registry", &serde_json::Value::Null).unwrap();
        assert!(matches!(op, ControlOperation::GetRegistry));
    }

    #[test]
    fn test_tool_to_operation_get_reload_status() {
        let op = tool_to_operation("get_reload_status", &serde_json::Value::Null).unwrap();
        assert!(matches!(op, ControlOperation::GetReloadStatus));
    }

    #[test]
    fn test_tool_to_operation_get_last_error() {
        let op = tool_to_operation("get_last_error", &serde_json::Value::Null).unwrap();
        assert!(matches!(op, ControlOperation::GetLastError));
    }

    #[test]
    fn test_tool_to_operation_get_bounding_box() {
        let op =
            tool_to_operation("get_bounding_box", &serde_json::json!({"entity": "Box"})).unwrap();
        assert!(matches!(op, ControlOperation::GetBoundingBox { .. }));
    }

    #[test]
    fn test_tool_to_operation_spawn_entity() {
        let op = tool_to_operation(
            "spawn_entity",
            &serde_json::json!({"components": {"Transform": {}}}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::SpawnEntity { .. }));
    }

    #[test]
    fn test_tool_to_operation_despawn_entity() {
        let op = tool_to_operation("despawn_entity", &serde_json::json!({"entity": "MyEntity"}))
            .unwrap();
        assert!(matches!(op, ControlOperation::DespawnEntity { .. }));
    }

    #[test]
    fn test_tool_to_operation_remove_component() {
        let op = tool_to_operation(
            "remove_component",
            &serde_json::json!({"entity": "E", "component": "Marker"}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::RemoveComponent(..)));
    }

    #[test]
    fn test_tool_to_operation_set_resource() {
        let op = tool_to_operation(
            "set_resource",
            &serde_json::json!({"resource_type": "GameSettings", "value": {}}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::SetResource(..)));
    }

    #[test]
    fn test_tool_to_operation_remove_resource() {
        let op = tool_to_operation(
            "remove_resource",
            &serde_json::json!({"resource_type": "GameSettings"}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::RemoveResource { .. }));
    }

    #[test]
    fn test_tool_to_operation_run_code() {
        let op = tool_to_operation("run_code", &serde_json::json!({"code": "print(1)"})).unwrap();
        assert!(matches!(op, ControlOperation::RunCode { .. }));
    }

    #[test]
    fn test_tool_to_operation_batch() {
        let op = tool_to_operation(
            "batch",
            &serde_json::json!({"operations": [{"type": "spawn"}]}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::Batch { .. }));
    }

    #[test]
    fn test_tool_to_operation_set_asset() {
        let op = tool_to_operation(
            "set_asset",
            &serde_json::json!({"entity": "E", "component": "MeshMaterial3d", "asset_type": "StandardMaterial", "fields": {}}),
        ).unwrap();
        assert!(matches!(op, ControlOperation::SetAsset(..)));
    }

    #[test]
    fn test_tool_to_operation_capture_timeline() {
        let op = tool_to_operation(
            "capture_timeline",
            &serde_json::json!({"total_frames": 120, "capture_count": 6}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::CaptureTimeline(..)));
    }

    #[test]
    fn test_tool_to_operation_capture_turnaround() {
        let op = tool_to_operation(
            "capture_turnaround",
            &serde_json::json!({"view_count": 8, "distance": 15.0}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::CaptureTurnaround(..)));
    }

    #[test]
    fn test_tool_to_operation_rejects_unknown_parameters() {
        let error = tool_to_operation(
            "capture_turnaround",
            &serde_json::json!({"views": 3, "view_count": 2}),
        )
        .unwrap_err();

        assert!(error.contains("unknown field `views`"), "{error}");
        assert!(error.contains("view_count"), "{error}");
    }

    #[test]
    fn test_tool_to_operation_capture_depth() {
        let op = tool_to_operation(
            "capture_depth",
            &serde_json::json!({"grid_density": 4, "include_rgb": false}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::CaptureDepth(..)));
    }

    #[test]
    fn test_tool_to_operation_reload() {
        let op = tool_to_operation("reload", &serde_json::json!({"mode": "partial"})).unwrap();
        assert!(matches!(
            op,
            ControlOperation::Reload(ReloadParams {
                mode: ReloadMode::Partial,
                ..
            })
        ));
    }

    #[test]
    fn test_tool_to_operation_reload_and_capture() {
        let op = tool_to_operation(
            "reload_and_capture",
            &serde_json::json!({"mode": "full", "delay_frames": 10}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::ReloadAndCapture(..)));
    }

    #[test]
    fn test_tool_to_operation_get_component_schema() {
        let op = tool_to_operation(
            "get_component_schema",
            &serde_json::json!({"name": "Transform"}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::GetComponentSchema { .. }));
    }

    #[test]
    fn test_tool_to_operation_get_component_missing_name() {
        let result = tool_to_operation("get_component_schema", &serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_to_operation_get_component_missing_component() {
        let result = tool_to_operation("get_component", &serde_json::json!({"entity": "E"}));
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_to_operation_seek_missing_seconds() {
        let result = tool_to_operation("seek_time", &serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_to_operation_screenshot_with_gizmos() {
        let op = tool_to_operation(
            "capture_screenshot",
            &serde_json::json!({"gizmos": true, "max_width": 1024}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::CaptureWithGizmos(..)));
    }

    #[test]
    fn test_tool_to_operation_entity_ref_by_id() {
        let op = tool_to_operation(
            "get_component",
            &serde_json::json!({"entity": 42, "component": "Transform"}),
        )
        .unwrap();
        assert!(matches!(
            op,
            ControlOperation::GetComponent(GetComponentParams {
                entity: EntityRef::Id(42),
                ..
            })
        ));
    }

    #[test]
    fn test_tool_to_operation_query_entities_with_filters() {
        let op = tool_to_operation(
            "query_entities",
            &serde_json::json!({
                "with": ["Transform", "Velocity"],
                "without": ["Marker"],
                "limit": 25,
            }),
        )
        .unwrap();
        assert!(matches!(
            op,
            ControlOperation::QueryEntities(QueryEntitiesParams { limit: 25, .. })
        ));
    }

    #[test]
    fn test_resolve_at_default() {
        let action = ScheduleAction {
            tool: "x".to_string(),
            args: serde_json::Value::Null,
            at: None,
            at_frame: None,
            label: None,
            skip_if_error: None,
        };
        assert_eq!(resolve_at(&action), 0.0);
    }

    #[test]
    fn test_resolve_at_with_value() {
        let action = ScheduleAction {
            tool: "x".to_string(),
            args: serde_json::Value::Null,
            at: Some(5.5),
            at_frame: None,
            label: None,
            skip_if_error: None,
        };
        assert_eq!(resolve_at(&action), 5.5);
    }

    #[test]
    fn test_resolve_at_frame() {
        let action = ScheduleAction {
            tool: "x".to_string(),
            args: serde_json::Value::Null,
            at: None,
            at_frame: Some(10),
            label: None,
            skip_if_error: None,
        };
        assert_eq!(resolve_at_frame(&action), Some(10));
    }

    #[test]
    fn test_resolve_at_frame_none() {
        let action = ScheduleAction {
            tool: "x".to_string(),
            args: serde_json::Value::Null,
            at: None,
            at_frame: None,
            label: None,
            skip_if_error: None,
        };
        assert_eq!(resolve_at_frame(&action), None);
    }

    #[test]
    fn test_validate_at_frame_non_monotonic() {
        let req = ScheduleRequest {
            actions: vec![
                ScheduleAction {
                    tool: "pause_time".to_string(),
                    args: serde_json::Value::Null,
                    at: None,
                    at_frame: Some(10),
                    label: None,
                    skip_if_error: None,
                },
                ScheduleAction {
                    tool: "resume_time".to_string(),
                    args: serde_json::Value::Null,
                    at: None,
                    at_frame: Some(5),
                    label: None,
                    skip_if_error: None,
                },
            ],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        let err = validate_schedule(&req).unwrap_err();
        assert!(err.contains("monotonically"));
    }

    #[test]
    fn test_validate_async_mode() {
        let req = ScheduleRequest {
            actions: vec![ScheduleAction {
                tool: "pause_time".to_string(),
                args: serde_json::Value::Null,
                at: Some(0.0),
                at_frame: None,
                label: None,
                skip_if_error: None,
            }],
            mode: ScheduleMode::Async,
            stop_on_error: false,
        };
        assert!(validate_schedule(&req).is_ok());
    }

    #[test]
    fn test_deserialize_invalid_mode() {
        let json = r#"{"actions": [{"tool": "pause_time"}], "mode": "invalid"}"#;
        let result: Result<ScheduleRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_schedule_request_deserialization_defaults() {
        let json = r#"{"actions": [{"tool": "pause_time"}]}"#;
        let req: ScheduleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mode, ScheduleMode::Sync);
        assert!(!req.stop_on_error);
        assert_eq!(req.actions.len(), 1);
    }

    #[test]
    fn test_schedule_action_deserialization_defaults() {
        let json = r#"{"tool": "pause_time"}"#;
        let action: ScheduleAction = serde_json::from_str(json).unwrap();
        assert_eq!(action.tool, "pause_time");
        assert!(action.at.is_none());
        assert!(action.at_frame.is_none());
        assert!(action.label.is_none());
        assert!(action.skip_if_error.is_none());
        assert!(action.args.is_null());
    }

    #[test]
    fn test_schedule_action_deserialization_full() {
        let json = r#"{"tool": "set_component", "args": {"x": 1}, "at": 2.5, "label": "step1", "skip_if_error": "step0"}"#;
        let action: ScheduleAction = serde_json::from_str(json).unwrap();
        assert_eq!(action.tool, "set_component");
        assert_eq!(action.at, Some(2.5));
        assert_eq!(action.label.as_deref(), Some("step1"));
        assert_eq!(action.skip_if_error.as_deref(), Some("step0"));
    }
}
