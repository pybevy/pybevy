use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{
        entity::Entity,
        hierarchy::{ChildOf, Children},
        world::World,
    },
    prelude::{GlobalTransform, Resource, Transform, Without},
    time::{Time, Virtual},
};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::{
    bridge::{
        ControlError, ControlOperation, DebugCameraRequest, EntityRef, PendingReloadResponses,
        PendingScreenshot, PendingScreenshots,
    },
    handlers::{
        self,
        reload::{PendingReloadAndCapture, PendingReloadAndCaptures, ReloadAndCaptureState},
        screenshot::{ActiveTimeline, PendingTimelines, setup_debug_camera},
        turnaround::{
            ActiveTurnaround, PendingTurnarounds, compute_scene_bounds, compute_viewpoints,
        },
    },
};

#[derive(Debug, Deserialize)]
pub struct ScheduleRequest {
    pub actions: Vec<ScheduleAction>,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub stop_on_error: bool,
}

fn default_mode() -> String {
    "sync".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScheduleAction {
    pub tool: String,
    #[serde(default)]
    pub args: serde_json::Value,
    pub at: Option<f64>,
    pub at_frame: Option<u64>,
    pub label: Option<String>,
    pub skip_if_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionResult {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub tool: String,
    pub at: f64,
    pub fired_at_game_time: f64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, PartialEq)]
enum ScheduleState {
    WaitingForTime,
    WaitingForDeferred,
    Done,
}

pub struct ActiveSchedule {
    pub schedule_id: String,
    actions: Vec<ScheduleAction>,
    results: Vec<ActionResult>,
    current_index: usize,
    state: ScheduleState,
    t0_game_time: f64,
    frame_counter: u64,
    stop_on_error: bool,
    errored_labels: HashSet<String>,
    sync_response_tx: Option<oneshot::Sender<Result<serde_json::Value, ControlError>>>,
    async_shared: Option<Arc<Mutex<SharedScheduleState>>>,
    deferred_rx: Option<oneshot::Receiver<Result<serde_json::Value, ControlError>>>,
}

#[derive(Resource, Default)]
pub struct ActiveSchedules {
    pub schedules: Vec<ActiveSchedule>,
    pub next_id: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SharedScheduleState {
    pub schedule_id: String,
    pub status: String,
    pub total_actions: usize,
    pub completed_actions: usize,
    pub results: Vec<ActionResult>,
    pub cancelled: bool,
}

impl SharedScheduleState {
    pub fn new(schedule_id: &str, total: usize) -> Self {
        Self {
            schedule_id: schedule_id.to_string(),
            status: "running".to_string(),
            total_actions: total,
            completed_actions: 0,
            results: Vec::new(),
            cancelled: false,
        }
    }
}

#[derive(Clone, Default)]
pub struct SharedScheduleRegistry {
    inner: Arc<Mutex<std::collections::HashMap<String, Arc<Mutex<SharedScheduleState>>>>>,
}

impl SharedScheduleRegistry {
    pub fn get(&self, id: &str) -> Option<SharedScheduleState> {
        let guard = self.inner.lock().ok()?;
        let arc = guard.get(id)?;
        arc.lock().ok().map(|g| g.clone())
    }

    pub fn insert(&self, id: String, state: Arc<Mutex<SharedScheduleState>>) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.insert(id, state);
        }
    }

    pub fn cancel(&self, id: &str) -> bool {
        if let Ok(guard) = self.inner.lock() {
            if let Some(arc) = guard.get(id) {
                if let Ok(mut state) = arc.lock() {
                    state.cancelled = true;
                    return true;
                }
            }
        }
        false
    }
}

#[derive(Resource, Clone)]
pub struct SharedScheduleRegistryResource(pub SharedScheduleRegistry);
const NON_SCHEDULABLE_TOOLS: &[&str] = &[
    "schedule_actions",
    "get_schedule_result",
    "run_scene",
    "get_started",
    "get_logs",
    "search_api",
    "get_type_definition",
];

fn is_non_schedulable(tool: &str) -> bool {
    NON_SCHEDULABLE_TOOLS.contains(&tool)
}

fn is_deferred_tool(name: &str) -> bool {
    matches!(
        name,
        "capture_screenshot"
            | "capture_timeline"
            | "capture_turnaround"
            | "capture_depth"
            | "reload"
            | "reload_and_capture"
    )
}

fn is_time_control_tool(name: &str) -> bool {
    matches!(
        name,
        "seek_time" | "pause_time" | "resume_time" | "set_time_scale"
    )
}

/// Returns true if the tool could modify transforms (and thus require
/// GlobalTransform propagation within the same frame).
fn is_transform_mutation_tool(name: &str) -> bool {
    matches!(name, "set_component" | "spawn_entity" | "batch")
}

