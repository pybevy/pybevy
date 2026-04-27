use std::sync::{Arc, Mutex};

use bevy::{ecs::world::World, prelude::Resource};
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};

use crate::handlers::{
    self,
    reload::{PendingReloadAndCapture, PendingReloadAndCaptures, ReloadAndCaptureState},
    schedule::{
        ActiveSchedule, ActiveSchedules, SharedScheduleRegistryResource, SharedScheduleState,
    },
    screenshot::{ActiveTimeline, PendingTimelines, setup_debug_camera},
    turnaround::{ActiveTurnaround, PendingTurnarounds, compute_scene_bounds, compute_viewpoints},
};

/// Maximum requests processed per frame to prevent frame spikes
const MAX_REQUESTS_PER_FRAME: usize = 32;

/// Marker component for internal overlay UI that should always be hidden in screenshots.
/// Applied to hot reload overlay entities so the screenshot handler can find and hide them.
#[derive(bevy::prelude::Component)]
pub struct InternalOverlayUi;

/// Entity can be addressed by numeric ID or Name string
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum EntityRef {
    Id(u64),
    Name(String),
}

/// Scene inspection operations (read-only)
#[derive(Debug)]
pub enum SceneOp {
    ListEntities,
    GetEntity {
        entity: EntityRef,
    },
    ListResources,
    ListSystems,
    QueryEntities {
        with: Vec<String>,
        without: Vec<String>,
    },
    GetComponentSchema {
        name: String,
    },
    GetComponent {
        entity: EntityRef,
        component: String,
    },
    SceneSummary,
    GetBoundingBox {
        entity: EntityRef,
    },
    DebugRegistry,
}

/// Mutation operations (spawn, despawn, set, remove)
#[derive(Debug)]
pub enum MutateOp {
    SpawnEntity {
        components: serde_json::Value,
    },
    DespawnEntity {
        entity: EntityRef,
    },
    SetComponent {
        entity: EntityRef,
        component: String,
        fields: serde_json::Value,
    },
    RemoveComponent {
        entity: EntityRef,
        component: String,
    },
    InsertResource {
        resource_type: String,
        value: serde_json::Value,
    },
    RemoveResource {
        resource_type: String,
    },
    BatchMutate {
        operations: Vec<serde_json::Value>,
    },
}

/// Time control operations
#[derive(Debug)]
pub enum TimeOp {
    PauseTime,
    ResumeTime,
    SetTimeScale { scale: f32 },
    GetTimeStatus,
    SeekTime { seconds: f64, pause: bool },
}

/// Visual capture operations (deferred response)
#[derive(Debug)]
pub enum VisualOp {
    CaptureScreenshot {
        delay_frames: u32,
        max_width: Option<u32>,
        position: Option<[f32; 3]>,
        look_at: Option<[f32; 3]>,
        hide_ui: bool,
    },
    CaptureWithGizmos {
        delay_frames: u32,
        max_width: Option<u32>,
        position: Option<[f32; 3]>,
        look_at: Option<[f32; 3]>,
        hide_ui: bool,
    },
    CaptureTimeline {
        total_frames: u32,
        capture_count: u32,
        max_width: Option<u32>,
        columns: u32,
        position: Option<[f32; 3]>,
        look_at: Option<[f32; 3]>,
    },
    CaptureTurnaround {
        look_at: Option<[f32; 3]>,
        distance: Option<f32>,
        elevation: Option<f32>,
        view_count: Option<u32>,
        include_top: Option<bool>,
        columns: Option<u32>,
        max_width: Option<u32>,
        hide_ui: Option<bool>,
    },
    CaptureDepth {
        position: Option<[f32; 3]>,
        look_at: Option<[f32; 3]>,
        sample_points: Option<Vec<[u32; 2]>>,
        grid_density: Option<u32>,
        include_rgb: Option<bool>,
        delay_frames: Option<u32>,
        hide_ui: Option<bool>,
        max_width: Option<u32>,
    },
}

/// Reload operations (deferred response)
#[derive(Debug)]
pub enum ReloadOp {
    TriggerReload {
        mode: String,
        pause: bool,
        time_scale: Option<f32>,
    },
    ReloadAndCapture {
        mode: String,
        pause: bool,
        time_scale: Option<f32>,
        delay_frames: Option<u32>,
        max_width: Option<u32>,
        position: Option<[f32; 3]>,
        look_at: Option<[f32; 3]>,
        hide_ui: Option<bool>,
    },
    GetReloadStatus,
    GetLastError,
}

