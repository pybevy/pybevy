use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::world::World,
    prelude::Resource,
    time::{Time, Virtual},
};
use pybevy_core::public_error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::{
    capture::screenshot::RenderFrameReadiness,
    engine::{channel::ControlRequest, deferred::DeferredBatch, dispatch::dispatch},
    protocol::{ControlError, ControlOperation, OperationKind, OperationTag, json_object_schema},
    pyo3::components::has_embedded_errors,
    scene::transform_sync::propagate_transforms,
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

/// Outcome of one scheduled action, serialized as its lowercase name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    #[default]
    Ok,
    /// The action ran but reported embedded per-item errors.
    Partial,
    Error,
    Skipped,
    Aborted,
    Cancelled,
}

impl ActionStatus {
    fn failed(self) -> bool {
        matches!(self, Self::Partial | Self::Error)
    }
}

/// Status and error message for a dispatched action's successful response.
fn classify(value: &serde_json::Value) -> (ActionStatus, Option<String>) {
    let run_code_failed = value
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .is_some_and(|success| !success);
    let error = run_code_failed
        .then(|| value.get("error").and_then(serde_json::Value::as_str))
        .flatten()
        .map(str::to_owned);
    let status = if has_embedded_errors(value) {
        ActionStatus::Partial
    } else if run_code_failed {
        ActionStatus::Error
    } else {
        ActionStatus::Ok
    };
    (status, error)
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ActionResult {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub tool: String,
    pub at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_frame: Option<u64>,
    /// Virtual time when the action was dispatched.
    pub fired_at_game_time: f64,
    /// Schedule frame when the action was dispatched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fired_at_frame: Option<u64>,
    /// Frames past `at_frame` when an earlier action held the schedule back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub late_by_frames: Option<u64>,
    /// Virtual seconds past `at` when an earlier action held the schedule back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub late_by_seconds: Option<f64>,
    /// Virtual time when a deferred action's result arrived.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at_game_time: Option<f64>,
    pub status: ActionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// When an action was dispatched, and how late relative to its offset.
#[derive(Debug, Clone, Copy, Default)]
struct Firing {
    game_time: f64,
    frame: u64,
    late_by_frames: Option<u64>,
    late_by_seconds: Option<f64>,
}

impl Firing {
    /// An action is late when it was already due in an earlier frame, which only
    /// happens while a deferred action holds the schedule.
    fn dispatch(schedule: &ActiveSchedule, action: &ScheduleAction, world: &World) -> Self {
        let time = world.get_resource::<Time<Virtual>>();
        let game_time = time.map_or(0.0, Time::elapsed_secs_f64);
        let previous_frame_time = game_time - time.map_or(0.0, Time::delta_secs_f64);
        let frame = schedule.frame_counter;
        match resolve_at_frame(action) {
            Some(at_frame) => Self {
                game_time,
                frame,
                late_by_frames: frame.checked_sub(at_frame).filter(|late| *late > 0),
                late_by_seconds: None,
            },
            None => {
                let target = schedule.timing_origin_game_time + resolve_at(action);
                Self {
                    game_time,
                    frame,
                    late_by_frames: None,
                    late_by_seconds: (previous_frame_time >= target && game_time > target)
                        .then_some(game_time - target),
                }
            }
        }
    }
}

impl ActionResult {
    /// Build a result mirroring an action's timing fields.
    fn new(
        action: &ScheduleAction,
        index: usize,
        fired: Option<Firing>,
        status: ActionStatus,
    ) -> Self {
        Self {
            index,
            label: action.label.clone(),
            tool: action.tool.clone(),
            at: action.at.unwrap_or(0.0),
            at_frame: resolve_at_frame(action),
            fired_at_game_time: fired.map_or(0.0, |fired| fired.game_time),
            fired_at_frame: fired.map(|fired| fired.frame),
            late_by_frames: fired.and_then(|fired| fired.late_by_frames),
            late_by_seconds: fired.and_then(|fired| fired.late_by_seconds),
            status,
            ..Self::default()
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

    fn with_optional_error(mut self, error: Option<String>) -> Self {
        self.error = error;
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
    start_game_time: f64,
    timing_origin_game_time: f64,
    frame_counter: u64,
    stop_on_error: bool,
    errored_labels: HashSet<String>,
    sync_response_tx: Option<oneshot::Sender<Result<serde_json::Value, ControlError>>>,
    async_shared: Option<Arc<Mutex<SharedScheduleState>>>,
    deferred_rx: Option<oneshot::Receiver<Result<serde_json::Value, ControlError>>>,
    deferred_firing: Option<Firing>,
}

#[derive(Resource, Default)]
pub struct ActiveSchedules {
    pub schedules: Vec<ActiveSchedule>,
    pub next_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleStatus {
    Running,
    Completed,
}

#[derive(Debug, Clone, Serialize)]
pub struct SharedScheduleState {
    pub schedule_id: String,
    pub status: ScheduleStatus,
    pub total_actions: usize,
    /// Number of actions with a terminal result, including actions that did not run.
    pub completed_actions: usize,
    /// Number of actions that were dispatched, including partial and error results.
    pub executed_actions: usize,
    pub skipped_actions: usize,
    pub aborted_actions: usize,
    pub cancelled_actions: usize,
    pub results: Vec<ActionResult>,
    pub cancelled: bool,
    #[serde(skip)]
    pub completed_at: Option<std::time::Instant>,
}

impl SharedScheduleState {
    pub fn new(schedule_id: &str, total: usize) -> Self {
        Self {
            schedule_id: schedule_id.to_string(),
            status: ScheduleStatus::Running,
            total_actions: total,
            completed_actions: 0,
            executed_actions: 0,
            skipped_actions: 0,
            aborted_actions: 0,
            cancelled_actions: 0,
            results: Vec::new(),
            cancelled: false,
            completed_at: None,
        }
    }
}

/// How long a completed schedule stays pollable before eviction.
pub const COMPLETED_SCHEDULE_RETENTION: std::time::Duration = std::time::Duration::from_secs(300);
/// Completed schedules kept regardless of age; oldest beyond this are evicted.
pub const MAX_COMPLETED_SCHEDULES: usize = 32;

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
            Self::evict_completed(&mut guard);
            guard.insert(id, state);
        }
    }

    fn evict_completed(
        entries: &mut std::collections::HashMap<String, Arc<Mutex<SharedScheduleState>>>,
    ) {
        let now = std::time::Instant::now();
        let mut completed: Vec<(String, std::time::Instant)> = entries
            .iter()
            .filter_map(|(id, state)| {
                let completed_at = state.lock().ok()?.completed_at?;
                Some((id.clone(), completed_at))
            })
            .collect();

        completed.retain(|(id, completed_at)| {
            if now.duration_since(*completed_at) > COMPLETED_SCHEDULE_RETENTION {
                entries.remove(id);
                false
            } else {
                true
            }
        });

        if completed.len() > MAX_COMPLETED_SCHEDULES {
            completed.sort_by_key(|(_, completed_at)| *completed_at);
            for (id, _) in &completed[..completed.len() - MAX_COMPLETED_SCHEDULES] {
                entries.remove(id);
            }
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
    action.at_frame.and_then(|frame| u64::try_from(frame).ok())
}

/// Returns true when virtual time cannot advance - either it is paused, or its
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
        start_game_time: f64,
        response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    ) -> Self {
        Self {
            schedule_id,
            stop_on_error: request.stop_on_error,
            actions: request.actions,
            results: Vec::new(),
            current_index: 0,
            state: ScheduleState::WaitingForTime,
            start_game_time,
            timing_origin_game_time: start_game_time,
            frame_counter: 0,
            errored_labels: HashSet::new(),
            sync_response_tx: Some(response_tx),
            async_shared: None,
            deferred_rx: None,
            deferred_firing: None,
        }
    }

    pub fn new_async(
        schedule_id: String,
        request: ScheduleRequest,
        start_game_time: f64,
        shared: Arc<Mutex<SharedScheduleState>>,
    ) -> Self {
        Self {
            schedule_id,
            stop_on_error: request.stop_on_error,
            actions: request.actions,
            results: Vec::new(),
            current_index: 0,
            state: ScheduleState::WaitingForTime,
            start_game_time,
            timing_origin_game_time: start_game_time,
            frame_counter: 0,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: Some(shared),
            deferred_rx: None,
            deferred_firing: None,
        }
    }

    /// Stores a result; a failed result marks its action's label as errored.
    fn record(&mut self, result: ActionResult) {
        if result.status.failed()
            && let Some(label) = &self.actions[result.index].label
        {
            self.errored_labels.insert(label.clone());
        }
        self.results.push(result);
    }

    /// Records the current action's outcome. Returns true when `stop_on_error`
    /// ended the schedule, after aborting every later action.
    fn record_outcome(
        &mut self,
        outcome: Result<serde_json::Value, String>,
        fired: Option<Firing>,
        completed_at: Option<f64>,
    ) -> bool {
        let action = &self.actions[self.current_index];
        let mut result = match outcome {
            Ok(value) => {
                let (status, error) = classify(&value);
                ActionResult::new(action, self.current_index, fired, status)
                    .with_result(value)
                    .with_optional_error(error)
            }
            Err(error) => ActionResult::new(action, self.current_index, fired, ActionStatus::Error)
                .with_error(error),
        };
        result.completed_at_game_time = completed_at;
        let failed = result.status.failed();
        self.record(result);
        if failed && self.stop_on_error {
            self.abort_from(self.current_index + 1);
            return true;
        }
        false
    }

    /// Aborts every action from `index` on and ends the schedule.
    fn abort_from(&mut self, index: usize) {
        for idx in index..self.actions.len() {
            let result = ActionResult::new(&self.actions[idx], idx, None, ActionStatus::Aborted)
                .with_error("Aborted due to stop_on_error");
            self.results.push(result);
        }
        self.state = ScheduleState::Done;
    }
}

fn virtual_elapsed(world: &World) -> f64 {
    world
        .get_resource::<Time<Virtual>>()
        .map_or(0.0, Time::elapsed_secs_f64)
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
        .remove_non_send::<Box<dyn crate::engine::runtime::ControlRuntime>>()
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
                        ActionResult::new(
                            action,
                            schedule.current_index,
                            None,
                            ActionStatus::Cancelled,
                        )
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

    world.insert_non_send(runtime);

    world.insert_resource(schedules);
}

fn process_single_schedule(
    world: &mut World,
    schedule: &mut ActiveSchedule,
    runtime: &mut dyn crate::engine::runtime::ControlRuntime,
) {
    loop {
        match schedule.state {
            ScheduleState::Done => return,

            ScheduleState::WaitingForDeferred => {
                let rx = schedule.deferred_rx.as_mut().unwrap();
                match rx.try_recv() {
                    Ok(result) => {
                        let completed_at = virtual_elapsed(world);
                        let fired = schedule.deferred_firing.take();
                        let outcome = result.map_err(|error| error.message);
                        if schedule.record_outcome(outcome, fired, Some(completed_at)) {
                            return;
                        }
                        schedule.deferred_rx = None;
                        schedule.current_index += 1;
                        schedule.state = ScheduleState::WaitingForTime;
                        update_async_progress(schedule);
                        continue;
                    }
                    Err(oneshot::error::TryRecvError::Empty) => return,
                    Err(oneshot::error::TryRecvError::Closed) => {
                        let fired = schedule.deferred_firing.take();
                        let outcome = Err("Deferred channel closed unexpectedly".to_string());
                        let stopped = schedule.record_outcome(outcome, fired, None);
                        schedule.deferred_rx = None;
                        schedule.current_index += 1;
                        if !stopped {
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
                let game_time = virtual_elapsed(world);

                let uses_frame_offset = resolve_at_frame(action).is_some();
                let ready = if let Some(frame_offset) = resolve_at_frame(action) {
                    schedule.frame_counter >= frame_offset
                } else {
                    let target_time = schedule.timing_origin_game_time + resolve_at(action);
                    game_time >= target_time
                };

                if !ready {
                    // An `at` gated on paused or zero-scaled virtual time can never
                    // fire, including any later resume_time behind it, so abort now
                    // rather than waiting for the engine's 120 s timeout.
                    if !uses_frame_offset && virtual_time_is_stalled(world) {
                        let error_msg = format!(
                            "Schedule self-deadlocked: virtual time is paused (or set to 0x \
                             scale), so action[{}] tool='{}' at={}s can never fire - virtual \
                             seconds do not advance while paused. Use 'at_frame' for actions \
                             that should run after pause_time/set_time_scale(0), or call \
                             resume_time from outside the schedule.",
                            schedule.current_index,
                            action.tool,
                            resolve_at(action),
                        );
                        let fired = Firing::dispatch(schedule, action, world);
                        let result = ActionResult::new(
                            action,
                            schedule.current_index,
                            Some(fired),
                            ActionStatus::Error,
                        )
                        .with_error(error_msg);
                        schedule.record(result);
                        schedule.current_index += 1;
                        schedule.abort_from(schedule.current_index);
                        update_async_progress(schedule);
                        return;
                    }
                    return;
                }

                let fired = Firing::dispatch(schedule, action, world);

                if let Some(ref skip_label) = action.skip_if_error
                    && schedule.errored_labels.contains(skip_label)
                {
                    let result = ActionResult::new(
                        action,
                        schedule.current_index,
                        Some(fired),
                        ActionStatus::Skipped,
                    )
                    .with_error(format!("Skipped due to error in '{}'", skip_label));
                    schedule.record(result);
                    schedule.current_index += 1;
                    update_async_progress(schedule);
                    continue;
                }

                let tool_name = action.tool.clone();
                let tool_args = action.args.clone();
                let action_at = resolve_at(action);
                let spec = OperationTag::from_name(&tool_name).map(OperationTag::spec);
                let is_transform_mutation = spec.is_some_and(|spec| spec.mutates_transforms);
                let is_render_mutation = spec.is_some_and(|spec| spec.mutates_render);

                if spec.is_some_and(|spec| spec.kind == OperationKind::Deferred && spec.schedulable)
                {
                    match setup_deferred_action(world, &tool_name, &tool_args) {
                        Ok(rx) => {
                            schedule.deferred_rx = Some(rx);
                            schedule.deferred_firing = Some(fired);
                            schedule.state = ScheduleState::WaitingForDeferred;
                            return;
                        }
                        Err(error) => {
                            if schedule.record_outcome(Err(error), Some(fired), None) {
                                return;
                            }
                            schedule.current_index += 1;
                            update_async_progress(schedule);
                            continue;
                        }
                    }
                }

                let outcome = tool_to_operation(&tool_name, &tool_args).and_then(|operation| {
                    dispatch(world, operation, runtime).map_err(|error| error.message)
                });
                let dispatched = outcome.is_ok();
                if schedule.record_outcome(outcome, Some(fired), None) {
                    return;
                }
                if dispatched {
                    // Later spatial queries in the same frame need current GlobalTransforms.
                    if is_transform_mutation {
                        propagate_transforms(world);
                    }
                    if is_render_mutation
                        && let Some(readiness) = world.get_resource::<RenderFrameReadiness>()
                    {
                        readiness.request_frame();
                    }
                }
                schedule.current_index += 1;
                if dispatched && tool_name == "seek_time" {
                    schedule.timing_origin_game_time = virtual_elapsed(world) - action_at;
                }
                update_async_progress(schedule);
                continue; // Same-at actions fire in the same frame
            }
        }
    }
}
fn setup_deferred_action(
    world: &mut World,
    tool: &str,
    args: &serde_json::Value,
) -> Result<oneshot::Receiver<Result<serde_json::Value, ControlError>>, String> {
    let operation = tool_to_operation(tool, args)?;
    let (response_tx, rx) = oneshot::channel();
    let render_epoch = world
        .get_resource::<RenderFrameReadiness>()
        .map(RenderFrameReadiness::requested_epoch);
    let mut batch = DeferredBatch::new(render_epoch);
    let request = ControlRequest {
        operation,
        response_tx,
    };
    if batch.enqueue(world, request).is_some() {
        return Err(format!("tool '{tool}' is not deferrable"));
    }
    batch.commit(world);
    Ok(rx)
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ActionCounts {
    executed: usize,
    skipped: usize,
    aborted: usize,
    cancelled: usize,
}

fn action_counts(results: &[ActionResult]) -> ActionCounts {
    let mut counts = ActionCounts::default();
    for result in results {
        match result.status {
            ActionStatus::Skipped => counts.skipped += 1,
            ActionStatus::Aborted => counts.aborted += 1,
            ActionStatus::Cancelled => counts.cancelled += 1,
            ActionStatus::Ok | ActionStatus::Partial | ActionStatus::Error => counts.executed += 1,
        }
    }
    counts
}

fn update_shared_action_counts(state: &mut SharedScheduleState, counts: ActionCounts) {
    state.executed_actions = counts.executed;
    state.skipped_actions = counts.skipped;
    state.aborted_actions = counts.aborted;
    state.cancelled_actions = counts.cancelled;
}

fn update_async_progress(schedule: &ActiveSchedule) {
    let counts = action_counts(&schedule.results);
    if let Some(ref shared) = schedule.async_shared
        && let Ok(mut guard) = shared.lock()
    {
        guard.completed_actions = schedule.results.len();
        update_shared_action_counts(&mut guard, counts);
        guard.results = schedule.results.clone();
    }
}

fn finalize_schedule(schedule: ActiveSchedule) {
    let game_times: Vec<f64> = schedule
        .results
        .iter()
        .map(|r| r.completed_at_game_time.unwrap_or(r.fired_at_game_time))
        .collect();
    let end_time = game_times.iter().copied().fold(0.0_f64, f64::max);
    let counts = action_counts(&schedule.results);

    let response = serde_json::json!({
        "schedule_id": schedule.schedule_id,
        "status": "completed",
        "total_actions": schedule.actions.len(),
        "completed_actions": schedule.results.len(),
        "executed_actions": counts.executed,
        "skipped_actions": counts.skipped,
        "aborted_actions": counts.aborted,
        "cancelled_actions": counts.cancelled,
        "start_game_time": schedule.start_game_time,
        "end_game_time": end_time,
        "results": schedule.results,
    });

    if let Some(tx) = schedule.sync_response_tx {
        let _ = tx.send(Ok(response));
    }

    if let Some(ref shared) = schedule.async_shared
        && let Ok(mut guard) = shared.lock()
    {
        guard.status = ScheduleStatus::Completed;
        guard.completed_actions = schedule.results.len();
        update_shared_action_counts(&mut guard, counts);
        guard.results = schedule.results;
        guard.completed_at = Some(std::time::Instant::now());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        capture::queue::PendingScreenshots,
        protocol::{
            CaptureScreenshotParams, CaptureStatsParams, CompareFramesParams, EntityRef,
            GetComponentParams, QueryEntitiesParams, ReloadMode, ReloadParams, SeekTimeParams,
            SetComponentParams,
        },
        pyo3::runtime::Pyo3ControlRuntime,
    };

    fn test_schedule(actions: Vec<ScheduleAction>, stop_on_error: bool) -> ActiveSchedule {
        ActiveSchedule {
            schedule_id: "test".to_string(),
            actions,
            results: vec![],
            current_index: 0,
            state: ScheduleState::WaitingForTime,
            start_game_time: 0.0,
            timing_origin_game_time: 0.0,
            frame_counter: 0,
            stop_on_error,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: None,
            deferred_rx: None,
            deferred_firing: None,
        }
    }

    fn timed_action(tool: &str, at: f64, label: &str) -> ScheduleAction {
        ScheduleAction {
            tool: tool.to_string(),
            args: serde_json::Value::Null,
            at: Some(at),
            at_frame: None,
            label: Some(label.to_string()),
            skip_if_error: None,
        }
    }

    #[test]
    fn abort_from_aborts_later_actions_and_ends_the_schedule() {
        let mut schedule = test_schedule(
            vec![
                timed_action("pause_time", 0.0, "a"),
                timed_action("resume_time", 1.0, "b"),
            ],
            true,
        );
        schedule.abort_from(0);
        assert_eq!(schedule.state, ScheduleState::Done);
        assert_eq!(
            schedule
                .results
                .iter()
                .map(|result| result.status)
                .collect::<Vec<_>>(),
            [ActionStatus::Aborted, ActionStatus::Aborted]
        );
        assert_eq!(
            action_counts(&schedule.results),
            ActionCounts {
                aborted: 2,
                ..Default::default()
            }
        );
    }

    #[test]
    fn classify_reports_run_code_failures_and_embedded_errors() {
        let cases = [
            (serde_json::json!({"success": true}), ActionStatus::Ok, None),
            (
                serde_json::json!({"success": false, "error": "NameError: x"}),
                ActionStatus::Error,
                Some("NameError: x"),
            ),
            (
                serde_json::json!({"errors": ["bad field"]}),
                ActionStatus::Partial,
                None,
            ),
            (
                serde_json::json!({"success": false, "error": "boom", "errors": ["bad"]}),
                ActionStatus::Partial,
                Some("boom"),
            ),
        ];
        for (value, status, error) in cases {
            assert_eq!(
                classify(&value),
                (status, error.map(str::to_owned)),
                "{value}"
            );
        }
    }

    #[test]
    fn serialized_statuses_keep_their_names() {
        assert_eq!(
            serde_json::to_value([
                ActionStatus::Ok,
                ActionStatus::Partial,
                ActionStatus::Error,
                ActionStatus::Skipped,
                ActionStatus::Aborted,
                ActionStatus::Cancelled,
            ])
            .unwrap(),
            serde_json::json!(["ok", "partial", "error", "skipped", "aborted", "cancelled"])
        );
        assert_eq!(
            serde_json::to_value([ScheduleStatus::Running, ScheduleStatus::Completed]).unwrap(),
            serde_json::json!(["running", "completed"])
        );
    }

    #[test]
    fn failed_outcome_marks_its_label_and_stops_only_with_stop_on_error() {
        let failure = || Ok(serde_json::json!({"success": false, "error": "boom"}));
        let actions = || {
            vec![
                timed_action("run_code", 0.0, "failing_code"),
                timed_action("pause_time", 0.0, "later"),
            ]
        };

        let mut continuing = test_schedule(actions(), false);
        assert!(!continuing.record_outcome(failure(), None, Some(2.0)));
        assert_eq!(continuing.state, ScheduleState::WaitingForTime);
        assert!(continuing.errored_labels.contains("failing_code"));
        assert_eq!(continuing.results.len(), 1);
        assert_eq!(continuing.results[0].status, ActionStatus::Error);
        assert_eq!(continuing.results[0].error.as_deref(), Some("boom"));
        assert_eq!(continuing.results[0].completed_at_game_time, Some(2.0));

        let mut stopping = test_schedule(actions(), true);
        assert!(stopping.record_outcome(failure(), None, None));
        assert_eq!(stopping.state, ScheduleState::Done);
        assert_eq!(
            stopping
                .results
                .iter()
                .map(|result| (result.tool.as_str(), result.status))
                .collect::<Vec<_>>(),
            [
                ("run_code", ActionStatus::Error),
                ("pause_time", ActionStatus::Aborted)
            ]
        );
        assert_eq!(stopping.results[0].completed_at_game_time, None);
    }

    #[test]
    fn successful_outcome_does_not_mark_its_label() {
        let mut schedule = test_schedule(vec![timed_action("get_time_status", 0.0, "a")], true);
        assert!(!schedule.record_outcome(Ok(serde_json::json!({"paused": false})), None, None));
        assert!(schedule.errored_labels.is_empty());
        assert_eq!(schedule.results[0].status, ActionStatus::Ok);
        assert_eq!(schedule.results[0].error, None);
    }

    /// A virtual-time `at` behind paused time aborts the schedule even
    /// without stop_on_error.
    #[test]
    fn stalled_virtual_time_aborts_the_schedule() {
        let mut world = World::new();
        let mut time = Time::<Virtual>::default();
        time.pause();
        world.insert_resource(time);
        let mut schedule = test_schedule(
            vec![
                timed_action("get_time_status", 0.5, "blocked"),
                timed_action("resume_time", 1.0, "never_runs"),
                timed_action("get_time_status", 1.5, "also_never"),
            ],
            false,
        );

        process_single_schedule(&mut world, &mut schedule, &mut Pyo3ControlRuntime);

        assert_eq!(schedule.state, ScheduleState::Done);
        assert!(schedule.errored_labels.contains("blocked"));
        assert_eq!(
            schedule
                .results
                .iter()
                .map(|result| (result.tool.as_str(), result.status))
                .collect::<Vec<_>>(),
            [
                ("get_time_status", ActionStatus::Error),
                ("resume_time", ActionStatus::Aborted),
                ("get_time_status", ActionStatus::Aborted),
            ]
        );
        assert!(
            schedule.results[0]
                .error
                .as_deref()
                .unwrap()
                .contains("self-deadlocked")
        );
    }

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
    fn test_action_result_serialization() {
        let result = ActionResult {
            index: 0,
            label: Some("test".to_string()),
            tool: "pause_time".to_string(),
            at: 0.0,
            at_frame: None,
            fired_at_game_time: 1.5,
            status: ActionStatus::Ok,
            result: Some(serde_json::json!({"paused": true})),
            error: None,
            ..Default::default()
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
        // at_frame must surface in the response.
        let action = ScheduleAction {
            tool: "pause_time".to_string(),
            args: serde_json::Value::Null,
            at: None,
            at_frame: Some(30),
            label: None,
            skip_if_error: None,
        };
        let r = ActionResult::new(&action, 0, None, ActionStatus::Ok)
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
        assert_eq!(state.status, ScheduleStatus::Running);
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

    fn completed_state(
        id: &str,
        completed_at: std::time::Instant,
    ) -> Arc<Mutex<SharedScheduleState>> {
        let mut state = SharedScheduleState::new(id, 1);
        state.status = ScheduleStatus::Completed;
        state.completed_at = Some(completed_at);
        Arc::new(Mutex::new(state))
    }

    #[test]
    fn registry_evicts_expired_completed_schedules_on_insert() {
        let registry = SharedScheduleRegistry::default();
        let expired = std::time::Instant::now() - COMPLETED_SCHEDULE_RETENTION * 2;
        registry.insert("old".to_string(), completed_state("old", expired));
        registry.insert(
            "fresh".to_string(),
            completed_state("fresh", std::time::Instant::now()),
        );
        registry.insert(
            "running".to_string(),
            Arc::new(Mutex::new(SharedScheduleState::new("running", 1))),
        );

        assert!(
            registry.get("old").is_none(),
            "expired entry must be evicted"
        );
        assert!(registry.get("fresh").is_some());
        assert!(registry.get("running").is_some());
    }

    #[test]
    fn registry_caps_completed_schedules_evicting_oldest() {
        let registry = SharedScheduleRegistry::default();
        let base = std::time::Instant::now();
        for index in 0..=MAX_COMPLETED_SCHEDULES {
            let id = format!("s-{index}");
            registry.insert(
                id.clone(),
                completed_state(&id, base + std::time::Duration::from_millis(index as u64)),
            );
        }
        registry.insert(
            "trigger".to_string(),
            Arc::new(Mutex::new(SharedScheduleState::new("trigger", 1))),
        );

        assert!(
            registry.get("s-0").is_none(),
            "oldest completed entry beyond the cap must be evicted"
        );
        assert!(
            registry
                .get(&format!("s-{MAX_COMPLETED_SCHEDULES}"))
                .is_some()
        );
        assert!(registry.get("trigger").is_some());
    }

    #[test]
    fn registry_never_evicts_running_schedules() {
        let registry = SharedScheduleRegistry::default();
        for index in 0..MAX_COMPLETED_SCHEDULES * 2 {
            let id = format!("run-{index}");
            registry.insert(
                id,
                Arc::new(Mutex::new(SharedScheduleState::new("running", 1))),
            );
        }
        assert!(registry.get("run-0").is_some());
    }

    #[test]
    fn finalize_schedule_stamps_completed_at() {
        let shared = Arc::new(Mutex::new(SharedScheduleState::new("s-done", 0)));
        let schedule = ActiveSchedule {
            schedule_id: "s-done".to_string(),
            actions: vec![],
            results: vec![],
            current_index: 0,
            state: ScheduleState::Done,
            start_game_time: 0.0,
            timing_origin_game_time: 0.0,
            frame_counter: 0,
            stop_on_error: false,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: Some(shared.clone()),
            deferred_rx: None,
            deferred_firing: None,
        };
        finalize_schedule(schedule);
        assert!(shared.lock().unwrap().completed_at.is_some());
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

    /// Verifies that the registry and ActiveSchedule share the same Arc,
    /// so updates from the schedule state machine are visible via registry.get().
    /// The registry previously stored a separate copy.
    #[test]
    fn test_registry_shares_arc_with_schedule() {
        let registry = SharedScheduleRegistry::default();
        let shared = Arc::new(Mutex::new(SharedScheduleState::new("s-1", 2)));
        registry.insert("s-1".to_string(), shared.clone());

        // Simulate what finalize_schedule does
        {
            let mut guard = shared.lock().unwrap();
            guard.status = ScheduleStatus::Completed;
            guard.completed_actions = 2;
            guard.results.push(ActionResult {
                index: 0,
                label: Some("a".to_string()),
                tool: "pause_time".to_string(),
                at: 0.0,
                at_frame: None,
                fired_at_game_time: 1.0,
                status: ActionStatus::Ok,
                result: Some(serde_json::json!({"paused": true})),
                error: None,
                ..Default::default()
            });
        }

        // Registry should see the updated state via the shared Arc
        let state = registry.get("s-1").unwrap();
        assert_eq!(state.status, ScheduleStatus::Completed);
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
    fn scheduled_capture_inherits_the_latest_render_epoch() {
        let mut world = World::new();
        let readiness = crate::capture::screenshot::RenderFrameReadiness::default();
        let required_epoch = readiness.request_frame();
        world.insert_resource(readiness);

        let _rx = setup_deferred_action(
            &mut world,
            "capture_stats",
            &serde_json::json!({"delay_frames": 2}),
        )
        .unwrap();

        let pending = world.resource::<PendingScreenshots>();
        assert_eq!(pending.pending.len(), 1);
        assert_eq!(
            pending.pending[0].required_render_epoch,
            Some(required_epoch)
        );
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
                status: ActionStatus::Ok,
                result: Some(serde_json::json!({"paused": true})),
                error: None,
                ..Default::default()
            }],
            current_index: 1,
            state: ScheduleState::Done,
            start_game_time: 0.25,
            timing_origin_game_time: 50.0,
            frame_counter: 1,
            stop_on_error: false,
            errored_labels: HashSet::new(),
            sync_response_tx: Some(tx),
            async_shared: None,
            deferred_rx: None,
            deferred_firing: None,
        };
        finalize_schedule(schedule);
        let response = rx.blocking_recv().unwrap().unwrap();
        assert_eq!(response["schedule_id"], "s-1");
        assert_eq!(response["status"], "completed");
        assert_eq!(response["total_actions"], 1);
        assert_eq!(response["completed_actions"], 1);
        assert_eq!(response["executed_actions"], 1);
        assert_eq!(response["skipped_actions"], 0);
        assert_eq!(response["aborted_actions"], 0);
        assert_eq!(response["cancelled_actions"], 0);
        assert_eq!(response["start_game_time"], 0.25);
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
                status: ActionStatus::Ok,
                result: None,
                error: None,
                ..Default::default()
            }],
            current_index: 1,
            state: ScheduleState::Done,
            start_game_time: 0.0,
            timing_origin_game_time: 0.0,
            frame_counter: 1,
            stop_on_error: false,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: Some(shared.clone()),
            deferred_rx: None,
            deferred_firing: None,
        };
        finalize_schedule(schedule);
        let guard = shared.lock().unwrap();
        assert_eq!(guard.status, ScheduleStatus::Completed);
        assert_eq!(guard.completed_actions, 1);
        assert_eq!(guard.executed_actions, 1);
        assert_eq!(guard.skipped_actions, 0);
        assert_eq!(guard.aborted_actions, 0);
        assert_eq!(guard.cancelled_actions, 0);
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
                    status: ActionStatus::Ok,
                    result: None,
                    error: None,
                    ..Default::default()
                },
                ActionResult {
                    index: 1,
                    label: None,
                    tool: "b".to_string(),
                    at: 0.0,
                    at_frame: None,
                    fired_at_game_time: 0.0,
                    status: ActionStatus::Ok,
                    result: None,
                    error: None,
                    ..Default::default()
                },
            ],
            current_index: 2,
            state: ScheduleState::Done,
            start_game_time: 0.0,
            timing_origin_game_time: 0.0,
            frame_counter: 0,
            stop_on_error: false,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: Some(shared.clone()),
            deferred_rx: None,
            deferred_firing: None,
        };
        update_async_progress(&schedule);
        let guard = shared.lock().unwrap();
        assert_eq!(guard.completed_actions, 2);
        assert_eq!(guard.executed_actions, 2);
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
            start_game_time: 0.0,
            timing_origin_game_time: 0.0,
            frame_counter: 0,
            stop_on_error: false,
            errored_labels: HashSet::new(),
            sync_response_tx: None,
            async_shared: None,
            deferred_rx: None,
            deferred_firing: None,
        };
        update_async_progress(&schedule); // No-op, should not panic
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

    #[test]
    fn test_shared_schedule_state_new() {
        let state = SharedScheduleState::new("test-id", 5);
        assert_eq!(state.schedule_id, "test-id");
        assert_eq!(state.status, ScheduleStatus::Running);
        assert_eq!(state.total_actions, 5);
        assert_eq!(state.completed_actions, 0);
        assert_eq!(state.executed_actions, 0);
        assert_eq!(state.skipped_actions, 0);
        assert_eq!(state.aborted_actions, 0);
        assert_eq!(state.cancelled_actions, 0);
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
        assert_eq!(schedule.start_game_time, 10.0);
        assert_eq!(schedule.timing_origin_game_time, 10.0);
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
}
