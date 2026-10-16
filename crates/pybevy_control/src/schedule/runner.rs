use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Instant,
};

use bevy::{
    ecs::world::World,
    prelude::Resource,
    time::{Time, Virtual},
};
use serde::Serialize;
use tokio::sync::oneshot;

use super::{
    registry::{ScheduleStatus, SharedScheduleState},
    request::{ScheduleAction, ScheduleRequest, resolve_at, resolve_at_frame, tool_to_operation},
};
use crate::{
    capture::readiness::RenderFrameReadiness,
    engine::{
        channel::ControlRequest, deferred::DeferredBatch, dispatch::dispatch,
        runtime::ControlRuntime,
    },
    protocol::{ControlError, OperationKind, OperationTag},
    scene::{entity::has_embedded_errors, transform_sync::propagate_transforms},
};

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
        .remove_non_send::<Box<dyn ControlRuntime>>()
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
    runtime: &mut dyn ControlRuntime,
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
        guard.completed_at = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::{Value, json};

    use super::*;
    #[cfg(feature = "pyo3-runtime")]
    use crate::pyo3::runtime::Pyo3ControlRuntime;
    use crate::{
        capture::{queue::PendingScreenshots, readiness::RenderFrameReadiness},
        engine::runtime::fake::FakeRuntime,
        schedule::request::ScheduleMode,
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

    /// A virtual-time `at` behind paused time aborts the schedule even
    /// without stop_on_error.
    #[cfg(feature = "pyo3-runtime")]
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
    fn scheduled_capture_inherits_the_latest_render_epoch() {
        let mut world = World::new();
        let readiness = RenderFrameReadiness::default();
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

    fn at_action(tool: &str, args: Value, at: f64) -> ScheduleAction {
        ScheduleAction {
            tool: tool.to_string(),
            args,
            at: Some(at),
            at_frame: None,
            label: None,
            skip_if_error: None,
        }
    }

    fn world_at_virtual_time(elapsed: f64, delta: f64) -> World {
        let mut time = Time::<Virtual>::default();
        time.advance_by(Duration::from_secs_f64(elapsed - delta));
        time.advance_by(Duration::from_secs_f64(delta));
        let mut world = World::new();
        world.insert_resource(time);
        world
    }

    fn statuses(results: &[ActionResult]) -> Vec<ActionStatus> {
        results.iter().map(|result| result.status).collect()
    }

    #[test]
    fn skip_if_error_skips_only_actions_behind_a_failed_label() {
        let mut world = world_at_virtual_time(0.0, 0.0);
        let mut failing = at_action("despawn_entity", json!({"entity": "Missing"}), 0.0);
        failing.label = Some("spawn".into());
        let mut dependent = at_action("get_time_status", json!({}), 0.0);
        dependent.skip_if_error = Some("spawn".into());
        let independent = at_action("get_time_status", json!({}), 0.0);
        let mut schedule = test_schedule(vec![failing, dependent, independent], false);

        process_single_schedule(&mut world, &mut schedule, &mut FakeRuntime);

        assert_eq!(schedule.state, ScheduleState::Done);
        assert_eq!(
            statuses(&schedule.results),
            [ActionStatus::Error, ActionStatus::Skipped, ActionStatus::Ok]
        );
        assert_eq!(
            schedule.results[1].error.as_deref(),
            Some("Skipped due to error in 'spawn'")
        );
    }

    #[test]
    fn cancelling_an_async_schedule_cancels_its_remaining_actions() {
        let mut world = world_at_virtual_time(0.0, 0.0);
        world.insert_non_send(Box::new(FakeRuntime) as Box<dyn ControlRuntime>);
        let shared = Arc::new(Mutex::new(SharedScheduleState::new("s-cancel", 2)));
        let request = ScheduleRequest {
            actions: vec![
                at_action("get_time_status", json!({}), 0.0),
                at_action("get_time_status", json!({}), 5.0),
            ],
            mode: ScheduleMode::Async,
            stop_on_error: false,
        };
        world.insert_resource(ActiveSchedules {
            schedules: vec![ActiveSchedule::new_async(
                "s-cancel".into(),
                request,
                0.0,
                shared.clone(),
            )],
            next_id: 1,
        });

        process_active_schedules(&mut world);
        assert_eq!(world.resource::<ActiveSchedules>().schedules.len(), 1);

        shared.lock().unwrap().cancelled = true;
        process_active_schedules(&mut world);

        assert!(world.resource::<ActiveSchedules>().schedules.is_empty());
        let state = shared.lock().unwrap();
        assert_eq!(state.status, ScheduleStatus::Completed);
        assert_eq!(
            statuses(&state.results),
            [ActionStatus::Ok, ActionStatus::Cancelled]
        );
        assert_eq!((state.executed_actions, state.cancelled_actions), (1, 1));
    }

    #[test]
    fn seek_time_moves_the_schedule_clock_with_virtual_time() {
        let mut world = world_at_virtual_time(0.0, 0.0);
        let mut schedule = test_schedule(
            vec![
                at_action("seek_time", json!({"seconds": 10.0, "pause": false}), 0.0),
                at_action("get_time_status", json!({}), 0.5),
            ],
            false,
        );

        process_single_schedule(&mut world, &mut schedule, &mut FakeRuntime);
        assert_eq!(statuses(&schedule.results), [ActionStatus::Ok]);
        assert_eq!(schedule.timing_origin_game_time, 10.0);

        world
            .resource_mut::<Time<Virtual>>()
            .advance_by(Duration::from_millis(500));
        process_single_schedule(&mut world, &mut schedule, &mut FakeRuntime);
        assert_eq!(
            statuses(&schedule.results),
            [ActionStatus::Ok, ActionStatus::Ok]
        );
        assert_eq!(schedule.state, ScheduleState::Done);
    }

    #[test]
    fn closed_deferred_channel_fails_the_held_action() {
        for stop_on_error in [false, true] {
            let mut world = world_at_virtual_time(0.0, 0.0);
            let mut schedule = test_schedule(
                vec![
                    at_action("capture_screenshot", json!({}), 0.0),
                    at_action("get_time_status", json!({}), 0.0),
                ],
                stop_on_error,
            );
            let (response_tx, response_rx) = oneshot::channel();
            drop(response_tx);
            schedule.deferred_rx = Some(response_rx);
            schedule.deferred_firing = Some(Firing::default());
            schedule.state = ScheduleState::WaitingForDeferred;

            process_single_schedule(&mut world, &mut schedule, &mut FakeRuntime);
            assert_eq!(
                schedule.results[0].error.as_deref(),
                Some("Deferred channel closed unexpectedly")
            );

            process_single_schedule(&mut world, &mut schedule, &mut FakeRuntime);
            let expected = if stop_on_error {
                [ActionStatus::Error, ActionStatus::Aborted]
            } else {
                [ActionStatus::Error, ActionStatus::Ok]
            };
            assert_eq!(statuses(&schedule.results), expected);
            assert_eq!(schedule.state, ScheduleState::Done);
        }
    }

    #[test]
    fn firing_is_late_only_when_the_action_was_due_in_an_earlier_frame() {
        let world = world_at_virtual_time(2.0, 0.5);
        let mut schedule = test_schedule(vec![], false);
        schedule.frame_counter = 5;

        let fire_at =
            |at: f64| Firing::dispatch(&schedule, &at_action("x", Value::Null, at), &world);
        assert_eq!(fire_at(1.0).late_by_seconds, Some(1.0));
        assert_eq!(fire_at(1.75).late_by_seconds, None);

        let fire_at_frame = |at_frame: i64| {
            let mut action = at_action("x", Value::Null, 0.0);
            action.at = None;
            action.at_frame = Some(at_frame);
            Firing::dispatch(&schedule, &action, &world).late_by_frames
        };
        assert_eq!(fire_at_frame(2), Some(3));
        assert_eq!(fire_at_frame(5), None);
    }
}