/// Spatial query operations
#[derive(Debug)]
pub enum SpatialOp {
    QuerySpatial {
        entity_a: EntityRef,
        entity_b: EntityRef,
    },
    QuerySpatialNeighborhood {
        entity: EntityRef,
        radius: f32,
        max_results: Option<usize>,
    },
    CheckOverlaps {
        entity: EntityRef,
        include_siblings: bool,
        max_float_gap: f32,
        ground_y: Option<f32>,
    },
    CheckAllOverlaps {
        min_penetration: Option<f32>,
        max_results: Option<usize>,
        max_float_gap: f32,
        ground_y: Option<f32>,
        include_siblings: bool,
    },
}

/// Other operations (execute, performance, assets, configs, schedule)
#[derive(Debug)]
pub enum OtherOp {
    ExecutePython {
        code: String,
    },
    GetPerformance,
    MutateAsset {
        entity: EntityRef,
        component: String,
        asset_type: String,
        fields: serde_json::Value,
    },
    GetConfig {
        key: String,
    },
    ListConfigs,
    SubmitSchedule {
        request: crate::handlers::schedule::ScheduleRequest,
    },
}

/// All control operations that require World access
#[derive(Debug)]
pub enum ControlOperation {
    Scene(SceneOp),
    Mutate(MutateOp),
    Time(TimeOp),
    Visual(VisualOp),
    Reload(ReloadOp),
    Spatial(SpatialOp),
    Other(OtherOp),
}

/// Error codes for control operations (JSON-RPC style)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ErrorCode {
    NotFound = -32001,
    NotSupported = -32601,
    InvalidParams = -32602,
    Internal = -32603,
}

impl ErrorCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Error type for control operations
#[derive(Debug)]
pub struct ControlError {
    pub code: ErrorCode,
    pub message: String,
}

impl ControlError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::NotFound,
            message: msg.into(),
        }
    }

    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidParams,
            message: msg.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Internal,
            message: msg.into(),
        }
    }

    pub fn not_supported(operation: &str) -> Self {
        Self {
            code: ErrorCode::NotSupported,
            message: format!(
                "Operation '{}' is not supported by the current Python runtime backend",
                operation
            ),
        }
    }
}

/// A request sent from the HTTP server thread to the Bevy exclusive system
pub struct ControlRequest {
    pub operation: ControlOperation,
    pub response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
}

/// Bevy resource holding the receive end of the control channel
#[derive(Resource)]
pub struct ControlReceiver {
    pub rx: mpsc::UnboundedReceiver<ControlRequest>,
}

/// Clonable sender for the control channel (stored in HTTP server state)
#[derive(Clone)]
pub struct ControlSender {
    pub tx: mpsc::UnboundedSender<ControlRequest>,
}

/// Bevy resource for pending screenshot responses (deferred until after render)
#[derive(Resource, Default)]
pub struct PendingScreenshots {
    pub pending: Vec<PendingScreenshot>,
}

pub struct PendingScreenshot {
    pub response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    pub frames_remaining: u32,
    pub with_gizmos: bool,
    pub max_width: Option<u32>,
    pub debug_camera: Option<DebugCameraRequest>,
    pub hide_ui: bool,
    /// Extra JSON fields to merge into the screenshot response.
    /// Used by `capture_depth` and `reload_and_capture` to avoid spawning
    /// a thread for the merge.
    pub extra_response: Option<serde_json::Value>,
}

/// Bevy resource for pending reload responses (deferred until reload completes)
#[derive(Resource, Default)]
pub struct PendingReloadResponses {
    pub pending: Vec<PendingReloadResponse>,
}

pub struct PendingReloadResponse {
    pub response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    pub frames_remaining: u32,
    pub mode: String,
    pub error_timestamp_before: f64,
}

#[derive(Debug, Clone)]
pub struct DebugCameraRequest {
    pub position: [f32; 3],
    pub look_at: [f32; 3],
}

/// SSE event broadcaster resource
#[derive(Resource, Clone)]
pub struct SseEventBroadcaster {
    pub tx: Arc<tokio::sync::broadcast::Sender<String>>,
}