/// Propagate transforms through the full hierarchy after mutations.
/// Updates GlobalTransform for root entities first, then recursively for children.
/// This ensures spatial queries see up-to-date world positions within the same frame.
fn propagate_transforms(world: &mut World) {
    // Update root entities (no ChildOf)
    let mut root_query = world.query_filtered::<(Entity, &Transform), Without<ChildOf>>();
    let roots: Vec<(Entity, GlobalTransform)> = root_query
        .iter(world)
        .map(|(e, t)| (e, GlobalTransform::from(*t)))
        .collect();
    for (entity, gt) in &roots {
        if let Some(mut gt_mut) = world.get_mut::<GlobalTransform>(*entity) {
            *gt_mut = *gt;
        }
    }

    // Propagate to children (breadth-first)
    let mut queue: Vec<(Entity, GlobalTransform)> = roots;
    while !queue.is_empty() {
        let mut next_queue = Vec::new();
        for (parent_entity, parent_gt) in &queue {
            // Get children of this entity
            let child_entities: Vec<Entity> =
                if let Some(children) = world.get::<Children>(*parent_entity) {
                    children.iter().copied().collect()
                } else {
                    continue;
                };
            for child in child_entities {
                if let Some(child_transform) = world.get::<Transform>(child) {
                    let child_gt = parent_gt.mul_transform(*child_transform);
                    if let Some(mut gt_mut) = world.get_mut::<GlobalTransform>(child) {
                        *gt_mut = child_gt;
                    }
                    next_queue.push((child, child_gt));
                }
            }
        }
        queue = next_queue;
    }
}
pub fn validate_schedule(request: &ScheduleRequest) -> Result<(), String> {
    if request.actions.is_empty() {
        return Err("actions must not be empty".to_string());
    }
    if request.actions.len() > 256 {
        return Err("actions must have at most 256 entries".to_string());
    }
    if request.mode != "sync" && request.mode != "async" {
        return Err(format!(
            "mode must be 'sync' or 'async', got '{}'",
            request.mode
        ));
    }

    let mut last_at: Option<f64> = None;
    let mut last_frame: Option<u64> = None;
    let mut uses_at = false;
    let mut uses_at_frame = false;

    for (i, action) in request.actions.iter().enumerate() {
        if is_non_schedulable(&action.tool) {
            return Err(format!(
                "action[{}]: tool '{}' is not schedulable",
                i, action.tool
            ));
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
                if let Some(prev) = last_at {
                    if at < prev {
                        return Err(format!(
                            "action[{}]: 'at' values must be monotonically non-decreasing (got {} after {})",
                            i, at, prev
                        ));
                    }
                }
                last_at = Some(at);
            }
            (None, Some(frame)) => {
                uses_at_frame = true;
                if let Some(prev) = last_frame {
                    if frame < prev {
                        return Err(format!(
                            "action[{}]: 'at_frame' values must be monotonically non-decreasing (got {} after {})",
                            i, frame, prev
                        ));
                    }
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
    let obj = args.as_object();
    let get_str = |key: &str| -> String {
        obj.and_then(|o| o.get(key))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let get_f64 =
        |key: &str| -> Option<f64> { obj.and_then(|o| o.get(key)).and_then(|v| v.as_f64()) };
    let get_f32 = |key: &str| -> Option<f32> { get_f64(key).map(|v| v as f32) };
    let get_u32 = |key: &str| -> Option<u32> {
        obj.and_then(|o| o.get(key))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
    };
    let get_bool = |key: &str, default: bool| -> bool {
        obj.and_then(|o| o.get(key))
            .and_then(|v| v.as_bool())
            .unwrap_or(default)
    };
    let get_entity_ref = || -> EntityRef {
        if let Some(id) = obj
            .and_then(|o| o.get("entity_id"))
            .and_then(|v| v.as_u64())
        {
            EntityRef::Id(id)
        } else {
            EntityRef::Name(get_str("name"))
        }
    };
    let get_vec3 = |key: &str| -> Option<[f32; 3]> {
        obj.and_then(|o| o.get(key)).and_then(|v| {
            let arr = v.as_array()?;
            if arr.len() != 3 {
                return None;
            }
            Some([
                arr[0].as_f64()? as f32,
                arr[1].as_f64()? as f32,
                arr[2].as_f64()? as f32,
            ])
        })
    };

    match tool {
        // Time control
        "pause_time" => Ok(ControlOperation::PauseTime),
        "resume_time" => Ok(ControlOperation::ResumeTime),
        "set_time_scale" => {
            let scale = get_f32("scale").unwrap_or(1.0);
            Ok(ControlOperation::SetTimeScale { scale })
        }
        "get_time_status" => Ok(ControlOperation::GetTimeStatus),
        "seek_time" => {
            let seconds =
                get_f64("seconds").ok_or_else(|| "seek_time requires 'seconds'".to_string())?;
            let pause = get_bool("pause", true);
            Ok(ControlOperation::SeekTime { seconds, pause })
        }

        // Scene queries
        "query_entities" => {
            let with = obj
                .and_then(|o| o.get("with"))
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let without = obj
                .and_then(|o| o.get("without"))
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            Ok(ControlOperation::QueryEntities { with, without })
        }
        "get_component" => {
            let component = get_str("component");
            if component.is_empty() {
                return Err("get_component requires 'component'".to_string());
            }
            Ok(ControlOperation::GetComponent {
                entity: get_entity_ref(),
                component,
            })
        }
        "get_component_schema" => {
            let name = get_str("name");
            if name.is_empty() {
                return Err("get_component_schema requires 'name'".to_string());
            }
            Ok(ControlOperation::GetComponentSchema { name })
        }
        "get_scene_summary" => Ok(ControlOperation::SceneSummary),
        "get_performance" => Ok(ControlOperation::GetPerformance),
        "get_registry" => Ok(ControlOperation::DebugRegistry),
        "get_reload_status" => Ok(ControlOperation::GetReloadStatus),
        "get_last_error" => Ok(ControlOperation::GetLastError),
        "get_bounding_box" => Ok(ControlOperation::GetBoundingBox {
            entity: get_entity_ref(),
        }),

        // Mutations
        "spawn_entity" => {
            let components = obj
                .and_then(|o| o.get("components"))
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            Ok(ControlOperation::SpawnEntity { components })
        }
        "despawn_entity" => Ok(ControlOperation::DespawnEntity {
            entity: get_entity_ref(),
        }),
        "set_component" => {
            let component = get_str("component");
            let fields = obj
                .and_then(|o| o.get("fields"))
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            Ok(ControlOperation::SetComponent {
                entity: get_entity_ref(),
                component,
                fields,
            })
        }
        "remove_component" => {
            let component = get_str("component");
            Ok(ControlOperation::RemoveComponent {
                entity: get_entity_ref(),
                component,
            })
        }
        "set_resource" => {
            let resource_type = get_str("resource_type");
            let value = obj
                .and_then(|o| o.get("value"))
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            Ok(ControlOperation::InsertResource {
                resource_type,
                value,
            })
        }
        "remove_resource" => {
            let resource_type = get_str("resource_type");
            Ok(ControlOperation::RemoveResource { resource_type })
        }
        "run_code" => {
            let code = get_str("code");
            Ok(ControlOperation::ExecutePython { code })
        }
        "batch" => {
            let operations = obj
                .and_then(|o| o.get("operations"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(ControlOperation::BatchMutate { operations })
        }

        // Asset mutation
        "set_asset" => {
            let component = get_str("component");
            let asset_type = get_str("asset_type");
            let fields = obj
                .and_then(|o| o.get("fields"))
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            Ok(ControlOperation::MutateAsset {
                entity: get_entity_ref(),
                component,
                asset_type,
                fields,
            })
        }

        // Spatial queries
        "query_spatial" => {
            if obj.map_or(false, |o| o.contains_key("radius")) {
                let radius = get_f32("radius").unwrap_or(10.0);
                let max_results = obj
                    .and_then(|o| o.get("max_results"))
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                Ok(ControlOperation::QuerySpatialNeighborhood {
                    entity: get_entity_ref(),
                    radius,
                    max_results,
                })
            } else {
                let entity_a = if let Some(id) = obj
                    .and_then(|o| o.get("entity_id_a"))
                    .and_then(|v| v.as_u64())
                {
                    EntityRef::Id(id)
                } else {
                    EntityRef::Name(get_str("name_a"))
                };
                let entity_b = if let Some(id) = obj
                    .and_then(|o| o.get("entity_id_b"))
                    .and_then(|v| v.as_u64())
                {
                    EntityRef::Id(id)
                } else {
                    EntityRef::Name(get_str("name_b"))
                };
                Ok(ControlOperation::QuerySpatial { entity_a, entity_b })
            }
        }
        "check_overlaps" => {
            let has_entity = obj.map_or(false, |o| {
                o.contains_key("entity_id") || o.contains_key("name")
            });
            let max_float_gap = get_f32("max_float_gap").unwrap_or(0.1);
            let ground_y = get_f32("ground_y");
            if has_entity {
                let include_siblings = get_bool("include_siblings", false);
                Ok(ControlOperation::CheckOverlaps {
                    entity: get_entity_ref(),
                    include_siblings,
                    max_float_gap,
                    ground_y,
                })
            } else {
                let min_penetration = get_f32("min_penetration");
                let max_results = obj
                    .and_then(|o| o.get("max_results"))
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                let include_siblings = get_bool("include_siblings", false);
                Ok(ControlOperation::CheckAllOverlaps {
                    min_penetration,
                    max_results,
                    max_float_gap,
                    ground_y,
                    include_siblings,
                })
            }
        }

        // Deferred tools (screenshots, reload, etc.)
        "capture_screenshot" => {
            let delay_frames = get_u32("delay_frames").unwrap_or(2);
            let max_width = get_u32("max_width");
            let position = get_vec3("position");
            let look_at = get_vec3("look_at");
            let hide_ui = get_bool("hide_ui", true);
            if get_bool("gizmos", false) {
                Ok(ControlOperation::CaptureWithGizmos {
                    delay_frames,
                    max_width,
                    position,
                    look_at,
                    hide_ui,
                })
            } else {
                Ok(ControlOperation::CaptureScreenshot {
                    delay_frames,
                    max_width,
                    position,
                    look_at,
                    hide_ui,
                })
            }
        }
        "capture_timeline" => Ok(ControlOperation::CaptureTimeline {
            total_frames: get_u32("total_frames").unwrap_or(60),
            capture_count: get_u32("capture_count").unwrap_or(6),
            max_width: get_u32("max_width"),
            columns: get_u32("columns").unwrap_or(3),
            position: get_vec3("position"),
            look_at: get_vec3("look_at"),
        }),
        "reload" => Ok(ControlOperation::TriggerReload {
            mode: get_str("mode"),
            pause: get_bool("pause", false),
            time_scale: get_f32("time_scale"),
        }),
        "reload_and_capture" => Ok(ControlOperation::ReloadAndCapture {
            mode: {
                let m = get_str("mode");
                if m.is_empty() { "full".to_string() } else { m }
            },
            pause: get_bool("pause", false),
            time_scale: get_f32("time_scale"),
            delay_frames: get_u32("delay_frames"),
            max_width: get_u32("max_width"),
            position: get_vec3("position"),
            look_at: get_vec3("look_at"),
            hide_ui: Some(get_bool("hide_ui", true)),
        }),
        "capture_turnaround" => Ok(ControlOperation::CaptureTurnaround {
            look_at: get_vec3("look_at"),
            distance: get_f32("distance"),
            elevation: get_f32("elevation"),
            view_count: get_u32("view_count"),
            include_top: obj
                .and_then(|o| o.get("include_top"))
                .and_then(|v| v.as_bool()),
            columns: get_u32("columns"),
            max_width: get_u32("max_width"),
            hide_ui: obj.and_then(|o| o.get("hide_ui")).and_then(|v| v.as_bool()),
        }),
        "capture_depth" => Ok(ControlOperation::CaptureDepth {
            position: get_vec3("position"),
            look_at: get_vec3("look_at"),
            sample_points: obj.and_then(|o| o.get("sample_points")).and_then(|v| {
                v.as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|pt| {
                            let a = pt.as_array()?;
                            if a.len() != 2 {
                                return None;
                            }
                            Some([a[0].as_u64()? as u32, a[1].as_u64()? as u32])
                        })
                        .collect()
                })
            }),
            grid_density: get_u32("grid_density"),
            include_rgb: obj
                .and_then(|o| o.get("include_rgb"))
                .and_then(|v| v.as_bool()),
            delay_frames: get_u32("delay_frames"),
            hide_ui: obj.and_then(|o| o.get("hide_ui")).and_then(|v| v.as_bool()),
            max_width: get_u32("max_width"),
        }),

        // Custom tools
        _ if tool.starts_with("custom.") => {
            let name = tool.to_string();
            let arguments = args.clone();
            Ok(ControlOperation::CallCustomTool { name, arguments })
        }

        _ => Err(format!("unknown tool: '{}'", tool)),
    }
}
fn resolve_at(action: &ScheduleAction) -> f64 {
    action.at.unwrap_or(0.0)
}

fn resolve_at_frame(action: &ScheduleAction) -> Option<u64> {
    action.at_frame
}
impl ActiveSchedule {
    pub fn new_sync(
        schedule_id: String,
        request: ScheduleRequest,
        t0_game_time: f64,
        response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    ) -> Self {
        Self {
            schedule_id,
            stop_on_error: request.stop_on_error,
            actions: request.actions,
            results: Vec::new(),
            current_index: 0,
            state: ScheduleState::WaitingForTime,
            t0_game_time,
            frame_counter: 0,
            errored_labels: HashSet::new(),
            sync_response_tx: Some(response_tx),
            async_shared: None,
            deferred_rx: None,
        }
    }

    pub fn new_async(
        schedule_id: String,
        request: ScheduleRequest,
        t0_game_time: f64,
        shared: Arc<Mutex<SharedScheduleState>>,
    ) -> Self {
        Self {
            schedule_id,
            stop_on_error: request.stop_on_error,
            actions: request.actions,
            results: Vec::new(),
            current_index: 0,
            state: ScheduleState::WaitingForTime,
            t0_game_time,
            frame_counter: 0,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: Some(shared),
            deferred_rx: None,
        }
    }
}
pub fn process_active_schedules(world: &mut World) {
    let mut schedules = world
        .remove_resource::<ActiveSchedules>()
        .unwrap_or_default();

    if schedules.schedules.is_empty() {
        world.insert_resource(schedules);
        return;
    }

    // Hold GIL for Python-touching tools (same pattern as control_poll_system)
    pyo3::Python::attach(|_py| {
        let mut i = 0;
        while i < schedules.schedules.len() {
            // Check for async cancellation
            let cancelled = schedules.schedules[i]
                .async_shared
                .as_ref()
                .and_then(|shared| shared.lock().ok().map(|g| g.cancelled))
                .unwrap_or(false);

            if cancelled {
                let schedule = &mut schedules.schedules[i];
                while schedule.current_index < schedule.actions.len() {
                    let action = &schedule.actions[schedule.current_index];
                    schedule.results.push(ActionResult {
                        index: schedule.current_index,
                        label: action.label.clone(),
                        tool: action.tool.clone(),
                        at: resolve_at(action),
                        fired_at_game_time: 0.0,
                        status: "cancelled".to_string(),
                        result: None,
                        error: Some("Schedule cancelled".to_string()),
                    });
                    schedule.current_index += 1;
                }
                schedule.state = ScheduleState::Done;
            }

            process_single_schedule(world, &mut schedules.schedules[i]);
            schedules.schedules[i].frame_counter += 1;

            if schedules.schedules[i].state == ScheduleState::Done {
                let schedule = schedules.schedules.remove(i);
                finalize_schedule(schedule);
            } else {
                i += 1;
            }
        }
    });

    world.insert_resource(schedules);
}

fn process_single_schedule(world: &mut World, schedule: &mut ActiveSchedule) {
    loop {
        match schedule.state {
            ScheduleState::Done => return,

            ScheduleState::WaitingForDeferred => {
                let rx = schedule.deferred_rx.as_mut().unwrap();
                match rx.try_recv() {
                    Ok(result) => {
                        let action = &schedule.actions[schedule.current_index];
                        let game_time = world
                            .get_resource::<Time<Virtual>>()
                            .map(|t| t.elapsed_secs_f64())
                            .unwrap_or(0.0);

                        match result {
                            Ok(value) => {
                                let has_errors =
                                    crate::handlers::mutate::has_embedded_errors(&value);
                                let has_run_code_failure = value
                                    .get("success")
                                    .and_then(|v| v.as_bool())
                                    .is_some_and(|s| !s);
                                let status = if has_errors {
                                    "partial"
                                } else if has_run_code_failure {
                                    "error"
                                } else {
                                    "ok"
                                };

                                if has_errors || has_run_code_failure {
                                    if let Some(ref label) = action.label {
                                        schedule.errored_labels.insert(label.clone());
                                    }
                                    if schedule.stop_on_error {
                                        let error_msg = if has_run_code_failure {
                                            value
                                                .get("error")
                                                .and_then(|v| v.as_str())
                                                .map(|s| s.to_string())
                                        } else {
                                            None
                                        };
                                        schedule.results.push(ActionResult {
                                            index: schedule.current_index,
                                            label: action.label.clone(),
                                            tool: action.tool.clone(),
                                            at: resolve_at(action),
                                            fired_at_game_time: game_time,
                                            status: status.to_string(),
                                            result: Some(value),
                                            error: error_msg,
                                        });
                                        schedule.current_index += 1;
                                        abort_remaining(schedule, schedule.current_index);
                                        schedule.state = ScheduleState::Done;
                                        return;
                                    }
                                }

                                let error_msg = if has_run_code_failure {
                                    value
                                        .get("error")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string())
                                } else {
                                    None
                                };
                                schedule.results.push(ActionResult {
                                    index: schedule.current_index,
                                    label: action.label.clone(),
                                    tool: action.tool.clone(),
                                    at: resolve_at(action),
                                    fired_at_game_time: game_time,
                                    status: status.to_string(),
                                    result: Some(value),
                                    error: error_msg,
                                });
                            }
                            Err(e) => {
                                if let Some(ref label) = action.label {
                                    schedule.errored_labels.insert(label.clone());
                                }
                                schedule.results.push(ActionResult {
                                    index: schedule.current_index,
                                    label: action.label.clone(),
                                    tool: action.tool.clone(),
                                    at: resolve_at(action),
                                    fired_at_game_time: game_time,
                                    status: "error".to_string(),
                                    result: None,
                                    error: Some(e.message),
                                });
                                if schedule.stop_on_error {
                                    abort_remaining(schedule, schedule.current_index + 1);
                                    schedule.state = ScheduleState::Done;
                                    return;
                                }
                            }
                        }
                        schedule.deferred_rx = None;
                        schedule.current_index += 1;
                        schedule.state = ScheduleState::WaitingForTime;
                        // Update async shared state
                        update_async_progress(schedule);
                        continue;
                    }
                    Err(oneshot::error::TryRecvError::Empty) => return,
                    Err(oneshot::error::TryRecvError::Closed) => {
                        let action = &schedule.actions[schedule.current_index];
                        if let Some(ref label) = action.label {
                            schedule.errored_labels.insert(label.clone());
                        }
                        schedule.results.push(ActionResult {
                            index: schedule.current_index,
                            label: action.label.clone(),
                            tool: action.tool.clone(),
                            at: resolve_at(action),
                            fired_at_game_time: 0.0,
                            status: "error".to_string(),
                            result: None,
                            error: Some("Deferred channel closed unexpectedly".to_string()),
                        });
                        schedule.deferred_rx = None;
                        schedule.current_index += 1;
                        if schedule.stop_on_error {
                            abort_remaining(schedule, schedule.current_index);
                            schedule.state = ScheduleState::Done;
                        } else {
                            schedule.state = ScheduleState::WaitingForTime;
                        }
                        update_async_progress(schedule);
                        return;
                    }
                }
            }

            ScheduleState::WaitingForTime => {
                if schedule.current_index >= schedule.actions.len() {
                    schedule.state = ScheduleState::Done;
                    return;
                }

                let action = &schedule.actions[schedule.current_index];
                let game_time = world
                    .get_resource::<Time<Virtual>>()
                    .map(|t| t.elapsed_secs_f64())
                    .unwrap_or(0.0);

                // Check timing
                let ready = if let Some(frame_offset) = resolve_at_frame(action) {
                    schedule.frame_counter >= frame_offset
                } else {
                    let target_time = schedule.t0_game_time + resolve_at(action);
                    game_time >= target_time
                };

                if !ready {
                    return; // Try next frame
                }

                // Check skip_if_error
                if let Some(ref skip_label) = action.skip_if_error {
                    if schedule.errored_labels.contains(skip_label) {
                        schedule.results.push(ActionResult {
                            index: schedule.current_index,
                            label: action.label.clone(),
                            tool: action.tool.clone(),
                            at: resolve_at(action),
                            fired_at_game_time: game_time,
                            status: "skipped".to_string(),
                            result: None,
                            error: Some(format!("Skipped due to error in '{}'", skip_label)),
                        });
                        schedule.current_index += 1;
                        update_async_progress(schedule);
                        continue;
                    }
                }

                // Execute the action
                let tool_name = action.tool.clone();
                let tool_args = action.args.clone();
                let action_label = action.label.clone();
                let action_at = resolve_at(action);
                let is_time_tool = is_time_control_tool(&tool_name);
                let is_mutation_tool = is_transform_mutation_tool(&tool_name);

                if is_deferred_tool(&tool_name) {
                    // Set up deferred execution via forwarder pattern
                    match setup_deferred_action(world, &tool_name, &tool_args) {
                        Ok(rx) => {
                            schedule.deferred_rx = Some(rx);
                            schedule.state = ScheduleState::WaitingForDeferred;
                            return; // Wait for result
                        }
                        Err(e) => {
                            if let Some(ref label) = action_label {
                                schedule.errored_labels.insert(label.clone());
                            }
                            schedule.results.push(ActionResult {
                                index: schedule.current_index,
                                label: action_label,
                                tool: tool_name,
                                at: action_at,
                                fired_at_game_time: game_time,
                                status: "error".to_string(),
                                result: None,
                                error: Some(e),
                            });
                            schedule.current_index += 1;
                            if schedule.stop_on_error {
                                abort_remaining(schedule, schedule.current_index);
                                schedule.state = ScheduleState::Done;
                                return;
                            }
                            update_async_progress(schedule);
                            continue;
                        }
                    }
                } else {
                    // Sync tool — execute immediately via dispatch
                    match tool_to_operation(&tool_name, &tool_args) {
                        Ok(op) => {
                            let result = handlers::dispatch(world, op);
                            match result {
                                Ok(value) => {
                                    let has_errors =
                                        crate::handlers::mutate::has_embedded_errors(&value);
                                    let has_run_code_failure = value
                                        .get("success")
                                        .and_then(|v| v.as_bool())
                                        .is_some_and(|s| !s);
                                    let status = if has_errors {
                                        "partial"
                                    } else if has_run_code_failure {
                                        "error"
                                    } else {
                                        "ok"
                                    };

                                    if has_errors || has_run_code_failure {
                                        if let Some(ref label) = action_label {
                                            schedule.errored_labels.insert(label.clone());
                                        }
                                        if schedule.stop_on_error {
                                            let error_msg = if has_run_code_failure {
                                                value
                                                    .get("error")
                                                    .and_then(|v| v.as_str())
                                                    .map(|s| s.to_string())
                                            } else {
                                                None
                                            };
                                            schedule.results.push(ActionResult {
                                                index: schedule.current_index,
                                                label: action_label,
                                                tool: tool_name,
                                                at: action_at,
                                                fired_at_game_time: game_time,
                                                status: status.to_string(),
                                                result: Some(value),
                                                error: error_msg,
                                            });
                                            schedule.current_index += 1;
                                            abort_remaining(schedule, schedule.current_index);
                                            schedule.state = ScheduleState::Done;
                                            return;
                                        }
                                    }

                                    let error_msg = if has_run_code_failure {
                                        value
                                            .get("error")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string())
                                    } else {
                                        None
                                    };
                                    schedule.results.push(ActionResult {
                                        index: schedule.current_index,
                                        label: action_label,
                                        tool: tool_name,
                                        at: action_at,
                                        fired_at_game_time: game_time,
                                        status: status.to_string(),
                                        result: Some(value),
                                        error: error_msg,
                                    });

                                    // After successful dispatch, sync transforms if
                                    // the tool could have modified them so that
                                    // subsequent spatial queries in the same frame
                                    // see up-to-date GlobalTransform values.
                                    if is_mutation_tool {
                                        propagate_transforms(world);
                                    }
                                }
                                Err(e) => {
                                    if let Some(ref label) = action_label {
                                        schedule.errored_labels.insert(label.clone());
                                    }
                                    schedule.results.push(ActionResult {
                                        index: schedule.current_index,
                                        label: action_label,
                                        tool: tool_name,
                                        at: action_at,
                                        fired_at_game_time: game_time,
                                        status: "error".to_string(),
                                        result: None,
                                        error: Some(e.message),
                                    });
                                    if schedule.stop_on_error {
                                        schedule.current_index += 1;
                                        abort_remaining(schedule, schedule.current_index);
                                        schedule.state = ScheduleState::Done;
                                        return;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            if let Some(ref label) = action_label {
                                schedule.errored_labels.insert(label.clone());
                            }
                            schedule.results.push(ActionResult {
                                index: schedule.current_index,
                                label: action_label,
                                tool: tool_name,
                                at: action_at,
                                fired_at_game_time: game_time,
                                status: "error".to_string(),
                                result: None,
                                error: Some(e),
                            });
                            if schedule.stop_on_error {
                                schedule.current_index += 1;
                                abort_remaining(schedule, schedule.current_index);
                                schedule.state = ScheduleState::Done;
                                return;
                            }
                        }
                    }
                    schedule.current_index += 1;
                    // Re-base t0 after time-manipulation tools (seek_time can
                    // move game_time backward below the original t0, which would
                    // stall all subsequent at:0 actions).
                    if is_time_tool {
                        schedule.t0_game_time = world
                            .get_resource::<Time<Virtual>>()
                            .map(|t| t.elapsed_secs_f64())
                            .unwrap_or(0.0);
                    }
                    update_async_progress(schedule);
                    continue; // Same-at actions fire in the same frame
                }
            }
        }
    }
}
fn setup_deferred_action(
    world: &mut World,
    tool: &str,
    args: &serde_json::Value,
) -> Result<oneshot::Receiver<Result<serde_json::Value, ControlError>>, String> {
    let (forward_tx, forward_rx) = oneshot::channel();

    match tool {
        "capture_screenshot" => {
            let op = tool_to_operation(tool, args).map_err(|e| e.to_string())?;
            let (delay_frames, max_width, position, look_at, hide_ui, with_gizmos) = match &op {
                ControlOperation::CaptureScreenshot {
                    delay_frames,
                    max_width,
                    position,
                    look_at,
                    hide_ui,
                } => (
                    *delay_frames,
                    *max_width,
                    *position,
                    *look_at,
                    *hide_ui,
                    false,
                ),
                ControlOperation::CaptureWithGizmos {
                    delay_frames,
                    max_width,
                    position,
                    look_at,
                    hide_ui,
                } => (
                    *delay_frames,
                    *max_width,
                    *position,
                    *look_at,
                    *hide_ui,
                    true,
                ),
                _ => unreachable!(),
            };

            let debug_camera = position.map(|pos| DebugCameraRequest {
                position: pos,
                look_at: look_at.unwrap_or([0.0, 0.0, 0.0]),
            });

            let mut pending = world.get_resource_or_insert_with(PendingScreenshots::default);
            pending.pending.push(PendingScreenshot {
                response_tx: forward_tx,
                frames_remaining: delay_frames,
                with_gizmos,
                max_width,
                debug_camera,
                hide_ui,
            });
            Ok(forward_rx)
        }

        "reload" => {
            let op = tool_to_operation(tool, args).map_err(|e| e.to_string())?;
            let (mode, pause, time_scale) = match &op {
                ControlOperation::TriggerReload {
                    mode,
                    pause,
                    time_scale,
                } => (mode.clone(), *pause, *time_scale),
                _ => unreachable!(),
            };

            let _ = handlers::reload::trigger_reload(world, mode.clone(), pause, time_scale);

            let error_ts = world
                .get_resource::<pybevy_core::LastSystemError>()
                .map(|e| e.timestamp_secs)
                .unwrap_or(0.0);

            let mut pending = world.get_resource_or_insert_with(PendingReloadResponses::default);
            pending.pending.push(crate::bridge::PendingReloadResponse {
                response_tx: forward_tx,
                frames_remaining: 5,
                mode,
                error_timestamp_before: error_ts,
            });
            Ok(forward_rx)
        }

        "reload_and_capture" => {
            let op = tool_to_operation(tool, args).map_err(|e| e.to_string())?;
            let (mode, pause, time_scale, delay_frames, max_width, position, look_at, hide_ui) =
                match &op {
                    ControlOperation::ReloadAndCapture {
                        mode,
                        pause,
                        time_scale,
                        delay_frames,
                        max_width,
                        position,
                        look_at,
                        hide_ui,
                    } => (
                        mode.clone(),
                        *pause,
                        *time_scale,
                        *delay_frames,
                        *max_width,
                        *position,
                        *look_at,
                        *hide_ui,
                    ),
                    _ => unreachable!(),
                };

            let _ = handlers::reload::trigger_reload(world, mode.clone(), pause, time_scale);

            let error_ts = world
                .get_resource::<pybevy_core::LastSystemError>()
                .map(|e| e.timestamp_secs)
                .unwrap_or(0.0);

            let mut pending = world.get_resource_or_insert_with(PendingReloadAndCaptures::default);
            pending.pending.push(PendingReloadAndCapture {
                response_tx: forward_tx,
                mode,
                error_timestamp_before: error_ts,
                reload_frames_remaining: 5,
                screenshot_delay_frames: delay_frames.unwrap_or(30),
                max_width,
                position,
                look_at,
                hide_ui: hide_ui.unwrap_or(true),
                state: ReloadAndCaptureState::WaitingForReload,
                reload_response: None,
            });
            Ok(forward_rx)
        }

        "capture_timeline" => {
            let op = tool_to_operation(tool, args).map_err(|e| e.to_string())?;
            let (total_frames, capture_count, max_width, columns, position, look_at) = match &op {
                ControlOperation::CaptureTimeline {
                    total_frames,
                    capture_count,
                    max_width,
                    columns,
                    position,
                    look_at,
                } => (
                    *total_frames,
                    *capture_count,
                    *max_width,
                    *columns,
                    *position,
                    *look_at,
                ),
                _ => unreachable!(),
            };

            let mut schedule_frames =
                crate::handlers::screenshot::compute_schedule(total_frames, capture_count);

            let debug_cleanup = position.map(|pos| {
                let debug_req = DebugCameraRequest {
                    position: pos,
                    look_at: look_at.unwrap_or([0.0, 0.0, 0.0]),
                };
                if let Some(first) = schedule_frames.front_mut() {
                    *first += 2;
                }
                setup_debug_camera(world, &debug_req)
            });

            let mut pending = world.get_resource_or_insert_with(PendingTimelines::default);
            let id = pending.next_id;
            pending.next_id += 1;
            pending.active.insert(
                id,
                ActiveTimeline {
                    response_tx: Some(forward_tx),
                    max_width,
                    columns,
                    debug_cleanup,
                    schedule: schedule_frames,
                    total_captures: capture_count,
                    next_capture_index: 0,
                    collected: Vec::new(),
                },
            );
            Ok(forward_rx)
        }

        "capture_turnaround" => {
            let op = tool_to_operation(tool, args).map_err(|e| e.to_string())?;
            let (
                look_at_opt,
                distance,
                elevation,
                view_count,
                include_top,
                columns,
                max_width,
                hide_ui,
            ) = match &op {
                ControlOperation::CaptureTurnaround {
                    look_at,
                    distance,
                    elevation,
                    view_count,
                    include_top,
                    columns,
                    max_width,
                    hide_ui,
                } => (
                    *look_at,
                    *distance,
                    *elevation,
                    *view_count,
                    *include_top,
                    *columns,
                    *max_width,
                    *hide_ui,
                ),
                _ => unreachable!(),
            };

            let vc = view_count.unwrap_or(6);
            let elev = elevation.unwrap_or(25.0);
            let top = include_top.unwrap_or(true);

            let (auto_look_at, auto_distance) = if distance.is_none() || look_at_opt.is_none() {
                if let Some((scene_min, scene_max)) = compute_scene_bounds(world) {
                    let center = (scene_min + scene_max) * 0.5;
                    let extent = scene_max - scene_min;
                    let diagonal =
                        (extent.x * extent.x + extent.y * extent.y + extent.z * extent.z).sqrt();
                    let dist = diagonal / (30.0_f32.to_radians().tan() * 2.0);
                    ([center.x, center.y, center.z], dist.max(2.0))
                } else {
                    ([0.0, 0.0, 0.0], 10.0)
                }
            } else {
                ([0.0, 0.0, 0.0], 10.0)
            };

            let final_look_at = look_at_opt.unwrap_or(auto_look_at);
            let final_distance = distance.unwrap_or(auto_distance);

            let viewpoints = compute_viewpoints(final_look_at, final_distance, elev, vc, top);

            let mut pending = world.get_resource_or_insert_with(PendingTurnarounds::default);
            pending.active.push(ActiveTurnaround {
                response_tx: Some(forward_tx),
                viewpoints,
                current_index: 0,
                captures: Vec::new(),
                columns: columns.unwrap_or(3),
                max_width: Some(max_width.unwrap_or(1200)),
                frames_remaining: 0,
                hide_ui: hide_ui.unwrap_or(true),
                ui_restore: None,
                debug_cleanup: None,
                look_at: final_look_at,
                pending_screenshot_entity: None,
            });
            Ok(forward_rx)
        }

        "capture_depth" => {
            let op = tool_to_operation(tool, args).map_err(|e| e.to_string())?;
            let (
                position,
                look_at,
                sample_points,
                grid_density,
                include_rgb,
                delay_frames,
                hide_ui,
                max_width,
            ) = match &op {
                ControlOperation::CaptureDepth {
                    position,
                    look_at,
                    sample_points,
                    grid_density,
                    include_rgb,
                    delay_frames,
                    hide_ui,
                    max_width,
                } => (
                    position.clone(),
                    look_at.clone(),
                    sample_points.clone(),
                    *grid_density,
                    *include_rgb,
                    *delay_frames,
                    *hide_ui,
                    *max_width,
                ),
                _ => unreachable!(),
            };

            // Compute depth samples synchronously
            let depth_result = crate::handlers::depth::compute_depth_samples(
                world,
                &position,
                &look_at,
                &sample_points,
                &grid_density,
            );

            let want_rgb = include_rgb.unwrap_or(true);
            let df = delay_frames.unwrap_or(2);
            let mw = Some(max_width.unwrap_or(768));
            let hu = hide_ui.unwrap_or(true);
            let dc = position.as_ref().map(|pos| DebugCameraRequest {
                position: *pos,
                look_at: look_at.unwrap_or([0.0, 0.0, 0.0]),
            });

            if want_rgb {
                let (screenshot_tx, screenshot_rx) =
                    oneshot::channel::<Result<serde_json::Value, ControlError>>();

                std::thread::spawn(move || {
                    let screenshot_result = screenshot_rx.blocking_recv();
                    let response = match (screenshot_result, depth_result) {
                        (Ok(Ok(sj)), Ok(depth)) => Ok(serde_json::json!({
                            "screenshot": sj.get("image"),
                            "screenshot_width": sj.get("width"),
                            "screenshot_height": sj.get("height"),
                            "depth_samples": depth,
                        })),
                        (_, Ok(depth)) => Ok(serde_json::json!({
                            "screenshot": null,
                            "depth_samples": depth,
                        })),
                        (_, Err(e)) => Err(e),
                    };
                    let _ = forward_tx.send(response);
                });

                let mut pending = world.get_resource_or_insert_with(PendingScreenshots::default);
                pending.pending.push(PendingScreenshot {
                    response_tx: screenshot_tx,
                    frames_remaining: df,
                    with_gizmos: false,
                    max_width: mw,
                    debug_camera: dc,
                    hide_ui: hu,
                });
            } else {
                let result = depth_result.map(|depth| {
                    serde_json::json!({
                        "screenshot": null,
                        "depth_samples": depth,
                    })
                });
                let _ = forward_tx.send(result);
            }
            Ok(forward_rx)
        }

        _ => Err(format!("Cannot defer tool '{}'", tool)),
    }
}
fn abort_remaining(schedule: &mut ActiveSchedule, from_index: usize) {
    for idx in from_index..schedule.actions.len() {
        let action = &schedule.actions[idx];
        schedule.results.push(ActionResult {
            index: idx,
            label: action.label.clone(),
            tool: action.tool.clone(),
            at: resolve_at(action),
            fired_at_game_time: 0.0,
            status: "aborted".to_string(),
            result: None,
            error: Some("Aborted due to stop_on_error".to_string()),
        });
    }
}

fn update_async_progress(schedule: &ActiveSchedule) {
    if let Some(ref shared) = schedule.async_shared {
        if let Ok(mut guard) = shared.lock() {
            guard.completed_actions = schedule.results.len();
            guard.results = schedule.results.clone();
        }
    }
}

fn finalize_schedule(schedule: ActiveSchedule) {
    let game_times: Vec<f64> = schedule
        .results
        .iter()
        .map(|r| r.fired_at_game_time)
        .collect();
    let end_time = game_times.iter().copied().fold(0.0_f64, f64::max);

    let response = serde_json::json!({
        "schedule_id": schedule.schedule_id,
        "status": "completed",
        "total_actions": schedule.actions.len(),
        "completed_actions": schedule.results.len(),
        "start_game_time": schedule.t0_game_time,
        "end_game_time": end_time,
        "results": schedule.results,
    });

    if let Some(tx) = schedule.sync_response_tx {
        let _ = tx.send(Ok(response));
    }

    if let Some(ref shared) = schedule.async_shared {
        if let Ok(mut guard) = shared.lock() {
            guard.status = "completed".to_string();
            guard.completed_actions = schedule.results.len();
            guard.results = schedule.results;
        }
    }
}
