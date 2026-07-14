use std::sync::{Arc, Mutex};

use bevy::{ecs::world::World, prelude::Resource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::handlers::{
    self,
    reload::{PendingReloadAndCapture, PendingReloadAndCaptures, ReloadAndCaptureState},
    schedule::{
        ActiveSchedule, ActiveSchedules, ScheduleMode, ScheduleRequest,
        SharedScheduleRegistryResource, SharedScheduleState,
    },
    screenshot::{ActiveTimeline, PendingTimelines, compute_schedule, setup_debug_camera},
    turnaround::{ActiveTurnaround, PendingTurnarounds, compute_scene_bounds, compute_viewpoints},
};

/// Maximum requests processed per frame to prevent frame spikes
const MAX_REQUESTS_PER_FRAME: usize = 32;

/// Marker component for internal overlay UI that should always be hidden in screenshots.
/// Applied to hot reload overlay entities so the screenshot handler can find and hide them.
#[derive(bevy::prelude::Component)]
pub struct InternalOverlayUi;

/// Refcount of in-flight captures (screenshots, timelines, turnarounds) that
/// want the internal overlay off-screen. Captures only increment/decrement;
/// the overlay's render system drives `Visibility` from this every frame, so
/// restoration is automatic once the count returns to zero (and survives the
/// overlay entities being respawned mid-capture by a Full hot reload).
#[derive(Resource, Default)]
pub struct OverlaySuppression(pub u32);

/// Entity can be addressed by numeric ID or Name string
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum EntityRef {
    Id(u64),
    Name(String),
}

/// Reload mode: full (default) re-executes all scene code, partial only reloads changed systems.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReloadMode {
    /// Re-execute all scene code
    #[default]
    Full,
    /// Only reload changed systems
    Partial,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryEntitiesParams {
    /// Component names entities must have
    #[serde(default)]
    pub with: Vec<String>,
    /// Component names entities must not have
    #[serde(default)]
    pub without: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetComponentParams {
    /// Entity ID or Name
    pub entity: EntityRef,
    /// Component name (e.g. 'Transform', 'PointLight')
    pub component: String,
}

/// schemars `schema_with` helper emitting `{"type": "object"}` for a
/// `serde_json::Value` field that must be a JSON object. Without an explicit
/// type, schemars renders `serde_json::Value` as an untyped (any) schema, and
/// MCP clients then serialize the argument as a JSON string, which the handlers
/// reject ("must be a JSON object").
fn json_object_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "type": "object" })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetComponentParams {
    /// Entity ID or Name
    pub entity: EntityRef,
    /// Component name
    pub component: String,
    /// Fields to update
    #[schemars(schema_with = "json_object_schema")]
    pub fields: serde_json::Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveComponentParams {
    /// Entity ID or Name
    pub entity: EntityRef,
    /// Component to remove
    pub component: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetResourceParams {
    /// Resource type name
    pub resource_type: String,
    /// Resource value as JSON
    #[schemars(schema_with = "json_object_schema")]
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SeekTimeParams {
    /// Target elapsed time in seconds
    pub seconds: f64,
    /// Pause after seeking (default true)
    #[serde(default = "default_true")]
    pub pause: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptureScreenshotParams {
    /// Frames to wait before capture (default 2)
    #[serde(default = "default_2")]
    pub delay_frames: u32,
    /// Max image width in pixels (default 768). Use 1280 for detail.
    pub max_width: Option<u32>,
    /// Camera position [x, y, z]. If set, spawns a temporary debug camera instead of using the scene camera.
    pub position: Option<[f32; 3]>,
    /// Point the camera looks at [x, y, z]. Defaults to [0, 0, 0] if position is set.
    pub look_at: Option<[f32; 3]>,
    /// Hide authored UI Node entities during capture (default true). Internal overlays (hot reload status) are always hidden regardless.
    #[serde(default = "default_true")]
    pub hide_ui: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptureTimelineParams {
    /// Frame span to capture over (~1s at 60fps)
    #[serde(default = "default_60")]
    pub total_frames: u32,
    /// Number of captures (max 20)
    #[serde(default = "default_6")]
    pub capture_count: u32,
    /// Max composite width in pixels
    pub max_width: Option<u32>,
    /// Grid columns
    #[serde(default = "default_3")]
    pub columns: u32,
    /// Debug camera position [x, y, z]
    pub position: Option<[f32; 3]>,
    /// Point the camera looks at [x, y, z]. Defaults to [0, 0, 0] if position is set.
    pub look_at: Option<[f32; 3]>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptureTurnaroundParams {
    /// Center point to orbit around. Auto-detected from scene bounds if omitted.
    pub look_at: Option<[f32; 3]>,
    /// Camera distance from center. Auto-fitted to scene bounds if omitted.
    pub distance: Option<f32>,
    /// Camera elevation in degrees (default 25)
    #[schemars(extend("default" = 25))]
    pub elevation: Option<f32>,
    /// Number of orbit positions (default 6)
    #[schemars(extend("default" = 6))]
    pub view_count: Option<u32>,
    /// Include top-down view (default true)
    #[schemars(extend("default" = true))]
    pub include_top: Option<bool>,
    /// Grid columns in contact sheet (default 3)
    #[schemars(extend("default" = 3))]
    pub columns: Option<u32>,
    /// Max composite width (default 1200)
    #[schemars(extend("default" = 1200))]
    pub max_width: Option<u32>,
    /// Hide UI during capture (default true)
    #[schemars(extend("default" = true))]
    pub hide_ui: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptureDepthParams {
    /// Camera position [x, y, z]
    pub position: Option<[f32; 3]>,
    /// Camera look-at [x, y, z]
    pub look_at: Option<[f32; 3]>,
    /// Screen-space sample points [[x, y], ...]. Auto-generates grid if omitted.
    pub sample_points: Option<Vec<[u32; 2]>>,
    /// Auto-generate NxN sample grid (default 8 if no sample_points)
    #[schemars(extend("default" = 8))]
    pub grid_density: Option<u32>,
    /// Include RGB screenshot (default true)
    #[schemars(extend("default" = true))]
    pub include_rgb: Option<bool>,
    /// Frames to wait before capture (default 2)
    #[schemars(extend("default" = 2))]
    pub delay_frames: Option<u32>,
    /// Hide UI during capture (default true)
    #[schemars(extend("default" = true))]
    pub hide_ui: Option<bool>,
    /// Max screenshot width (default 768)
    #[schemars(extend("default" = 768))]
    pub max_width: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReloadParams {
    /// Reload mode
    #[serde(default)]
    pub mode: ReloadMode,
    /// Pause time immediately (before reload runs). Scene starts frozen so you can capture at t=0.
    #[serde(default)]
    pub pause: bool,
    /// Set time scale atomically with reload (e.g. 0.1 for slow-motion). Applied before any frames run.
    pub time_scale: Option<f32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReloadAndCaptureParams {
    /// Reload mode (default full)
    #[serde(default)]
    pub mode: ReloadMode,
    /// Pause time before reload
    #[serde(default)]
    pub pause: bool,
    /// Set time scale atomically with reload
    pub time_scale: Option<f32>,
    /// Frames to wait after reload before capture (default 30)
    #[schemars(extend("default" = 30))]
    pub delay_frames: Option<u32>,
    /// Max screenshot width (default 768)
    pub max_width: Option<u32>,
    /// Debug camera position [x, y, z]
    pub position: Option<[f32; 3]>,
    /// Camera look-at point [x, y, z]
    pub look_at: Option<[f32; 3]>,
    /// Hide UI during capture (default true)
    #[schemars(extend("default" = true))]
    pub hide_ui: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QuerySpatialParams {
    /// First entity (ID or Name)
    pub entity_a: EntityRef,
    /// Second entity (ID or Name)
    pub entity_b: EntityRef,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QuerySpatialNeighborhoodParams {
    /// Center entity
    pub entity: EntityRef,
    /// Search radius
    pub radius: f32,
    /// Max neighbors to return (default 50)
    #[schemars(extend("default" = 50))]
    pub max_results: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckOverlapsParams {
    /// Entity to check
    pub entity: EntityRef,
    /// Include siblings under same parent (default false, since parented parts overlap by design)
    #[serde(default)]
    pub include_siblings: bool,
    /// Max gap (units) between entity bottom and surface below to still count as grounded (default 0.1). Increase for scenes with small placement gaps.
    #[schemars(extend("default" = 0.1))]
    #[serde(default)]
    pub max_float_gap: f32,
    /// Ground plane Y coordinate for sunk-detection. When provided, entities whose world AABB min_y is below this value are flagged as sunken. Useful for detecting GLB models placed at origin that are half-buried below the ground plane.
    pub ground_y: Option<f32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckAllOverlapsParams {
    /// Minimum overlap depth to report (default 0.001)
    #[schemars(extend("default" = 0.001))]
    pub min_penetration: Option<f32>,
    /// Max overlapping pairs to return (default 100)
    #[schemars(extend("default" = 100))]
    pub max_results: Option<usize>,
    /// Max gap to still count as grounded (default 0.1)
    #[schemars(extend("default" = 0.1))]
    #[serde(default)]
    pub max_float_gap: f32,
    /// Ground plane Y for sunk-detection
    pub ground_y: Option<f32>,
    /// Include siblings under same parent (default false)
    #[serde(default)]
    pub include_siblings: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetAssetParams {
    /// Entity ID or Name
    pub entity: EntityRef,
    /// Handle component: MeshMaterial3d, Mesh3d, MeshMaterial2d, AudioPlayer
    pub component: String,
    /// Asset type: StandardMaterial, Mesh, ColorMaterial, AudioSource
    pub asset_type: String,
    /// Fields to update on the asset
    #[schemars(schema_with = "json_object_schema")]
    pub fields: serde_json::Value,
}

/// All control operations that require World access.
///
/// To deserialize from an MCP tool call: inject the tool name as `"tool"` into
/// the args object, then call `serde_json::from_value::<ControlOperation>(args)`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum ControlOperation {
    /// List all entities in the scene.
    #[schemars(extend("x-hidden" = true))]
    ListEntities,
    /// Get details for a single entity.
    #[schemars(extend("x-hidden" = true))]
    GetEntity {
        /// Entity ID or Name
        entity: EntityRef,
    },
    /// List all resources in the world.
    #[schemars(extend("x-hidden" = true))]
    ListResources,
    /// List all registered systems.
    #[schemars(extend("x-hidden" = true))]
    ListSystems,
    /// Get component field names, types, defaults, and a JSON spawn example.
    GetComponentSchema {
        /// Component name (e.g. 'Transform')
        name: String,
    },
    /// Get live field values for a specific component on an entity.
    GetComponent(GetComponentParams),
    /// Query entities by With/Without component filters. Also supports Name lookup.
    QueryEntities(QueryEntitiesParams),
    /// Get a grouped entity inventory: counts by type (e.g. '30 Bubble, 6 Fish, 1 Camera3d'). Faster than query_entities for understanding scene composition.
    GetSceneSummary,
    /// Get the axis-aligned bounding box (AABB) of an entity, both local and world-space. Requires entity to have a mesh.
    GetBoundingBox {
        /// Entity ID or Name
        entity: EntityRef,
    },
    /// Show bridge registry state, entity count, and component detection status.
    GetRegistry,

    /// Capture a screenshot. Default 768px wide. UI elements are hidden by default. Use position/look_at to capture from an arbitrary viewpoint without affecting the scene camera.
    #[schemars(extend("x-feature-gate" = "screenshot"))]
    CaptureScreenshot(CaptureScreenshotParams),
    /// Capture a screenshot with entity ID/Name gizmo labels overlaid.
    #[schemars(extend("x-hidden" = true))]
    CaptureWithGizmos(CaptureScreenshotParams),
    /// Capture multiple frames over time into a contact sheet. Shows animation/motion in one image.
    #[schemars(extend("x-feature-gate" = "screenshot"))]
    CaptureTimeline(CaptureTimelineParams),
    /// Capture multiple viewpoints orbiting around a target, composited into one contact sheet. Auto-fits distance to scene bounds if not specified.
    #[schemars(extend("x-feature-gate" = "screenshot"))]
    CaptureTurnaround(CaptureTurnaroundParams),
    /// Capture RGB screenshot + ray-AABB depth samples. Casts rays from camera through sample points (or auto-grid) against entity bounding boxes. Returns structured depth data with hit entity, distance, and world position.
    #[schemars(extend("x-feature-gate" = "screenshot"))]
    CaptureDepth(CaptureDepthParams),

    /// Hot-reload the scene (full or partial). Waits for reload to complete before responding - response includes any errors or warnings.
    Reload(ReloadParams),
    /// Primary iteration tool. Reload and capture in one round-trip. Hot-reloads the scene, waits for completion, checks for errors, then captures a screenshot.
    #[schemars(extend("x-feature-gate" = "screenshot"))]
    ReloadAndCapture(ReloadAndCaptureParams),
    /// Get current reload generation, mode, and enabled state.
    GetReloadStatus,
    /// Get the last Python system error traceback.
    GetLastError,

    /// Spawn a new entity with components specified as JSON.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    SpawnEntity {
        /// Component name -> field values (e.g. {"Transform": {"translation": [0, 5, 0]}})
        #[schemars(schema_with = "json_object_schema")]
        components: serde_json::Value,
    },
    /// Remove an entity by ID or Name.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    DespawnEntity {
        /// Entity ID or Name
        entity: EntityRef,
    },
    /// Update specific fields on a component without replacing it.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    SetComponent(SetComponentParams),
    /// Remove a component from an entity.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    RemoveComponent(RemoveComponentParams),
    /// Insert or update a resource on a running scene (in scene code, use commands.insert_resource() instead).
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    SetResource(SetResourceParams),
    /// Remove a resource from the world.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    RemoveResource {
        /// Resource type name
        resource_type: String,
    },
    /// Execute multiple mutation operations in a single round-trip. Each operation runs independently - failures don't abort the batch. Actions: set_component, spawn, despawn, remove_component.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    Batch {
        /// Array of operations to execute. Each item: {"action": "set_component|spawn|despawn|remove_component", "entity": id_or_name, ...}
        operations: Vec<serde_json::Value>,
    },
    /// Update asset properties (material color, mesh settings) live without code reload.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    SetAsset(SetAssetParams),

    /// Pause game time. Scene freezes but rendering continues. Useful for inspecting a specific moment.
    PauseTime,
    /// Resume game time after pause.
    ResumeTime,
    /// Set game speed multiplier (0.1 = slow-mo, 2.0 = 2x speed). Does not affect pause state.
    SetTimeScale {
        /// Speed multiplier (default 1.0)
        scale: f32,
    },
    /// Get current time state: paused, speed, elapsed seconds.
    GetTimeStatus,
    /// Jump virtual time to a specific moment. Seeking backwards resets virtual time (preserves speed). Pauses by default so you can capture at that exact time.
    SeekTime(SeekTimeParams),

    /// Spatial query between two entities: distance, direction, AABB overlap. For finding all entities within a radius, use query_spatial_neighborhood instead.
    QuerySpatial(QuerySpatialParams),
    /// Find all entities within radius of a center entity.
    QuerySpatialNeighborhood(QuerySpatialNeighborhoodParams),
    /// Detect AABB overlaps for a single entity against all others.
    CheckOverlaps(CheckOverlapsParams),
    /// Detect all AABB overlaps scene-wide.
    CheckAllOverlaps(CheckAllOverlapsParams),

    /// Run arbitrary Python code with World context. Returns stdout/stderr capture and change summary.
    #[schemars(extend("x-feature-gate" = "execute_python"))]
    RunCode {
        /// Python code to execute
        code: String,
    },
    /// Get full diagnostics: FPS, CPU/GPU/RAM/VRAM usage, entity/asset counts, system profiling times.
    GetPerformance,
    /// Submit batched, timed tool calls that execute inside the engine frame loop with frame-precise timing. Same-'at' actions fire in the same frame (atomic). Supports: time control, mutations, queries, screenshots. NOT supported: get_logs, search_api, run_scene (bridge-local tools).
    ScheduleActions(ScheduleRequest),
    /// Get internal config value.
    #[schemars(extend("x-hidden" = true))]
    GetConfig {
        /// Config key
        key: String,
    },
    /// List all config values.
    #[schemars(extend("x-hidden" = true))]
    ListConfigs,
}

fn default_2() -> u32 {
    2
}
fn default_3() -> u32 {
    3
}
fn default_6() -> u32 {
    6
}
fn default_60() -> u32 {
    60
}
fn default_true() -> bool {
    true
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

/// Default screenshot width when the caller doesn't pass `max_width`.
/// Keep in sync with the "(default 768)" schema docs on the capture params.
pub const DEFAULT_SCREENSHOT_MAX_WIDTH: u32 = 768;

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
    pub mode: ReloadMode,
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

pub fn push_pending_screenshot(
    params: CaptureScreenshotParams,
    with_gizmos: bool,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    deferred: &mut Vec<PendingScreenshot>,
) {
    let debug_camera = params.position.map(|pos| DebugCameraRequest {
        position: pos,
        look_at: params.look_at.unwrap_or([0.0, 0.0, 0.0]),
    });
    deferred.push(PendingScreenshot {
        response_tx,
        frames_remaining: params.delay_frames,
        with_gizmos,
        max_width: params.max_width.or(Some(DEFAULT_SCREENSHOT_MAX_WIDTH)),
        debug_camera,
        hide_ui: params.hide_ui,
        extra_response: None,
    });
}

pub fn push_pending_timeline(
    params: CaptureTimelineParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    let mut schedule = compute_schedule(params.total_frames, params.capture_count);

    let debug_cleanup = params.position.map(|pos| {
        let debug_req = DebugCameraRequest {
            position: pos,
            look_at: params.look_at.unwrap_or([0.0, 0.0, 0.0]),
        };
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
            response_tx: Some(response_tx),
            max_width: params.max_width,
            columns: params.columns,
            debug_cleanup,
            schedule,
            total_captures: params.capture_count,
            next_capture_index: 0,
            collected: Vec::new(),
            overlay_suppressed: false,
        },
    );
}

pub fn push_pending_reload(
    params: ReloadParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    // Surface validation errors (e.g. an out-of-range time_scale) directly
    // instead of dropping the bad param and reporting a false success later.
    if let Err(e) = handlers::reload::trigger_reload(
        world,
        params.mode.clone(),
        params.pause,
        params.time_scale,
    ) {
        let _ = response_tx.send(Err(e));
        return;
    }

    let error_ts = world
        .get_resource::<pybevy_core::LastSystemError>()
        .map(|e| e.timestamp_secs)
        .unwrap_or(0.0);

    let mut pending = world.get_resource_or_insert_with(PendingReloadResponses::default);
    pending.pending.push(PendingReloadResponse {
        response_tx,
        frames_remaining: 5,
        mode: params.mode,
        error_timestamp_before: error_ts,
    });
}

pub fn push_pending_reload_and_capture(
    params: ReloadAndCaptureParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    // Surface validation errors (e.g. an out-of-range time_scale) directly.
    // Otherwise the caller would silently drop the bad param and still receive
    // a false "reload_completed" from the deferred response waiter below.
    if let Err(e) = handlers::reload::trigger_reload(
        world,
        params.mode.clone(),
        params.pause,
        params.time_scale,
    ) {
        let _ = response_tx.send(Err(e));
        return;
    }

    let error_ts = world
        .get_resource::<pybevy_core::LastSystemError>()
        .map(|e| e.timestamp_secs)
        .unwrap_or(0.0);

    let mut pending = world.get_resource_or_insert_with(PendingReloadAndCaptures::default);
    pending.pending.push(PendingReloadAndCapture {
        response_tx,
        mode: params.mode,
        error_timestamp_before: error_ts,
        reload_frames_remaining: 5,
        screenshot_delay_frames: params.delay_frames.unwrap_or(30),
        max_width: params.max_width,
        position: params.position,
        look_at: params.look_at,
        hide_ui: params.hide_ui.unwrap_or(true),
        state: ReloadAndCaptureState::WaitingForReload,
        reload_response: None,
    });
}

pub fn push_pending_turnaround(
    params: CaptureTurnaroundParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    let vc = params.view_count.unwrap_or(6);
    let elev = params.elevation.unwrap_or(25.0);
    let top = params.include_top.unwrap_or(true);

    let (auto_look_at, auto_distance) = if params.distance.is_none() || params.look_at.is_none() {
        if let Some((scene_min, scene_max)) = compute_scene_bounds(world) {
            let center = (scene_min + scene_max) * 0.5;
            let extent = scene_max - scene_min;
            let diagonal = (extent.x * extent.x + extent.y * extent.y + extent.z * extent.z).sqrt();
            let dist = diagonal / (30.0_f32.to_radians().tan() * 2.0);
            ([center.x, center.y, center.z], dist.max(2.0))
        } else {
            ([0.0, 0.0, 0.0], 10.0)
        }
    } else {
        ([0.0, 0.0, 0.0], 10.0)
    };

    let final_look_at = params.look_at.unwrap_or(auto_look_at);
    let final_distance = params.distance.unwrap_or(auto_distance);
    let viewpoints = compute_viewpoints(final_look_at, final_distance, elev, vc, top);

    let mut pending = world.get_resource_or_insert_with(PendingTurnarounds::default);
    pending.active.push(ActiveTurnaround {
        response_tx: Some(response_tx),
        viewpoints,
        current_index: 0,
        captures: Vec::new(),
        columns: params.columns.unwrap_or(3),
        max_width: Some(params.max_width.unwrap_or(1200)),
        frames_remaining: 0,
        hide_ui: params.hide_ui.unwrap_or(true),
        ui_restore: None,
        overlay_suppressed: false,
        debug_cleanup: None,
        look_at: final_look_at,
        pending_screenshot_entity: None,
    });
}

pub fn push_pending_depth(
    params: CaptureDepthParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    deferred: &mut Vec<PendingScreenshot>,
    world: &mut World,
) {
    let depth_result = crate::handlers::depth::compute_depth_samples(
        world,
        &params.position,
        &params.look_at,
        &params.sample_points,
        &params.grid_density,
    );

    let want_rgb = params.include_rgb.unwrap_or(true);
    let df = params.delay_frames.unwrap_or(2);
    let mw = Some(params.max_width.unwrap_or(DEFAULT_SCREENSHOT_MAX_WIDTH));
    let hu = params.hide_ui.unwrap_or(true);
    let dc = params.position.as_ref().map(|pos| DebugCameraRequest {
        position: *pos,
        look_at: params.look_at.unwrap_or([0.0, 0.0, 0.0]),
    });

    if want_rgb {
        let depth = match depth_result {
            Ok(d) => d,
            Err(e) => {
                let _ = response_tx.send(Err(e));
                return;
            }
        };

        deferred.push(PendingScreenshot {
            response_tx,
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
        let _ = response_tx.send(result);
    }
}

/// Handle ScheduleActions requests (no GIL needed).
fn handle_submit_schedule(
    sched_req: ScheduleRequest,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    let mut active = world.get_resource_or_insert_with(ActiveSchedules::default);
    let schedule_id = format!("schedule-{}", active.next_id);
    active.next_id += 1;

    let t0 = world
        .get_resource::<bevy::time::Time<bevy::time::Virtual>>()
        .map(|t| t.elapsed_secs_f64())
        .unwrap_or(0.0);

    if sched_req.mode == ScheduleMode::Async {
        let shared = std::sync::Arc::new(std::sync::Mutex::new(SharedScheduleState::new(
            &schedule_id,
            sched_req.actions.len(),
        )));

        if let Some(registry_res) = world.get_resource::<SharedScheduleRegistryResource>() {
            registry_res.0.insert(schedule_id.clone(), shared.clone());
        }

        let _ = response_tx.send(Ok(serde_json::json!({
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
            response_tx,
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

                // Handle ScheduleActions (no GIL needed - just stores in World resource)
                if let ControlOperation::ScheduleActions(sched) = request.operation {
                    handle_submit_schedule(sched, request.response_tx, world);
                    continue;
                }

                // Classify: deferred ops go directly to their queues,
                // sync ops are collected for batched GIL processing
                match request.operation {
                    ControlOperation::CaptureScreenshot(p) => {
                        push_pending_screenshot(
                            p,
                            false,
                            request.response_tx,
                            &mut deferred_screenshots,
                        );
                    }
                    ControlOperation::CaptureWithGizmos(p) => {
                        push_pending_screenshot(
                            p,
                            true,
                            request.response_tx,
                            &mut deferred_screenshots,
                        );
                    }
                    ControlOperation::CaptureTimeline(p) => {
                        push_pending_timeline(p, request.response_tx, world);
                    }
                    ControlOperation::Reload(p) => {
                        push_pending_reload(p, request.response_tx, world);
                    }
                    ControlOperation::ReloadAndCapture(p) => {
                        push_pending_reload_and_capture(p, request.response_tx, world);
                    }
                    ControlOperation::CaptureTurnaround(p) => {
                        push_pending_turnaround(p, request.response_tx, world);
                    }
                    ControlOperation::CaptureDepth(p) => {
                        push_pending_depth(
                            p,
                            request.response_tx,
                            &mut deferred_screenshots,
                            world,
                        );
                    }
                    other => {
                        sync_requests.push(ControlRequest {
                            operation: other,
                            response_tx: request.response_tx,
                        });
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
            .remove_non_send::<Box<dyn crate::runtime::ControlRuntime>>()
            .expect("ControlRuntime resource missing");

        runtime.dispatch_batch(world, sync_requests);

        world.insert_non_send(runtime);
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
    use pybevy_core::bridge_inventory::collect_all;
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
    fn push_pending_screenshot_applies_default_max_width() {
        let (tx, _rx) = oneshot::channel();
        let params: CaptureScreenshotParams = serde_json::from_str("{}").unwrap();
        let mut deferred = Vec::new();
        push_pending_screenshot(params, false, tx, &mut deferred);
        assert_eq!(deferred[0].max_width, Some(DEFAULT_SCREENSHOT_MAX_WIDTH));
    }

    #[test]
    fn push_pending_screenshot_keeps_explicit_max_width() {
        let (tx, _rx) = oneshot::channel();
        let params: CaptureScreenshotParams =
            serde_json::from_str(r#"{"max_width": 1280}"#).unwrap();
        let mut deferred = Vec::new();
        push_pending_screenshot(params, false, tx, &mut deferred);
        assert_eq!(deferred[0].max_width, Some(1280));
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
                operation: ControlOperation::ListEntities,
                response_tx: tx,
            })
            .unwrap();

        let req = receiver.rx.try_recv().unwrap();
        assert!(matches!(req.operation, ControlOperation::ListEntities));
    }

    #[test]
    fn push_pending_reload_surfaces_time_scale_error() {
        // Regression: trigger_reload's Result was discarded with `let _ =`, so a
        // rejected time_scale silently dropped the param and the deferred waiter
        // still reported a false "reload_completed". The error must reach the
        // caller and no response waiter should be queued.
        let mut world = World::new();
        world.init_resource::<bevy::time::Time<bevy::time::Virtual>>();
        let (tx, mut rx) = oneshot::channel();
        push_pending_reload(
            ReloadParams {
                mode: ReloadMode::Full,
                pause: false,
                time_scale: Some(1.0e6),
            },
            tx,
            &mut world,
        );
        let result = rx.try_recv().expect("caller should receive a response");
        let err = result.expect_err("out-of-range time_scale must be an error");
        assert!(err.message.contains("1000"));
        // No deferred waiter queued, and the bad scale was not applied.
        let queued = world
            .get_resource::<PendingReloadResponses>()
            .map(|p| p.pending.len())
            .unwrap_or(0);
        assert_eq!(queued, 0);
        let speed = world
            .resource::<bevy::time::Time<bevy::time::Virtual>>()
            .relative_speed();
        assert!((speed - 1.0).abs() < 1e-6);
    }

    #[test]
    fn push_pending_reload_valid_time_scale_defers() {
        // A valid time_scale applies and queues the deferred waiter (no immediate
        // response), confirming the error path did not regress the happy path.
        let mut world = World::new();
        world.init_resource::<bevy::time::Time<bevy::time::Virtual>>();
        let (tx, mut rx) = oneshot::channel();
        push_pending_reload(
            ReloadParams {
                mode: ReloadMode::Full,
                pause: false,
                time_scale: Some(2.0),
            },
            tx,
            &mut world,
        );
        assert!(
            rx.try_recv().is_err(),
            "response is deferred, not immediate"
        );
        let queued = world
            .get_resource::<PendingReloadResponses>()
            .map(|p| p.pending.len())
            .unwrap_or(0);
        assert_eq!(queued, 1);
        let speed = world
            .resource::<bevy::time::Time<bevy::time::Virtual>>()
            .relative_speed();
        assert!((speed - 2.0).abs() < 1e-6);
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
                world.insert_non_send(Box::new(Pyo3ControlRuntime) as Box<dyn ControlRuntime>);

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
                            operation: ControlOperation::GetComponent(GetComponentParams {
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

    #[test]
    fn control_operation_schema_generates() {
        let schema = schemars::schema_for!(ControlOperation);
        let json = serde_json::to_string_pretty(&schema).unwrap();
        // Basic sanity: schema should contain tool names as snake_case
        assert!(json.contains("query_entities"), "missing query_entities");
        assert!(
            json.contains("capture_screenshot"),
            "missing capture_screenshot"
        );
        assert!(json.contains("pause_time"), "missing pause_time");
        assert!(json.contains("spawn_entity"), "missing spawn_entity");
        assert!(
            json.contains("schedule_actions"),
            "missing schedule_actions"
        );
        // Should contain descriptions from doc comments
        assert!(json.contains("description"), "no descriptions in schema");
    }
}
