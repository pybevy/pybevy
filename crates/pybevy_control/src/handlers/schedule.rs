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
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::{
    bridge::{
        ControlError, ControlOperation, PendingScreenshots, push_pending_depth,
        push_pending_screenshot, push_pending_timeline, push_pending_turnaround,
    },
    handlers,
};

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
pub struct ScheduleAction {
    /// Tool name to call
    pub tool: String,
    /// Tool arguments (default {})
    #[serde(default)]
    pub args: serde_json::Value,
    /// Time offset in **virtual** seconds from schedule start (default 0). Must be monotonically
    /// non-decreasing. Note: virtual seconds do not advance while time is paused, so a schedule
    /// that calls `pause_time` cannot use `at` for any later action - use `at_frame` instead, or
    /// call `resume_time` from outside the schedule.
    pub at: Option<f64>,
    /// Frame offset from schedule start (alternative to 'at'). Cannot mix with 'at'.
    pub at_frame: Option<u64>,
    /// Label for this action (used by skip_if_error)
    pub label: Option<String>,
    /// Skip this action if the action with the given label errored
    pub skip_if_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionResult {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub tool: String,
    pub at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_frame: Option<u64>,
    pub fired_at_game_time: f64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ActionResult {
    /// Build a result mirroring an action's timing fields.
    fn new(
        action: &ScheduleAction,
        index: usize,
        fired_at_game_time: f64,
        status: impl Into<String>,
    ) -> Self {
        Self {
            index,
            label: action.label.clone(),
            tool: action.tool.clone(),
            at: action.at.unwrap_or(0.0),
            at_frame: action.at_frame,
            fired_at_game_time,
            status: status.into(),
            result: None,
            error: None,
        }
    }

    fn with_result(mut self, result: serde_json::Value) -> Self {
        self.result = Some(result);
        self
    }

    fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }
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
        if let Ok(guard) = self.inner.lock()
            && let Some(arc) = guard.get(id)
            && let Ok(mut state) = arc.lock()
        {
            state.cancelled = true;
            return true;
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
    // reload tools cannot run inside a schedule: the schedule blocks waiting
    // for the deferred response (ScheduleState::WaitingForDeferred), but the
    // reload itself needs the app loop to drop and re-enter, which can't
    // happen while process_active_schedules is mid-flight. Result: 120 s
    // engine timeout. Reject at validation instead.
    "reload",
    "reload_and_capture",
];

fn is_non_schedulable(tool: &str) -> bool {
    NON_SCHEDULABLE_TOOLS.contains(&tool)
}

fn is_deferred_tool(name: &str) -> bool {
    matches!(
        name,
        "capture_screenshot" | "capture_timeline" | "capture_turnaround" | "capture_depth"
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
            map.remove("gizmos");
            "capture_with_gizmos"
        }
        "query_spatial" if map.contains_key("radius") => "query_spatial_neighborhood",
        "check_overlaps" if !map.contains_key("entity") => "check_all_overlaps",
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
fn resolve_at(action: &ScheduleAction) -> f64 {
    action.at.unwrap_or(0.0)
}

fn resolve_at_frame(action: &ScheduleAction) -> Option<u64> {
    action.at_frame
}

/// Returns true when virtual time cannot advance — either it is paused, or its
/// relative speed is zero. Either condition means an action gated on `at`
/// (virtual seconds) can never become ready.
fn virtual_time_is_stalled(world: &World) -> bool {
    world
        .get_resource::<Time<Virtual>>()
        .map(|t| t.is_paused() || t.relative_speed() == 0.0)
        .unwrap_or(false)
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

    // Remove runtime from World to use for dispatch (same scope as old Python::attach)
    let mut runtime = world
        .remove_non_send_resource::<Box<dyn crate::runtime::ControlRuntime>>()
        .expect("ControlRuntime resource missing");

    {
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
                    schedule.results.push(
                        ActionResult::new(action, schedule.current_index, 0.0, "cancelled")
                            .with_error("Schedule cancelled"),
                    );
                    schedule.current_index += 1;
                }
                schedule.state = ScheduleState::Done;
            }

            process_single_schedule(world, &mut schedules.schedules[i], &mut *runtime);
            schedules.schedules[i].frame_counter += 1;

            if schedules.schedules[i].state == ScheduleState::Done {
                let schedule = schedules.schedules.remove(i);
                finalize_schedule(schedule);
            } else {
                i += 1;
            }
        }
    }

    world.insert_non_send_resource(runtime);

