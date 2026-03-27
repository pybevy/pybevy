use std::sync::{Arc, Mutex};

use bevy::{ecs::world::World, prelude::Resource};
use pyo3::Python;
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

/// All control operations that require World access
#[derive(Debug)]
pub enum ControlOperation {
    // Read-only scene
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
    GetPerformance,
    GetReloadStatus,
    GetLastError,

    // Write operations
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
    TriggerReload {
        mode: String,
        pause: bool,
        time_scale: Option<f32>,
    },
    ExecutePython {
        code: String,
    },

    // Visual (deferred response)
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

    // Debug
    DebugRegistry,

    // Time control
    PauseTime,
    ResumeTime,
    SetTimeScale {
        scale: f32,
    },
    GetTimeStatus,
    SeekTime {
        seconds: f64,
        pause: bool,
    },

    // Asset mutation
    MutateAsset {
        entity: EntityRef,
        component: String,
        asset_type: String,
        fields: serde_json::Value,
    },

    // Bounding box
    GetBoundingBox {
        entity: EntityRef,
    },

    // Scene summary
    SceneSummary,

    // Spatial queries
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

    // Reload + capture combo (deferred)
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

    // Turnaround capture (deferred)
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

    // Depth capture (deferred)
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

    // Batch mutations
    BatchMutate {
        operations: Vec<serde_json::Value>,
    },

    // Custom tools
    CallCustomTool {
        name: String,
        arguments: serde_json::Value,
    },

    // Plugin configs
    GetConfig {
        key: String,
    },
    ListConfigs,

    // Schedule (batched timed actions)
    SubmitSchedule {
        request: crate::handlers::schedule::ScheduleRequest,
    },
}

/// Error type for control operations
#[derive(Debug)]
pub struct ControlError {
    pub code: i32,
    pub message: String,
}