impl Default for SseEventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl SseEventBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(256);
        Self { tx: Arc::new(tx) }
    }

    pub fn send(&self, event: &crate::protocol::SseEvent) {
        if let Ok(json) = serde_json::to_string(event) {
            // Ignore send errors (no subscribers)
            let _ = self.tx.send(json);
        }
    }
}

/// Tracks the last error timestamp we've already broadcast via SSE,
/// so we only push new errors to clients.
#[derive(Resource, Default)]
pub struct LastBroadcastedErrorTimestamp {
    pub timestamp_secs: f64,
}

/// Shared latest error state, readable from both Bevy system and Axum handler.
/// Updated by control_poll_system, read by HTTP handlers to piggyback on responses.
#[derive(Clone, Default)]
pub struct SharedLatestError {
    inner: Arc<Mutex<Option<ErrorSnapshot>>>,
}

#[derive(Clone)]
pub struct ErrorSnapshot {
    pub message: String,
    pub timestamp_secs: f64,
}

impl SharedLatestError {
    pub fn update(&self, message: String, timestamp_secs: f64) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some(ErrorSnapshot {
                message,
                timestamp_secs,
            });
        }
    }

    /// Get the latest error if it hasn't been reported yet.
    /// Marks it as reported so it's only included once.
    pub fn take_if_new(&self) -> Option<ErrorSnapshot> {
        if let Ok(mut guard) = self.inner.lock() {
            guard.take()
        } else {
            None
        }
    }
}

/// Bevy resource wrapper for SharedLatestError
#[derive(Resource, Clone)]
pub struct SharedLatestErrorResource(pub SharedLatestError);

/// Create the mpsc channel pair
pub fn create_channel() -> (ControlSender, ControlReceiver) {
    let (tx, rx) = mpsc::unbounded_channel();
    (ControlSender { tx }, ControlReceiver { rx })
}

/// Handle deferred CaptureScreenshot or CaptureWithGizmos requests.
fn handle_deferred_screenshot(request: ControlRequest, deferred: &mut Vec<PendingScreenshot>) {
    match request.operation {
        ControlOperation::Visual(VisualOp::CaptureScreenshot {
            delay_frames,
            max_width,
            position,
            look_at,
            hide_ui,
        }) => {
            let debug_camera = position.map(|pos| DebugCameraRequest {
                position: pos,
                look_at: look_at.unwrap_or([0.0, 0.0, 0.0]),
            });
            deferred.push(PendingScreenshot {
                response_tx: request.response_tx,
                frames_remaining: delay_frames,
                with_gizmos: false,
                max_width,
                debug_camera,
                hide_ui,
                extra_response: None,
            });
        }
        ControlOperation::Visual(VisualOp::CaptureWithGizmos {
            delay_frames,
            max_width,
            position,
            look_at,
            hide_ui,
        }) => {
            let debug_camera = position.map(|pos| DebugCameraRequest {
                position: pos,
                look_at: look_at.unwrap_or([0.0, 0.0, 0.0]),
            });
            deferred.push(PendingScreenshot {
                response_tx: request.response_tx,
                frames_remaining: delay_frames,
                with_gizmos: true,
                max_width,
                debug_camera,
                hide_ui,
                extra_response: None,
            });
        }
        _ => unreachable!(),
    }
}