    world.insert_resource(schedules);
}

fn process_single_schedule(
    world: &mut World,
    schedule: &mut ActiveSchedule,
    runtime: &mut dyn crate::runtime::ControlRuntime,
) {
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
                                    crate::handlers::pyo3::mutate::has_embedded_errors(&value);
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
                                        let mut r = ActionResult::new(
                                            action,
                                            schedule.current_index,
                                            game_time,
                                            status,
                                        )
                                        .with_result(value);
                                        if let Some(msg) = error_msg {
                                            r = r.with_error(msg);
                                        }
                                        schedule.results.push(r);
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
                                let mut r = ActionResult::new(
                                    action,
                                    schedule.current_index,
                                    game_time,
                                    status,
                                )
                                .with_result(value);
                                if let Some(msg) = error_msg {
                                    r = r.with_error(msg);
                                }
                                schedule.results.push(r);
                            }
                            Err(e) => {
                                if let Some(ref label) = action.label {
                                    schedule.errored_labels.insert(label.clone());
                                }
                                schedule.results.push(
                                    ActionResult::new(
                                        action,
                                        schedule.current_index,
                                        game_time,
                                        "error",
                                    )
                                    .with_error(e.message),
                                );
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
                        schedule.results.push(
                            ActionResult::new(action, schedule.current_index, 0.0, "error")
                                .with_error("Deferred channel closed unexpectedly"),
                        );
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
                let uses_frame_offset = resolve_at_frame(action).is_some();
                let ready = if let Some(frame_offset) = resolve_at_frame(action) {
                    schedule.frame_counter >= frame_offset
                } else {
                    let target_time = schedule.t0_game_time + resolve_at(action);
                    game_time >= target_time
                };

                if !ready {
                    // Detect self-deadlock: if the next action gates on virtual
                    // time but a previous action paused (or zero-scaled) virtual
                    // time, virtual seconds will never advance — including any
                    // later resume_time, since it too sits behind an `at` that
                    // can never fire. Abort immediately rather than waiting for
                    // the engine's 120 s timeout.
                    if !uses_frame_offset && virtual_time_is_stalled(world) {
                        let action_at = resolve_at(action);
                        let action_label = action.label.clone();
                        let action_tool = action.tool.clone();
                        let error_msg = format!(
                            "Schedule self-deadlocked: virtual time is paused (or set to 0x \
                             scale), so action[{}] tool='{}' at={}s can never fire — virtual \
                             seconds do not advance while paused. Use 'at_frame' for actions \
                             that should run after pause_time/set_time_scale(0), or call \
                             resume_time from outside the schedule.",
                            schedule.current_index, action_tool, action_at,
                        );
                        if let Some(ref label) = action_label {
                            schedule.errored_labels.insert(label.clone());
                        }
                        schedule.results.push(ActionResult {
                            index: schedule.current_index,
                            label: action_label,
                            tool: action_tool,
                            at: action_at,
                            at_frame: action.at_frame,
                            fired_at_game_time: game_time,
                            status: "error".to_string(),
                            result: None,
                            error: Some(error_msg),
                        });
                        schedule.current_index += 1;
                        abort_remaining(schedule, schedule.current_index);
                        schedule.state = ScheduleState::Done;
                        update_async_progress(schedule);
                        return;
                    }
                    return; // Try next frame
                }

                // Check skip_if_error
                if let Some(ref skip_label) = action.skip_if_error
                    && schedule.errored_labels.contains(skip_label)
                {
                    schedule.results.push(
                        ActionResult::new(action, schedule.current_index, game_time, "skipped")
                            .with_error(format!("Skipped due to error in '{}'", skip_label)),
                    );
                    schedule.current_index += 1;
                    update_async_progress(schedule);
                    continue;
                }

                // Execute the action
                let tool_name = action.tool.clone();
                let tool_args = action.args.clone();
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
                            if let Some(ref label) = action.label {
                                schedule.errored_labels.insert(label.clone());
                            }
                            let r = ActionResult::new(
                                action,
                                schedule.current_index,
                                game_time,
                                "error",
                            )
                            .with_error(e);
                            schedule.results.push(r);
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
                            let result = handlers::dispatch(world, op, runtime);
                            match result {
                                Ok(value) => {
                                    let has_errors =
                                        crate::handlers::pyo3::mutate::has_embedded_errors(&value);
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
                                            let mut r = ActionResult::new(
                                                action,
                                                schedule.current_index,
                                                game_time,
                                                status,
                                            )
                                            .with_result(value);
                                            if let Some(msg) = error_msg {
                                                r = r.with_error(msg);
                                            }
                                            schedule.results.push(r);
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
                                    let mut r = ActionResult::new(
                                        action,
                                        schedule.current_index,
                                        game_time,
                                        status,
                                    )
                                    .with_result(value);
                                    if let Some(msg) = error_msg {
                                        r = r.with_error(msg);
                                    }
                                    schedule.results.push(r);

                                    // After successful dispatch, sync transforms if
                                    // the tool could have modified them so that
                                    // subsequent spatial queries in the same frame
                                    // see up-to-date GlobalTransform values.
                                    if is_mutation_tool {
                                        propagate_transforms(world);
                                    }
                                }
                                Err(e) => {
                                    if let Some(ref label) = action.label {
                                        schedule.errored_labels.insert(label.clone());
                                    }
                                    let r = ActionResult::new(
                                        action,
                                        schedule.current_index,
                                        game_time,
                                        "error",
                                    )
                                    .with_error(e.message);
                                    schedule.results.push(r);
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
                            if let Some(ref label) = action.label {
                                schedule.errored_labels.insert(label.clone());
                            }
                            let r = ActionResult::new(
                                action,
                                schedule.current_index,
                                game_time,
                                "error",
                            )
                            .with_error(e);
                            schedule.results.push(r);
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
    let op = tool_to_operation(tool, args)?;
    let (tx, rx) = oneshot::channel();
    let mut screenshots = Vec::new();

    match op {
        ControlOperation::CaptureScreenshot(p) => {
            push_pending_screenshot(p, false, tx, &mut screenshots)
        }
        ControlOperation::CaptureWithGizmos(p) => {
            push_pending_screenshot(p, true, tx, &mut screenshots)
        }
        ControlOperation::CaptureTimeline(p) => push_pending_timeline(p, tx, world),
        ControlOperation::CaptureTurnaround(p) => push_pending_turnaround(p, tx, world),
        ControlOperation::CaptureDepth(p) => push_pending_depth(p, tx, &mut screenshots, world),
        _ => return Err(format!("tool '{tool}' is not deferrable")),
    }

    if !screenshots.is_empty() {
        let mut pending = world.get_resource_or_insert_with(PendingScreenshots::default);
        pending.pending.extend(screenshots);
    }

    Ok(rx)
}
fn abort_remaining(schedule: &mut ActiveSchedule, from_index: usize) {
    for idx in from_index..schedule.actions.len() {
        let action = &schedule.actions[idx];
        let r = ActionResult::new(action, idx, 0.0, "aborted")
            .with_error("Aborted due to stop_on_error");
        schedule.results.push(r);
    }
}

fn update_async_progress(schedule: &ActiveSchedule) {
    if let Some(ref shared) = schedule.async_shared
        && let Ok(mut guard) = shared.lock()
    {
        guard.completed_actions = schedule.results.len();
        guard.results = schedule.results.clone();
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

    if let Some(ref shared) = schedule.async_shared
        && let Ok(mut guard) = shared.lock()
    {
        guard.status = "completed".to_string();
        guard.completed_actions = schedule.results.len();
        guard.results = schedule.results;
    }
}
#[cfg(test)]
mod tests {
    use bevy::math::Vec3;

    use super::*;
    use crate::{
        bridge::{
            CaptureScreenshotParams, EntityRef, GetComponentParams, ReloadMode, ReloadParams,
            SeekTimeParams, SetComponentParams,
        },
        handlers::pyo3::mutate::has_embedded_errors,
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
    fn test_deserialize_invalid_mode() {
        let json = r#"{"actions": [{"tool": "pause_time"}], "mode": "invalid"}"#;
        let result: Result<ScheduleRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
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
                    skip_if_error: None,
                },
            ],
            mode: ScheduleMode::Sync,
            stop_on_error: false,
        };
        assert!(validate_schedule(&req).is_ok());
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
    fn test_tool_to_operation_unknown() {
        let result = tool_to_operation("nonexistent_tool", &serde_json::Value::Null);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_deferred_tool() {
        assert!(is_deferred_tool("capture_screenshot"));
        assert!(is_deferred_tool("capture_timeline"));
        assert!(is_deferred_tool("capture_turnaround"));
        assert!(is_deferred_tool("capture_depth"));
        // reload variants are NOT deferrable inside a schedule: see
        // NON_SCHEDULABLE_TOOLS for the deadlock rationale.
        assert!(!is_deferred_tool("reload"));
        assert!(!is_deferred_tool("reload_and_capture"));
        assert!(!is_deferred_tool("pause_time"));
        assert!(!is_deferred_tool("set_component"));
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
    fn test_action_result_serialization() {
        let result = ActionResult {
            index: 0,
            label: Some("test".to_string()),
            tool: "pause_time".to_string(),
            at: 0.0,
            at_frame: None,
            fired_at_game_time: 1.5,
            status: "ok".to_string(),
            result: Some(serde_json::json!({"paused": true})),
            error: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["index"], 0);
        assert_eq!(json["label"], "test");
        assert_eq!(json["status"], "ok");
        assert!(json.get("error").is_none()); // skip_serializing_if
        assert!(json.get("at_frame").is_none()); // skip_serializing_if
    }

    #[test]
    fn test_action_result_at_frame_round_trip() {
        // Bug #301: at_frame must surface in the response.
        let action = ScheduleAction {
            tool: "pause_time".to_string(),
            args: serde_json::Value::Null,
            at: None,
            at_frame: Some(30),
            label: None,
            skip_if_error: None,
        };
        let r = ActionResult::new(&action, 0, 0.5, "ok")
            .with_result(serde_json::json!({"paused": true}));
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["at"], 0.0);
        assert_eq!(json["at_frame"], 30);
        assert_eq!(json["status"], "ok");
        assert_eq!(json["tool"], "pause_time");
    }

    #[test]
    fn test_shared_schedule_registry() {
        let registry = SharedScheduleRegistry::default();
        let shared = Arc::new(Mutex::new(SharedScheduleState::new("s-1", 3)));
        registry.insert("s-1".to_string(), shared.clone());

        let state = registry.get("s-1").unwrap();
        assert_eq!(state.schedule_id, "s-1");
        assert_eq!(state.status, "running");
        assert_eq!(state.total_actions, 3);

        // Update via the Arc directly (same as schedule state machine does)
        shared.lock().unwrap().completed_actions = 2;
        let state = registry.get("s-1").unwrap();
        assert_eq!(state.completed_actions, 2);

        assert!(registry.cancel("s-1"));
        let state = registry.get("s-1").unwrap();
        assert!(state.cancelled);

        assert!(!registry.cancel("nonexistent"));
        assert!(registry.get("nonexistent").is_none());
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
    fn test_abort_remaining() {
        let mut schedule = ActiveSchedule {
            schedule_id: "test".to_string(),
            actions: vec![
                ScheduleAction {
                    tool: "pause_time".to_string(),
                    args: serde_json::Value::Null,
                    at: Some(0.0),
                    at_frame: None,
                    label: Some("a".to_string()),
                    skip_if_error: None,
                },
                ScheduleAction {
                    tool: "resume_time".to_string(),
                    args: serde_json::Value::Null,
                    at: Some(1.0),
                    at_frame: None,
                    label: Some("b".to_string()),
                    skip_if_error: None,
                },
            ],
            results: vec![],
            current_index: 0,
            state: ScheduleState::WaitingForTime,
            t0_game_time: 0.0,
            frame_counter: 0,
            stop_on_error: true,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: None,
            deferred_rx: None,
        };
        abort_remaining(&mut schedule, 0);
        assert_eq!(schedule.results.len(), 2);
        assert_eq!(schedule.results[0].status, "aborted");
        assert_eq!(schedule.results[1].status, "aborted");
    }

    #[test]
    fn test_is_time_control_tool() {
        assert!(is_time_control_tool("seek_time"));
        assert!(is_time_control_tool("pause_time"));
        assert!(is_time_control_tool("resume_time"));
        assert!(is_time_control_tool("set_time_scale"));
        assert!(!is_time_control_tool("capture_screenshot"));
        assert!(!is_time_control_tool("set_component"));
        assert!(!is_time_control_tool("query_entities"));
        assert!(!is_time_control_tool("get_scene_summary"));
    }

    /// Verifies that the registry and ActiveSchedule share the same Arc,
    /// so updates from the schedule state machine are visible via registry.get().
    /// This was bug #3: the registry previously stored a separate copy.
    #[test]
    fn test_registry_shares_arc_with_schedule() {
        let registry = SharedScheduleRegistry::default();
        let shared = Arc::new(Mutex::new(SharedScheduleState::new("s-1", 2)));
        registry.insert("s-1".to_string(), shared.clone());

        // Simulate what finalize_schedule does
        {
            let mut guard = shared.lock().unwrap();
            guard.status = "completed".to_string();
            guard.completed_actions = 2;
            guard.results.push(ActionResult {
                index: 0,
                label: Some("a".to_string()),
                tool: "pause_time".to_string(),
                at: 0.0,
                at_frame: None,
                fired_at_game_time: 1.0,
                status: "ok".to_string(),
                result: Some(serde_json::json!({"paused": true})),
                error: None,
            });
        }

        // Registry should see the updated state via the shared Arc
        let state = registry.get("s-1").unwrap();
        assert_eq!(state.status, "completed");
        assert_eq!(state.completed_actions, 2);
        assert_eq!(state.results.len(), 1);
        assert_eq!(state.results[0].tool, "pause_time");
    }

    /// Verifies cancel propagates through the shared Arc so the schedule
    /// state machine sees it on the next frame poll.
    #[test]
    fn test_registry_cancel_visible_through_arc() {
        let registry = SharedScheduleRegistry::default();
        let shared = Arc::new(Mutex::new(SharedScheduleState::new("s-2", 1)));
        registry.insert("s-2".to_string(), shared.clone());

        // Cancel via registry (HTTP handler path)
        assert!(registry.cancel("s-2"));

        // Schedule state machine reads from its own Arc clone
        let guard = shared.lock().unwrap();
        assert!(guard.cancelled);
    }

    #[test]
    fn propagate_transforms_updates_root() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::default(),
            ))
            .id();

        // Modify Transform without running propagation
        world.get_mut::<Transform>(entity).unwrap().translation = Vec3::new(10.0, 20.0, 30.0);

        // GlobalTransform should still be at origin
        let gt = world.get::<GlobalTransform>(entity).unwrap();
        assert_eq!(gt.translation(), Vec3::ZERO);

        // Propagate
        propagate_transforms(&mut world);

        // Now GlobalTransform should match
        let gt = world.get::<GlobalTransform>(entity).unwrap();
        assert_eq!(gt.translation(), Vec3::new(10.0, 20.0, 30.0));
    }

    #[test]
    fn schedule_actions_run_code_failure_status() {
        // Simulate what happens when run_code returns {"success": false, "error": "..."}
        let value = serde_json::json!({"success": false, "error": "NameError: x is not defined"});
        let has_errors = has_embedded_errors(&value);
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
        assert!(!has_errors, "run_code failure has no 'errors' array");
        assert!(has_run_code_failure, "run_code failure has success=false");
        assert_eq!(status, "error");
    }

    #[test]
    fn schedule_actions_stop_on_error_with_run_code() {
        // Verify that a run_code failure with success=false triggers stop_on_error logic.
        // The stop_on_error path calls abort_remaining, so we test that the remaining
        // actions would be aborted after a run_code failure.
        let mut schedule = ActiveSchedule {
            schedule_id: "test-stop".to_string(),
            actions: vec![
                ScheduleAction {
                    tool: "run_code".to_string(),
                    args: serde_json::json!({"code": "x"}),
                    at: Some(0.0),
                    at_frame: None,
                    label: Some("failing_code".to_string()),
                    skip_if_error: None,
                },
                ScheduleAction {
                    tool: "pause_time".to_string(),
                    args: serde_json::Value::Null,
                    at: Some(0.0),
                    at_frame: None,
                    label: Some("should_abort".to_string()),
                    skip_if_error: None,
                },
            ],
            results: vec![],
            current_index: 1, // Simulate that first action already processed
            state: ScheduleState::WaitingForTime,
            t0_game_time: 0.0,
            frame_counter: 0,
            stop_on_error: true,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: None,
            deferred_rx: None,
        };
        abort_remaining(&mut schedule, 1);
        assert_eq!(schedule.results.len(), 1);
        assert_eq!(schedule.results[0].status, "aborted");
        assert_eq!(schedule.results[0].tool, "pause_time");
    }

    #[test]
    fn schedule_actions_run_code_success_is_ok() {
        // Verify that run_code with success=true gets status "ok"
        let value = serde_json::json!({"success": true});
        let has_errors = has_embedded_errors(&value);
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
        assert_eq!(status, "ok");
    }

    #[test]
    fn test_is_transform_mutation_tool() {
        assert!(is_transform_mutation_tool("set_component"));
        assert!(is_transform_mutation_tool("spawn_entity"));
        assert!(is_transform_mutation_tool("batch"));
        assert!(!is_transform_mutation_tool("pause_time"));
        assert!(!is_transform_mutation_tool("query_entities"));
        assert!(!is_transform_mutation_tool("capture_screenshot"));
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
            &serde_json::json!({"with": ["Transform", "Velocity"], "without": ["Marker"]}),
        )
        .unwrap();
        assert!(matches!(op, ControlOperation::QueryEntities(..)));
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
    fn test_finalize_schedule_sync() {
        let (tx, rx) = oneshot::channel();
        let schedule = ActiveSchedule {
            schedule_id: "s-1".to_string(),
            actions: vec![ScheduleAction {
                tool: "pause_time".to_string(),
                args: serde_json::Value::Null,
                at: Some(0.0),
                at_frame: None,
                label: None,
                skip_if_error: None,
            }],
            results: vec![ActionResult {
                index: 0,
                label: None,
                tool: "pause_time".to_string(),
                at: 0.0,
                at_frame: None,
                fired_at_game_time: 1.0,
                status: "ok".to_string(),
                result: Some(serde_json::json!({"paused": true})),
                error: None,
            }],
            current_index: 1,
            state: ScheduleState::Done,
            t0_game_time: 0.0,
            frame_counter: 1,
            stop_on_error: false,
            errored_labels: HashSet::new(),
            sync_response_tx: Some(tx),
            async_shared: None,
            deferred_rx: None,
        };
        finalize_schedule(schedule);
        let response = rx.blocking_recv().unwrap().unwrap();
        assert_eq!(response["schedule_id"], "s-1");
        assert_eq!(response["status"], "completed");
        assert_eq!(response["total_actions"], 1);
        assert_eq!(response["completed_actions"], 1);
    }

    #[test]
    fn test_finalize_schedule_async() {
        let shared = Arc::new(Mutex::new(SharedScheduleState::new("s-2", 1)));
        let schedule = ActiveSchedule {
            schedule_id: "s-2".to_string(),
            actions: vec![ScheduleAction {
                tool: "resume_time".to_string(),
                args: serde_json::Value::Null,
                at: Some(0.0),
                at_frame: None,
                label: None,
                skip_if_error: None,
            }],
            results: vec![ActionResult {
                index: 0,
                label: None,
                tool: "resume_time".to_string(),
                at: 0.0,
                at_frame: None,
                fired_at_game_time: 2.0,
                status: "ok".to_string(),
                result: None,
                error: None,
            }],
            current_index: 1,
            state: ScheduleState::Done,
            t0_game_time: 0.0,
            frame_counter: 1,
            stop_on_error: false,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: Some(shared.clone()),
            deferred_rx: None,
        };
        finalize_schedule(schedule);
        let guard = shared.lock().unwrap();
        assert_eq!(guard.status, "completed");
        assert_eq!(guard.completed_actions, 1);
    }

    #[test]
    fn test_update_async_progress() {
        let shared = Arc::new(Mutex::new(SharedScheduleState::new("s-3", 2)));
        let schedule = ActiveSchedule {
            schedule_id: "s-3".to_string(),
            actions: vec![],
            results: vec![
                ActionResult {
                    index: 0,
                    label: None,
                    tool: "a".to_string(),
                    at: 0.0,
                    at_frame: None,
                    fired_at_game_time: 0.0,
                    status: "ok".to_string(),
                    result: None,
                    error: None,
                },
                ActionResult {
                    index: 1,
                    label: None,
                    tool: "b".to_string(),
                    at: 0.0,
                    at_frame: None,
                    fired_at_game_time: 0.0,
                    status: "ok".to_string(),
                    result: None,
                    error: None,
                },
            ],
            current_index: 2,
            state: ScheduleState::Done,
            t0_game_time: 0.0,
            frame_counter: 0,
            stop_on_error: false,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: Some(shared.clone()),
            deferred_rx: None,
        };
        update_async_progress(&schedule);
        let guard = shared.lock().unwrap();
        assert_eq!(guard.completed_actions, 2);
        assert_eq!(guard.results.len(), 2);
    }

    #[test]
    fn test_update_async_progress_no_shared() {
        // Should not panic when async_shared is None (sync schedule)
        let schedule = ActiveSchedule {
            schedule_id: "s-4".to_string(),
            actions: vec![],
            results: vec![],
            current_index: 0,
            state: ScheduleState::Done,
            t0_game_time: 0.0,
            frame_counter: 0,
            stop_on_error: false,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: None,
            deferred_rx: None,
        };
        update_async_progress(&schedule); // No-op, should not panic
    }

    #[test]
    fn test_schedule_request_deserialization_defaults() {
        let json = r#"{"actions": [{"tool": "pause_time"}]}"#;
        let req: ScheduleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mode, ScheduleMode::Sync);
        assert_eq!(req.stop_on_error, false);
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

    #[test]
    fn test_shared_schedule_state_new() {
        let state = SharedScheduleState::new("test-id", 5);
        assert_eq!(state.schedule_id, "test-id");
        assert_eq!(state.status, "running");
        assert_eq!(state.total_actions, 5);
        assert_eq!(state.completed_actions, 0);
        assert!(state.results.is_empty());
        assert!(!state.cancelled);
    }

    #[test]
    fn test_active_schedule_new_sync() {
        let (tx, _rx) = oneshot::channel();
        let req = ScheduleRequest {
            actions: vec![ScheduleAction {
                tool: "pause_time".to_string(),
                args: serde_json::Value::Null,
                at: Some(0.0),
                at_frame: None,
                label: None,
                skip_if_error: None,
            }],
            mode: ScheduleMode::Sync,
            stop_on_error: true,
        };
        let schedule = ActiveSchedule::new_sync("s-test".to_string(), req, 10.0, tx);
        assert_eq!(schedule.schedule_id, "s-test");
        assert_eq!(schedule.t0_game_time, 10.0);
        assert!(schedule.stop_on_error);
        assert!(schedule.sync_response_tx.is_some());
        assert!(schedule.async_shared.is_none());
    }

    #[test]
    fn test_active_schedule_new_async() {
        let shared = Arc::new(Mutex::new(SharedScheduleState::new("s-async", 1)));
        let req = ScheduleRequest {
            actions: vec![ScheduleAction {
                tool: "resume_time".to_string(),
                args: serde_json::Value::Null,
                at: None,
                at_frame: None,
                label: None,
                skip_if_error: None,
            }],
            mode: ScheduleMode::Async,
            stop_on_error: false,
        };
        let schedule = ActiveSchedule::new_async("s-async".to_string(), req, 0.0, shared);
        assert_eq!(schedule.schedule_id, "s-async");
        assert!(!schedule.stop_on_error);
        assert!(schedule.sync_response_tx.is_none());
        assert!(schedule.async_shared.is_some());
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
    fn propagate_transforms_updates_children() {
        let mut world = World::new();

        // Parent at x=100
        let parent = world
            .spawn((
                Transform::from_xyz(100.0, 0.0, 0.0),
                GlobalTransform::default(),
            ))
            .id();

        // Child at local x=40
        let child = world
            .spawn((
                Transform::from_xyz(40.0, 0.0, 0.0),
                GlobalTransform::default(),
            ))
            .id();

        // Establish hierarchy
        world.entity_mut(parent).add_children(&[child]);

        // Move parent to x=140
        world.get_mut::<Transform>(parent).unwrap().translation.x = 140.0;

        // Before propagation, child GlobalTransform is stale
        let gt = world.get::<GlobalTransform>(child).unwrap();
        assert_eq!(gt.translation().x, 0.0);

        // Propagate
        propagate_transforms(&mut world);

        // Parent should be at x=140
        let gt = world.get::<GlobalTransform>(parent).unwrap();
        assert_eq!(gt.translation().x, 140.0);

        // Child should be at x=140+40=180
        let gt = world.get::<GlobalTransform>(child).unwrap();
        assert_eq!(gt.translation().x, 180.0);
    }

    #[test]
    fn virtual_time_is_stalled_detects_paused() {
        let mut world = World::new();
        let mut time = Time::<Virtual>::default();
        time.pause();
        world.insert_resource(time);
        assert!(virtual_time_is_stalled(&world));
    }

    #[test]
    fn virtual_time_is_stalled_detects_running() {
        let mut world = World::new();
        world.insert_resource(Time::<Virtual>::default());
        assert!(!virtual_time_is_stalled(&world));
    }

    #[test]
    fn virtual_time_is_stalled_missing_resource_is_false() {
        let world = World::new();
        // Missing resource: be conservative and treat as not stalled so we
        // don't spuriously abort schedules in environments without Time<Virtual>.
        assert!(!virtual_time_is_stalled(&world));
    }

    /// Drives the WaitingForTime branch directly via a stripped-down clone of
    /// the deadlock-detection logic. We avoid invoking process_single_schedule
    /// here because that requires a full ControlRuntime impl; the salient
    /// behaviour — abort with a clear error when virtual time is paused and
    /// the next action gates on a future virtual-time target — is exercised
    /// end-to-end by the Python integration tests under tests/mcp/.
    #[test]
    fn schedule_deadlock_guard_aborts_when_virtual_time_paused() {
        let mut world = World::new();
        let mut time = Time::<Virtual>::default();
        time.pause();
        world.insert_resource(time);

        let mut schedule = ActiveSchedule {
            schedule_id: "deadlock-test".to_string(),
            actions: vec![ScheduleAction {
                tool: "get_time_status".to_string(),
                args: serde_json::Value::Null,
                at: Some(0.5),
                at_frame: None,
                label: Some("future".to_string()),
                skip_if_error: None,
            }],
            results: vec![],
            current_index: 0,
            state: ScheduleState::WaitingForTime,
            t0_game_time: 0.0,
            frame_counter: 0,
            stop_on_error: false,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: None,
            deferred_rx: None,
        };

        // Reproduce the relevant slice of process_single_schedule's
        // WaitingForTime branch. Keep this in sync with the production code.
        let action = schedule.actions[schedule.current_index].clone();
        let game_time = world
            .get_resource::<Time<Virtual>>()
            .map(|t| t.elapsed_secs_f64())
            .unwrap_or(0.0);
        let uses_frame_offset = resolve_at_frame(&action).is_some();
        let target_time = schedule.t0_game_time + resolve_at(&action);
        let ready = uses_frame_offset || game_time >= target_time;
        assert!(!ready, "test setup: action should not be ready");
        assert!(virtual_time_is_stalled(&world));

        let action_at = resolve_at(&action);
        let action_label = action.label.clone();
        let action_tool = action.tool.clone();
        if let Some(ref label) = action_label {
            schedule.errored_labels.insert(label.clone());
        }
        schedule.results.push(ActionResult {
            index: schedule.current_index,
            label: action_label,
            tool: action_tool,
            at: action_at,
            fired_at_game_time: game_time,
            status: "error".to_string(),
            result: None,
            error: Some("Schedule self-deadlocked: virtual time is paused".to_string()),
        });
        schedule.current_index += 1;
        let from = schedule.current_index;
        abort_remaining(&mut schedule, from);
        schedule.state = ScheduleState::Done;

        assert_eq!(schedule.state, ScheduleState::Done);
        assert_eq!(schedule.results.len(), 1);
        assert_eq!(schedule.results[0].status, "error");
        assert!(
            schedule.results[0]
                .error
                .as_deref()
                .unwrap()
                .contains("self-deadlocked")
        );
    }

    /// Verifies that a follow-up action still in the schedule gets aborted
    /// (rather than left as a phantom pending entry) when the deadlock guard
    /// trips on the current action.
    #[test]
    fn schedule_deadlock_guard_aborts_remaining_followup_actions() {
        let mut schedule = ActiveSchedule {
            schedule_id: "abort-followups".to_string(),
            actions: vec![
                ScheduleAction {
                    tool: "get_time_status".to_string(),
                    args: serde_json::Value::Null,
                    at: Some(0.5),
                    at_frame: None,
                    label: Some("blocked".to_string()),
                    skip_if_error: None,
                },
                ScheduleAction {
                    tool: "resume_time".to_string(),
                    args: serde_json::Value::Null,
                    at: Some(1.0),
                    at_frame: None,
                    label: Some("never_runs".to_string()),
                    skip_if_error: None,
                },
                ScheduleAction {
                    tool: "get_time_status".to_string(),
                    args: serde_json::Value::Null,
                    at: Some(1.5),
                    at_frame: None,
                    label: Some("also_never".to_string()),
                    skip_if_error: None,
                },
            ],
            results: vec![ActionResult {
                index: 0,
                label: Some("blocked".to_string()),
                tool: "get_time_status".to_string(),
                at: 0.5,
                fired_at_game_time: 0.0,
                status: "error".to_string(),
                result: None,
                error: Some("Schedule self-deadlocked".to_string()),
            }],
            current_index: 1, // simulate the deadlock guard already consumed action 0
            state: ScheduleState::WaitingForTime,
            t0_game_time: 0.0,
            frame_counter: 0,
            stop_on_error: false,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: None,
            deferred_rx: None,
        };
        let from = schedule.current_index;
        abort_remaining(&mut schedule, from);
        assert_eq!(schedule.results.len(), 3);
        assert_eq!(schedule.results[0].status, "error");
        assert_eq!(schedule.results[1].status, "aborted");
        assert_eq!(schedule.results[2].status, "aborted");
        assert_eq!(schedule.results[1].tool, "resume_time");
    }
}