impl ControlError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            code: -32001,
            message: msg.into(),
        }
    }

    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: msg.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: msg.into(),
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
                if matches!(&request.operation, ControlOperation::SubmitSchedule { .. }) {
                    let sched_req = match request.operation {
                        ControlOperation::SubmitSchedule { request: r } => r,
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
                        let shared = std::sync::Arc::new(std::sync::Mutex::new(
                            SharedScheduleState::new(&schedule_id, sched_req.actions.len()),
                        ));

                        if let Some(registry_res) =
                            world.get_resource::<SharedScheduleRegistryResource>()
                        {
                            registry_res.0.insert(schedule_id.clone(), shared.clone());
                        }

                        let _ = request.response_tx.send(Ok(serde_json::json!({
                            "schedule_id": schedule_id,
                            "status": "running",
                        })));

                        let mut active =
                            world.get_resource_or_insert_with(ActiveSchedules::default);
                        active.schedules.push(ActiveSchedule::new_async(
                            schedule_id,
                            sched_req,
                            t0,
                            shared,
                        ));
                    } else {
                        let mut active =
                            world.get_resource_or_insert_with(ActiveSchedules::default);
                        active.schedules.push(ActiveSchedule::new_sync(
                            schedule_id,
                            sched_req,
                            t0,
                            request.response_tx,
                        ));
                    }
                    continue;
                }

                // Classify: deferred ops go directly to their queues,
                // sync ops are collected for batched GIL processing
                match &request.operation {
                    ControlOperation::CaptureScreenshot {
                        delay_frames,
                        max_width,
                        position,
                        look_at,
                        hide_ui,
                    } => {
                        let debug_camera = position.map(|pos| DebugCameraRequest {
                            position: pos,
                            look_at: look_at.unwrap_or([0.0, 0.0, 0.0]),
                        });
                        deferred_screenshots.push(PendingScreenshot {
                            response_tx: request.response_tx,
                            frames_remaining: *delay_frames,
                            with_gizmos: false,
                            max_width: *max_width,
                            debug_camera,
                            hide_ui: *hide_ui,
                        });
                    }
                    ControlOperation::CaptureWithGizmos {
                        delay_frames,
                        max_width,
                        position,
                        look_at,
                        hide_ui,
                    } => {
                        let debug_camera = position.map(|pos| DebugCameraRequest {
                            position: pos,
                            look_at: look_at.unwrap_or([0.0, 0.0, 0.0]),
                        });
                        deferred_screenshots.push(PendingScreenshot {
                            response_tx: request.response_tx,
                            frames_remaining: *delay_frames,
                            with_gizmos: true,
                            max_width: *max_width,
                            debug_camera,
                            hide_ui: *hide_ui,
                        });
                    }
                    ControlOperation::CaptureTimeline {
                        total_frames,
                        capture_count,
                        max_width,
                        columns,
                        position,
                        look_at,
                    } => {
                        let mut schedule = crate::handlers::screenshot::compute_schedule(
                            *total_frames,
                            *capture_count,
                        );

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

                        let mut pending =
                            world.get_resource_or_insert_with(PendingTimelines::default);
                        let id = pending.next_id;
                        pending.next_id += 1;
                        pending.active.insert(
                            id,
                            ActiveTimeline {
                                response_tx: Some(request.response_tx),
                                max_width: *max_width,
                                columns: *columns,
                                debug_cleanup,
                                schedule,
                                total_captures: *capture_count,
                                next_capture_index: 0,
                                collected: Vec::new(),
                            },
                        );
                    }
                    ControlOperation::TriggerReload {
                        mode,
                        pause,
                        time_scale,
                    } => {
                        // Execute the reload synchronously, then defer the response
                        let mode_str = mode.clone();
                        let _ = handlers::reload::trigger_reload(
                            world,
                            mode_str.clone(),
                            *pause,
                            *time_scale,
                        );

                        // Record current error timestamp to detect new errors after reload
                        let error_ts = world
                            .get_resource::<pybevy_core::LastSystemError>()
                            .map(|e| e.timestamp_secs)
                            .unwrap_or(0.0);

                        let mut pending_reloads =
                            world.get_resource_or_insert_with(PendingReloadResponses::default);
                        pending_reloads.pending.push(PendingReloadResponse {
                            response_tx: request.response_tx,
                            frames_remaining: 5,
                            mode: mode_str,
                            error_timestamp_before: error_ts,
                        });
                    }
                    ControlOperation::ReloadAndCapture {
                        mode,
                        pause,
                        time_scale,
                        delay_frames,
                        max_width,
                        position,
                        look_at,
                        hide_ui,
                    } => {
                        // Trigger the reload
                        let mode_str = mode.clone();
                        let _ = handlers::reload::trigger_reload(
                            world,
                            mode_str.clone(),
                            *pause,
                            *time_scale,
                        );

                        let error_ts = world
                            .get_resource::<pybevy_core::LastSystemError>()
                            .map(|e| e.timestamp_secs)
                            .unwrap_or(0.0);

                        let mut pending =
                            world.get_resource_or_insert_with(PendingReloadAndCaptures::default);
                        pending.pending.push(PendingReloadAndCapture {
                            response_tx: request.response_tx,
                            mode: mode_str,
                            error_timestamp_before: error_ts,
                            reload_frames_remaining: 5,
                            screenshot_delay_frames: delay_frames.unwrap_or(30),
                            max_width: *max_width,
                            position: *position,
                            look_at: *look_at,
                            hide_ui: hide_ui.unwrap_or(true),
                            state: ReloadAndCaptureState::WaitingForReload,
                            reload_response: None,
                        });
                    }
                    ControlOperation::CaptureTurnaround {
                        look_at,
                        distance,
                        elevation,
                        view_count,
                        include_top,
                        columns,
                        max_width,
                        hide_ui,
                    } => {
                        let vc = view_count.unwrap_or(6);
                        let elev = elevation.unwrap_or(25.0);
                        let top = include_top.unwrap_or(true);

                        // Auto-fit distance and look_at from scene bounds
                        let (auto_look_at, auto_distance) =
                            if distance.is_none() || look_at.is_none() {
                                if let Some((scene_min, scene_max)) = compute_scene_bounds(world) {
                                    let center = (scene_min + scene_max) * 0.5;
                                    let extent = scene_max - scene_min;
                                    let diagonal = (extent.x * extent.x
                                        + extent.y * extent.y
                                        + extent.z * extent.z)
                                        .sqrt();
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

                        let viewpoints =
                            compute_viewpoints(final_look_at, final_distance, elev, vc, top);

                        let mut pending =
                            world.get_resource_or_insert_with(PendingTurnarounds::default);
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
                    ControlOperation::CaptureDepth {
                        position,
                        look_at,
                        sample_points,
                        grid_density,
                        include_rgb,
                        delay_frames,
                        hide_ui,
                        max_width,
                    } => {
                        // Compute depth samples synchronously
                        let depth_result = crate::handlers::depth::compute_depth_samples(
                            world,
                            position,
                            look_at,
                            sample_points,
                            grid_density,
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
                            let (forward_tx, forward_rx) = tokio::sync::oneshot::channel::<
                                Result<serde_json::Value, ControlError>,
                            >();
                            let original_tx = request.response_tx;

                            std::thread::spawn(move || {
                                let screenshot_result = forward_rx.blocking_recv();
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
                                let _ = original_tx.send(response);
                            });

                            deferred_screenshots.push(PendingScreenshot {
                                response_tx: forward_tx,
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
                            let _ = request.response_tx.send(result);
                        }
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

    // Phase 2: Execute sync handlers with one batched GIL acquire.
    // This prevents rapid Python::attach()/release cycling (which corrupts
    // GIL state after py.detach()) while keeping GIL hold time minimal.
    if !sync_requests.is_empty() {
        Python::attach(|_py| {
            for request in sync_requests {
                let result = handlers::dispatch(world, request.operation);

                // Inject pause warning into successful responses
                let result = result.map(|mut val| {
                    if let Some(time) =
                        world.get_resource::<bevy::time::Time<bevy::time::Virtual>>()
                        && time.is_paused()
                        && let Some(obj) = val.as_object_mut()
                    {
                        obj.insert("_time_paused".to_string(), serde_json::json!(true));
                    }
                    val
                });

                let _ = request.response_tx.send(result);
            }
        });
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