/// Handle deferred CaptureTimeline requests.
fn handle_deferred_timeline(request: ControlRequest, world: &mut World) {
    let ControlOperation::Visual(VisualOp::CaptureTimeline {
        total_frames,
        capture_count,
        max_width,
        columns,
        position,
        look_at,
    }) = request.operation
    else {
        unreachable!()
    };

    let mut schedule = crate::handlers::screenshot::compute_schedule(total_frames, capture_count);

    let debug_cleanup = position.map(|pos| {
        let debug_req = DebugCameraRequest {
            position: pos,
            look_at: look_at.unwrap_or([0.0, 0.0, 0.0]),
        };
        // Add extra frames for debug camera to render
        if let Some(first) = schedule.front_mut() {
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
            response_tx: Some(request.response_tx),
            max_width,
            columns,
            debug_cleanup,
            schedule,
            total_captures: capture_count,
            next_capture_index: 0,
            collected: Vec::new(),
        },
    );
}

/// Handle deferred TriggerReload requests.
fn handle_deferred_reload(request: ControlRequest, world: &mut World) {
    let ControlOperation::Reload(ReloadOp::TriggerReload {
        mode,
        pause,
        time_scale,
    }) = request.operation
    else {
        unreachable!()
    };

    // Execute the reload synchronously, then defer the response
    let mode_str = mode;
    let _ = handlers::reload::trigger_reload(world, mode_str.clone(), pause, time_scale);

    // Record current error timestamp to detect new errors after reload
    let error_ts = world
        .get_resource::<pybevy_core::LastSystemError>()
        .map(|e| e.timestamp_secs)
        .unwrap_or(0.0);

    let mut pending_reloads = world.get_resource_or_insert_with(PendingReloadResponses::default);
    pending_reloads.pending.push(PendingReloadResponse {
        response_tx: request.response_tx,
        frames_remaining: 5,
        mode: mode_str,
        error_timestamp_before: error_ts,
    });
}

/// Handle deferred ReloadAndCapture requests.
fn handle_deferred_reload_and_capture(request: ControlRequest, world: &mut World) {
    let ControlOperation::Reload(ReloadOp::ReloadAndCapture {
        mode,
        pause,
        time_scale,
        delay_frames,
        max_width,
        position,
        look_at,
        hide_ui,
    }) = request.operation
    else {
        unreachable!()
    };

    // Trigger the reload
    let mode_str = mode;
    let _ = handlers::reload::trigger_reload(world, mode_str.clone(), pause, time_scale);

    let error_ts = world
        .get_resource::<pybevy_core::LastSystemError>()
        .map(|e| e.timestamp_secs)
        .unwrap_or(0.0);

    let mut pending = world.get_resource_or_insert_with(PendingReloadAndCaptures::default);
    pending.pending.push(PendingReloadAndCapture {
        response_tx: request.response_tx,
        mode: mode_str,
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
}

/// Handle deferred CaptureTurnaround requests.
fn handle_deferred_turnaround(request: ControlRequest, world: &mut World) {
    let ControlOperation::Visual(VisualOp::CaptureTurnaround {
        look_at,
        distance,
        elevation,
        view_count,
        include_top,
        columns,
        max_width,
        hide_ui,
    }) = request.operation
    else {
        unreachable!()
    };

    let vc = view_count.unwrap_or(6);
    let elev = elevation.unwrap_or(25.0);
    let top = include_top.unwrap_or(true);

    // Auto-fit distance and look_at from scene bounds
    let (auto_look_at, auto_distance) = if distance.is_none() || look_at.is_none() {
        if let Some((scene_min, scene_max)) = compute_scene_bounds(world) {
            let center = (scene_min + scene_max) * 0.5;
            let extent = scene_max - scene_min;
            let diagonal = (extent.x * extent.x + extent.y * extent.y + extent.z * extent.z).sqrt();
            // Distance so diagonal subtends ~60° FOV
            let dist = diagonal / (30.0_f32.to_radians().tan() * 2.0);
            ([center.x, center.y, center.z], dist.max(2.0))
        } else {
            ([0.0, 0.0, 0.0], 10.0)
        }
    } else {
        ([0.0, 0.0, 0.0], 10.0)
    };

    let final_look_at = look_at.unwrap_or(auto_look_at);
    let final_distance = distance.unwrap_or(auto_distance);

    let viewpoints = compute_viewpoints(final_look_at, final_distance, elev, vc, top);

    let mut pending = world.get_resource_or_insert_with(PendingTurnarounds::default);
    pending.active.push(ActiveTurnaround {
        response_tx: Some(request.response_tx),
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
}

/// Handle deferred CaptureDepth requests.
fn handle_deferred_depth(
    request: ControlRequest,
    deferred: &mut Vec<PendingScreenshot>,
    world: &mut World,
) {
    let ControlOperation::Visual(VisualOp::CaptureDepth {
        position,
        look_at,
        sample_points,
        grid_density,
        include_rgb,
        delay_frames,
        hide_ui,
        max_width,
    }) = request.operation
    else {
        unreachable!()
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
        let depth = match depth_result {
            Ok(d) => d,
            Err(e) => {
                let _ = request.response_tx.send(Err(e));
                return;
            }
        };

        deferred.push(PendingScreenshot {
            response_tx: request.response_tx,
            frames_remaining: df,
            with_gizmos: false,
            max_width: mw,
            debug_camera: dc,
            hide_ui: hu,
            extra_response: Some(serde_json::json!({ "depth_samples": depth })),
        });
    } else {
        let result = depth_result.map(|depth| {
            serde_json::json!({
                "screenshot": null,
                "depth_samples": depth,
            })
        });
        let _ = request.response_tx.send(result);
    }
}

/// Handle SubmitSchedule requests (no GIL needed).
fn handle_submit_schedule(request: ControlRequest, world: &mut World) {
    let sched_req = match request.operation {
        ControlOperation::Other(OtherOp::SubmitSchedule { request: r }) => r,
        _ => unreachable!(),
    };

    let mut active = world.get_resource_or_insert_with(ActiveSchedules::default);
    let schedule_id = format!("schedule-{}", active.next_id);
    active.next_id += 1;

    let t0 = world
        .get_resource::<bevy::time::Time<bevy::time::Virtual>>()
        .map(|t| t.elapsed_secs_f64())
        .unwrap_or(0.0);

    if sched_req.mode == "async" {
        let shared = std::sync::Arc::new(std::sync::Mutex::new(SharedScheduleState::new(
            &schedule_id,
            sched_req.actions.len(),
        )));

        if let Some(registry_res) = world.get_resource::<SharedScheduleRegistryResource>() {
            registry_res.0.insert(schedule_id.clone(), shared.clone());
        }

        let _ = request.response_tx.send(Ok(serde_json::json!({
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
            request.response_tx,
        ));
    }
}

/// Exclusive Bevy system that drains the control request channel.
/// Runs in First schedule, before Python systems.
pub fn control_poll_system(world: &mut World) {
    // Take the receiver out of the resource temporarily
    let Some(mut receiver) = world.remove_resource::<ControlReceiver>() else {
        return;
    };

    let mut processed = 0;
    let mut deferred_screenshots: Vec<PendingScreenshot> = Vec::new();

    // Two-phase processing: drain all requests first (no GIL needed), then
    // execute sync handlers in one batched Python::attach. This prevents both
    // rapid GIL acquire/release cycling (which corrupts GIL state after
    // py.detach()) AND holding the GIL for deferred ops that don't need it
    // (which increases contention with the file watcher thread and can
    // contribute to deadlocks during hot reload).

    // Phase 1: Drain channel and classify requests (no GIL needed)
    let mut sync_requests: Vec<ControlRequest> = Vec::new();

    while processed < MAX_REQUESTS_PER_FRAME {
        match receiver.rx.try_recv() {
            Ok(request) => {
                processed += 1;

                // Handle SubmitSchedule (no GIL needed — just stores in World resource)
                if matches!(
                    &request.operation,
                    ControlOperation::Other(OtherOp::SubmitSchedule { .. })
                ) {
                    handle_submit_schedule(request, world);
                    continue;
                }

                // Classify: deferred ops go directly to their queues,
                // sync ops are collected for batched GIL processing
                match &request.operation {
                    ControlOperation::Visual(VisualOp::CaptureScreenshot { .. })
                    | ControlOperation::Visual(VisualOp::CaptureWithGizmos { .. }) => {
                        handle_deferred_screenshot(request, &mut deferred_screenshots);
                    }
                    ControlOperation::Visual(VisualOp::CaptureTimeline { .. }) => {
                        handle_deferred_timeline(request, world);
                    }
                    ControlOperation::Reload(ReloadOp::TriggerReload { .. }) => {
                        handle_deferred_reload(request, world);
                    }
                    ControlOperation::Reload(ReloadOp::ReloadAndCapture { .. }) => {
                        handle_deferred_reload_and_capture(request, world);
                    }
                    ControlOperation::Visual(VisualOp::CaptureTurnaround { .. }) => {
                        handle_deferred_turnaround(request, world);
                    }
                    ControlOperation::Visual(VisualOp::CaptureDepth { .. }) => {
                        handle_deferred_depth(request, &mut deferred_screenshots, world);
                    }
                    // Sync operation — collect for batched GIL processing
                    _ => {
                        sync_requests.push(request);
                    }
                }
            }
            Err(mpsc::error::TryRecvError::Empty) => break,
            Err(mpsc::error::TryRecvError::Disconnected) => break,
        }
    }

    // Phase 2: Dispatch sync requests through the runtime.
    // The runtime impl batches GIL acquisition (PyO3) or enters interpreter
    // scope as appropriate for the backend.
    if !sync_requests.is_empty() {
        let mut runtime = world
            .remove_non_send_resource::<Box<dyn crate::runtime::ControlRuntime>>()
            .expect("ControlRuntime resource missing");

        runtime.dispatch_batch(world, sync_requests);

        world.insert_non_send_resource(runtime);
    }

    // Store deferred screenshots
    if !deferred_screenshots.is_empty() {
        let mut pending = world.get_resource_or_insert_with(PendingScreenshots::default);
        pending.pending.extend(deferred_screenshots);
    }

    // Put the receiver back
    world.insert_resource(receiver);

    // Broadcast new system errors via SSE + update shared state for HTTP piggyback
    if let Some(last_error) = world.get_resource::<pybevy_core::LastSystemError>()
        && let Some(ref msg) = last_error.error
    {
        let error_ts = last_error.timestamp_secs;
        let msg = msg.clone();
        let traceback = last_error.traceback.clone();

        let mut tracker = world.get_resource_or_insert_with(LastBroadcastedErrorTimestamp::default);

        if error_ts > tracker.timestamp_secs {
            tracker.timestamp_secs = error_ts;

            // SSE broadcast (for clients that subscribe to /api/v1/sse)
            if let Some(broadcaster) = world.get_resource::<SseEventBroadcaster>() {
                broadcaster.send(&crate::protocol::SseEvent::Error {
                    message: msg.clone(),
                    traceback,
                });
            }

            // Shared state update (for HTTP piggyback on next tool response)
            if let Some(shared) = world.get_resource::<SharedLatestErrorResource>() {
                shared.0.update(msg, error_ts);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use bevy::{ecs::entity::Entity, prelude::Transform};
    use pyo3::Python;

    use super::*;
    use crate::{protocol::SseEvent, runtime::ControlRuntime, runtime_pyo3::Pyo3ControlRuntime};

    #[test]
    fn entity_ref_deserialize_id() {
        let json = "42";
        let entity_ref: EntityRef = serde_json::from_str(json).unwrap();
        assert!(matches!(entity_ref, EntityRef::Id(42)));
    }

    #[test]
    fn entity_ref_deserialize_name() {
        let json = r#""MyEntity""#;
        let entity_ref: EntityRef = serde_json::from_str(json).unwrap();
        assert!(matches!(entity_ref, EntityRef::Name(ref s) if s == "MyEntity"));
    }

    #[test]
    fn control_error_not_found() {
        let err = ControlError::not_found("missing");
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.message, "missing");
    }

    #[test]
    fn control_error_invalid_params() {
        let err = ControlError::invalid_params("bad input");
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert_eq!(err.message, "bad input");
    }

    #[test]
    fn control_error_internal() {
        let err = ControlError::internal("crash");
        assert_eq!(err.code, ErrorCode::Internal);
        assert_eq!(err.message, "crash");
    }

    #[test]
    fn shared_latest_error_update_and_take() {
        let shared = SharedLatestError::default();

        // Initially empty
        assert!(shared.take_if_new().is_none());

        // Update with an error
        shared.update("test error".into(), 1.0);

        // First take returns the error
        let snapshot = shared.take_if_new().unwrap();
        assert_eq!(snapshot.message, "test error");
        assert_eq!(snapshot.timestamp_secs, 1.0);

        // Second take returns None (already consumed)
        assert!(shared.take_if_new().is_none());
    }

    #[test]
    fn shared_latest_error_update_overwrites() {
        let shared = SharedLatestError::default();
        shared.update("first".into(), 1.0);
        shared.update("second".into(), 2.0);

        let snapshot = shared.take_if_new().unwrap();
        assert_eq!(snapshot.message, "second");
        assert_eq!(snapshot.timestamp_secs, 2.0);
    }

    #[test]
    fn create_channel_works() {
        let (sender, mut receiver) = create_channel();
        let (tx, _rx) = tokio::sync::oneshot::channel();

        sender
            .tx
            .send(ControlRequest {
                operation: ControlOperation::Scene(SceneOp::ListEntities),
                response_tx: tx,
            })
            .unwrap();

        let req = receiver.rx.try_recv().unwrap();
        assert!(matches!(
            req.operation,
            ControlOperation::Scene(SceneOp::ListEntities)
        ));
    }

    #[test]
    fn sse_event_broadcaster_new() {
        let broadcaster = SseEventBroadcaster::new();
        // Should be able to subscribe
        let _rx = broadcaster.tx.subscribe();
    }

    #[test]
    fn sse_event_broadcaster_send_no_subscribers() {
        let broadcaster = SseEventBroadcaster::new();
        // Should not panic even with no subscribers
        broadcaster.send(&SseEvent::ReloadStarted {
            mode: "full".into(),
            generation: 0,
        });
    }

    #[test]
    fn sse_event_broadcaster_send_with_subscriber() {
        let broadcaster = SseEventBroadcaster::new();
        let mut rx = broadcaster.tx.subscribe();
        broadcaster.send(&SseEvent::ReloadStarted {
            mode: "full".into(),
            generation: 0,
        });
        let msg = rx.try_recv().unwrap();
        assert!(msg.contains("reload_started"));
    }

    #[test]
    fn control_error_debug_format() {
        let err = ControlError::not_found("test");
        let debug = format!("{:?}", err);
        assert!(debug.contains("NotFound"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn entity_ref_debug_format() {
        let id_ref = EntityRef::Id(42);
        let name_ref = EntityRef::Name("Test".into());
        assert!(format!("{:?}", id_ref).contains("42"));
        assert!(format!("{:?}", name_ref).contains("Test"));
    }

    #[test]
    fn shared_latest_error_default_is_empty() {
        let shared = SharedLatestError::default();
        assert!(shared.take_if_new().is_none());
    }

    #[test]
    fn shared_latest_error_clone_shares_state() {
        let shared = SharedLatestError::default();
        let cloned = shared.clone();
        shared.update("error".into(), 1.0);
        // The clone should see the same update
        let snapshot = cloned.take_if_new().unwrap();
        assert_eq!(snapshot.message, "error");
    }

    #[test]
    fn max_requests_per_frame_is_reasonable() {
        assert!(MAX_REQUESTS_PER_FRAME > 0);
        assert!(MAX_REQUESTS_PER_FRAME <= 1000);
    }

    /// Regression test: multiple get_component requests processed in a single
    /// control_poll_system frame must not crash.
    ///
    /// Without the Python::attach() wrapper around the dispatch loop, rapid
    /// GIL acquire/release cycling after py.detach() can corrupt GIL state
    /// and cause a silent segfault (the original bug).
    #[test]
    fn parallel_get_component_does_not_crash() {
        // Force linker to include pybevy_transform (its inventory entries register Transform bridge)
        extern crate pybevy_transform;

        static INIT: Once = Once::new();
        INIT.call_once(|| {
            Python::initialize();
            collect_all();
        });

        Python::attach(|py| {
            // Release the GIL to simulate the production environment where
            // app.run() calls py.detach() before entering the Bevy main loop.
            py.detach(|| {
                let mut world = World::new();
                let (sender, receiver) = create_channel();
                world.insert_resource(receiver);

                // Insert the runtime resource
                world.insert_non_send_resource(
                    Box::new(Pyo3ControlRuntime) as Box<dyn ControlRuntime>
                );

                // Spawn 6 entities with Transform
                let entities: Vec<Entity> = (0..6)
                    .map(|i| world.spawn(Transform::from_xyz(i as f32, 0.0, 0.0)).id())
                    .collect();

                // Queue 6 get_component requests (all in one batch, like parallel HTTP)
                let mut response_rxs = Vec::new();
                for entity in &entities {
                    let (tx, rx) = oneshot::channel();
                    sender
                        .tx
                        .send(ControlRequest {
                            operation: ControlOperation::Scene(SceneOp::GetComponent {
                                entity: EntityRef::Id(entity.to_bits()),
                                component: "Transform".into(),
                            }),
                            response_tx: tx,
                        })
                        .unwrap();
                    response_rxs.push(rx);
                }

                // Process all 6 in a single control_poll_system call
                control_poll_system(&mut world);

                // Verify all 6 responses arrived successfully
                for (i, rx) in response_rxs.into_iter().enumerate() {
                    let result = rx
                        .blocking_recv()
                        .unwrap_or_else(|_| panic!("Response {i} channel closed"));
                    let value =
                        result.unwrap_or_else(|e| panic!("Response {i} error: {}", e.message));
                    assert_eq!(
                        value["component"], "Transform",
                        "Response {i}: expected Transform component"
                    );
                    assert!(
                        value["fields"].is_object(),
                        "Response {i}: expected fields object"
                    );
                }
            });
        });
    }
}
