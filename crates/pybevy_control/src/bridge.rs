use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use bevy::{ecs::world::World, prelude::Resource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::handlers::{
    self,
    frame_analysis::{DEFAULT_COMPARE_EPSILON, FrameStatsOptions, validate_frame_stats_options},
    reload::{PendingReloadAndCapture, PendingReloadAndCaptures},
    schedule::{
        ActiveSchedule, ActiveSchedules, ScheduleMode, ScheduleRequest,
        SharedScheduleRegistryResource, SharedScheduleState,
    },
    screenshot::{
        ActiveTimeline, MAX_TIMELINE_CAPTURES, PendingTimelines, compute_schedule,
        headless_frame_sequence, prepare_capture_visibility, setup_debug_camera,
    },
    turnaround::{
        ActiveTurnaround, MAX_TURNAROUND_VIEWS, PendingTurnarounds, compute_scene_bounds,
        compute_viewpoints,
    },
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
#[serde(deny_unknown_fields)]
pub struct QueryEntitiesParams {
    /// Component names entities must have
    #[serde(default)]
    pub with: Vec<String>,
    /// Component names entities must not have
    #[serde(default)]
    pub without: Vec<String>,
    /// Maximum number of entity records to return; the response still reports the total number of matches.
    #[serde(default = "default_query_entity_limit")]
    #[schemars(range(max = 1000))]
    pub limit: usize,
}

pub const MAX_QUERY_ENTITY_LIMIT: usize = 1000;

const fn default_query_entity_limit() -> usize {
    100
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetComponentParams {
    /// Entity ID or Name
    pub entity: EntityRef,
    /// Component name (e.g. 'Transform', 'PointLight')
    pub component: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetResourceParams {
    /// Resource type name (e.g. 'ClearColor', 'State[game.Phase]')
    pub resource_type: String,
}

/// schemars `schema_with` helper emitting `{"type": "object"}` for a
/// `serde_json::Value` field that must be a JSON object. Without an explicit
/// type, schemars renders `serde_json::Value` as an untyped (any) schema, and
/// MCP clients then serialize the argument as a JSON string, which the handlers
/// reject ("must be a JSON object").
pub(crate) fn json_object_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "type": "object" })
}

/// schemars `schema_with` helper for arrays whose elements must be JSON
/// objects. `Vec<serde_json::Value>` otherwise emits the valid JSON Schema
/// boolean form `{"items": true}`, which some function-calling providers do
/// not accept.
fn json_object_array_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "array",
        "items": { "type": "object" }
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct RemoveComponentParams {
    /// Entity ID or Name
    pub entity: EntityRef,
    /// Component to remove
    pub component: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetResourceParams {
    /// Resource type name
    pub resource_type: String,
    /// Fields to update as JSON. Custom resources must declare annotated fields, typically with `@resource` above `@dataclass`; attributes created only in `__init__` are not editable here.
    #[schemars(schema_with = "json_object_schema")]
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeekTimeParams {
    /// Target elapsed time in seconds
    pub seconds: f64,
    /// Pause after seeking (default true)
    #[serde(default = "default_true")]
    pub pause: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureScreenshotParams {
    /// Optional entity name or numeric ID to isolate, including its descendants.
    pub entity: Option<EntityRef>,
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
    /// Include gizmos drawn by Python systems, native hosts, or extensions (default false). Does not generate entity labels.
    #[serde(default)]
    pub gizmos: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureStatsParams {
    /// Optional entity name or numeric ID to isolate, including its descendants.
    pub entity: Option<EntityRef>,
    /// Divide the selected region into an NxN grid (default 1, max 16).
    #[serde(default = "default_1")]
    #[schemars(range(min = 1, max = 16))]
    pub grid: u32,
    /// Optional [x, y, width, height] sub-rectangle in resized output pixels.
    pub region: Option<[i64; 4]>,
    /// Optional output-pixel coordinates to sample (max 256).
    #[schemars(extend("maxItems" = 256))]
    pub sample_points: Option<Vec<[i64; 2]>>,
    /// Frames to wait before capture (default 2).
    #[serde(default = "default_2")]
    pub delay_frames: u32,
    /// Maximum analyzed width in pixels (default 768).
    #[schemars(range(min = 1))]
    pub max_width: Option<u32>,
    /// Camera position [x, y, z]. If set, temporarily reuses a Camera3d.
    pub position: Option<[f32; 3]>,
    /// Point the camera looks at [x, y, z]. Defaults to [0, 0, 0] with position.
    pub look_at: Option<[f32; 3]>,
    /// Hide authored UI during capture (default true).
    #[serde(default = "default_true")]
    pub hide_ui: bool,
    /// Include gizmos drawn by Python systems, native hosts, or extensions (default false).
    #[serde(default)]
    pub gizmos: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompareFramesParams {
    /// First retained frame ID.
    pub a: String,
    /// Second retained frame ID.
    pub b: String,
    /// A pixel is changed when its largest normalized channel difference exceeds this value.
    #[serde(default = "default_compare_epsilon")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub epsilon: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureTimelineParams {
    /// Frame span to capture over (~1s at 60fps)
    #[serde(default = "default_60")]
    pub total_frames: u32,
    /// Number of captures (max 20)
    #[serde(default = "default_6")]
    pub capture_count: u32,
    /// Max composite width in pixels (default 1200)
    #[schemars(extend("default" = 1200))]
    pub max_width: Option<u32>,
    /// Grid columns
    #[serde(default = "default_3")]
    pub columns: u32,
    /// Debug camera position [x, y, z]
    pub position: Option<[f32; 3]>,
    /// Point the camera looks at [x, y, z]. Defaults to [0, 0, 0] if position is set.
    pub look_at: Option<[f32; 3]>,
    /// Hide authored UI Node entities during capture (default true). Internal overlays are always hidden.
    #[serde(default = "default_true")]
    pub hide_ui: bool,
    /// Include gizmos drawn by Python systems, native hosts, or extensions (default false).
    #[serde(default)]
    pub gizmos: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureTurnaroundParams {
    /// Center point to orbit around. Auto-detected from scene bounds if omitted.
    pub look_at: Option<[f32; 3]>,
    /// Camera distance from center. Auto-fitted to scene bounds if omitted.
    #[schemars(extend("exclusiveMinimum" = 0.0))]
    pub distance: Option<f32>,
    /// Camera elevation in degrees (default 25)
    #[schemars(extend("default" = 25))]
    pub elevation: Option<f32>,
    /// Number of orbit positions (default 6)
    #[schemars(extend("default" = 6, "minimum" = 1, "maximum" = 20))]
    pub view_count: Option<u32>,
    /// Include top-down view (default true)
    #[schemars(extend("default" = true))]
    pub include_top: Option<bool>,
    /// Maximum grid columns in the contact sheet (default 3). The compositor may use fewer to avoid empty cells.
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
#[serde(deny_unknown_fields)]
pub struct CaptureDepthParams {
    /// Camera position [x, y, z]
    pub position: Option<[f32; 3]>,
    /// Camera look-at [x, y, z]
    pub look_at: Option<[f32; 3]>,
    /// Screen-space sample points [[x, y], ...], with coordinates in [0, 800).
    /// Auto-generates a grid if omitted.
    pub sample_points: Option<Vec<[i64; 2]>>,
    /// Auto-generate NxN sample grid (default 8 if no sample_points)
    #[schemars(extend("default" = 8))]
    #[schemars(range(min = 1))]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct QuerySpatialParams {
    /// First entity (ID or Name)
    pub entity_a: EntityRef,
    /// Second entity (ID or Name)
    pub entity_b: EntityRef,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuerySpatialNeighborhoodParams {
    /// Center entity
    pub entity: EntityRef,
    /// Search radius
    #[schemars(range(min = 0.0))]
    pub radius: f32,
    /// Max neighbors to return (default 50)
    #[schemars(extend("default" = 50))]
    pub max_results: Option<usize>,
}

pub(crate) fn default_max_float_gap() -> f32 {
    0.1
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckOverlapsParams {
    /// Entity to check
    pub entity: EntityRef,
    /// Include siblings under same parent (default false, since parented parts overlap by design)
    #[serde(default)]
    pub include_siblings: bool,
    /// Max gap (units) between entity bottom and surface below to still count as grounded (default 0.1). Increase for scenes with small placement gaps.
    #[schemars(extend("default" = 0.1))]
    #[serde(default = "default_max_float_gap")]
    pub max_float_gap: f32,
    /// Ground plane Y coordinate for sunk-detection. When provided, entities whose world AABB min_y is below this value are flagged as sunken. Useful for detecting GLB models placed at origin that are half-buried below the ground plane.
    pub ground_y: Option<f32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckAllOverlapsParams {
    /// Minimum overlap depth to report (default 0.001)
    #[schemars(extend("default" = 0.001))]
    pub min_penetration: Option<f32>,
    /// Max overlapping pairs to return (default 100)
    #[schemars(extend("default" = 100))]
    pub max_results: Option<usize>,
    /// Max gap between an entity bottom and a physical surface or ground_y to still count as grounded (default 0.1)
    #[schemars(extend("default" = 0.1))]
    #[serde(default = "default_max_float_gap")]
    pub max_float_gap: f32,
    /// Ground plane Y for sunk-detection
    pub ground_y: Option<f32>,
    /// Include siblings under same parent (default false)
    #[serde(default)]
    pub include_siblings: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
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
#[serde(tag = "tool", rename_all = "snake_case", deny_unknown_fields)]
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
    /// Get scene-owned systems by stage, optionally including engine internals.
    #[serde(rename = "get_system_list")]
    ListSystems {
        /// Include Bevy, PyBevy, and host-internal systems.
        #[serde(default)]
        include_internal: bool,
    },
    /// Get component field names, types, defaults, and a JSON spawn example.
    GetComponentSchema {
        /// Component name (e.g. 'Transform')
        name: String,
    },
    /// Get live field values for a specific component on an entity.
    GetComponent(GetComponentParams),
    /// Get the presence and live field values of a resource.
    GetResource(GetResourceParams),
    /// Query entities by With/Without component filters. Returns at most 100 records by default; use limit up to 1000 and inspect total_count/truncated.
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
    /// Capture numeric RGB/luma statistics, grid cells, and optional sampled pixels without returning a PNG.
    #[schemars(extend("x-feature-gate" = "screenshot"))]
    CaptureStats(CaptureStatsParams),
    /// Compare two retained capture frames and report numeric pixel differences and their bounding box.
    #[schemars(extend("x-feature-gate" = "screenshot"))]
    CompareFrames(CompareFramesParams),
    /// Capture a screenshot including existing gizmos.
    #[schemars(extend("x-hidden" = true))]
    CaptureWithGizmos(CaptureScreenshotParams),
    /// Capture multiple frames over time into a contact sheet. UI is hidden by default; gizmos are optional.
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
    /// Insert or update a resource on a running scene. Custom resources must declare annotated fields, typically with `@resource` above `@dataclass`; attributes created only in `__init__` are not editable here. In scene code, use commands.insert_resource() instead.
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
        #[schemars(schema_with = "json_object_array_schema")]
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
    /// Jump virtual time to a specific moment. Absolute-time systems observe the new elapsed time; delta-accumulated state is not replayed. Seeking backwards resets virtual time (preserves speed). Pauses by default.
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

fn default_1() -> u32 {
    1
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
fn default_compare_epsilon() -> f64 {
    DEFAULT_COMPARE_EPSILON
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

/// App-local gate shared by the HTTP server and the exclusive control system.
///
/// While `run_code` owns the gate, another control request cannot be serviced:
/// the engine thread is executing Python with exclusive World access. HTTP
/// admission checks this state before queueing so a callback into the same
/// server fails immediately instead of waiting on the World it already owns.
#[derive(Resource, Clone, Default)]
pub(crate) struct SharedExclusiveExecution {
    active: Arc<AtomicBool>,
}

impl SharedExclusiveExecution {
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub(crate) fn try_enter(&self) -> Option<ExclusiveExecutionGuard> {
        self.active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| ExclusiveExecutionGuard {
                state: self.clone(),
            })
    }
}

pub(crate) struct ExclusiveExecutionGuard {
    state: SharedExclusiveExecution,
}

impl Drop for ExclusiveExecutionGuard {
    fn drop(&mut self) {
        self.state.active.store(false, Ordering::Release);
    }
}

/// Clonable sender for the control channel (stored in HTTP server state)
#[derive(Clone)]
pub struct ControlSender {
    pub tx: mpsc::UnboundedSender<ControlRequest>,
    exclusive_execution: SharedExclusiveExecution,
}

impl ControlSender {
    pub(crate) fn exclusive_execution(&self) -> SharedExclusiveExecution {
        self.exclusive_execution.clone()
    }
}

/// Default screenshot width when the caller doesn't pass `max_width`.
/// Keep in sync with the "(default 768)" schema docs on the capture params.
pub const DEFAULT_SCREENSHOT_MAX_WIDTH: u32 = 768;
/// Default contact-sheet width for `capture_timeline`, matching the value
/// `capture_turnaround` uses for the same composition.
pub const DEFAULT_TIMELINE_MAX_WIDTH: u32 = 1200;

/// Bevy resource for pending screenshot responses (deferred until after render)
#[derive(Resource, Default)]
pub struct PendingScreenshots {
    pub pending: Vec<PendingScreenshot>,
}

pub struct PendingScreenshot {
    pub response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    pub frames_remaining: u32,
    /// A scheduled scene mutation may require a render-world pipeline update
    /// before its capture delay begins.
    pub required_render_epoch: Option<u64>,
    pub with_gizmos: bool,
    /// Original gizmo state captured before Update draws for this request.
    pub gizmo_restore: Option<bool>,
    pub max_width: Option<u32>,
    pub debug_camera: Option<DebugCameraRequest>,
    pub hide_ui: bool,
    pub entity: Option<EntityRef>,
    pub response_kind: CaptureResponseKind,
    /// Extra JSON fields to merge into the screenshot response.
    /// Used by `capture_depth` and `reload_and_capture` to avoid spawning
    /// a thread for the merge.
    pub extra_response: Option<serde_json::Value>,
}

#[derive(Debug)]
pub enum CaptureResponseKind {
    Screenshot,
    UnretainedScreenshot,
    Stats(FrameStatsOptions),
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
    /// The countdown expired while a definition fetch was still running; a
    /// fresh grace countdown starts when the fetch finishes.
    pub awaiting_fetch: bool,
    /// Backstop for a fetch flag that never clears.
    pub fetch_deadline: Option<std::time::Instant>,
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

/// Tracks the timestamp of the error we last broadcast via SSE, so each
/// drained error is pushed to clients exactly once. Compared by equality,
/// not ordering: a full reload resets the clock, so a genuinely new
/// post-reload error can carry a smaller timestamp than the previous one.
#[derive(Resource, Default)]
pub struct LastBroadcastedErrorTimestamp {
    pub timestamp_secs: Option<f64>,
}

/// Create the mpsc channel pair
pub fn create_channel() -> (ControlSender, ControlReceiver) {
    let (tx, rx) = mpsc::unbounded_channel();
    (
        ControlSender {
            tx,
            exclusive_execution: SharedExclusiveExecution::default(),
        },
        ControlReceiver { rx },
    )
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
        required_render_epoch: None,
        with_gizmos,
        gizmo_restore: None,
        max_width: params.max_width.or(Some(DEFAULT_SCREENSHOT_MAX_WIDTH)),
        debug_camera,
        hide_ui: params.hide_ui,
        entity: params.entity,
        response_kind: CaptureResponseKind::Screenshot,
        extra_response: None,
    });
}

pub fn push_pending_stats(
    params: CaptureStatsParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    deferred: &mut Vec<PendingScreenshot>,
) {
    if params.max_width == Some(0) {
        let _ = response_tx.send(Err(ControlError::invalid_params(
            "max_width must be a positive integer",
        )));
        return;
    }
    let options = FrameStatsOptions {
        grid: params.grid,
        region: params.region,
        sample_points: params.sample_points,
    };
    if let Err(error) = validate_frame_stats_options(&options) {
        let _ = response_tx.send(Err(error));
        return;
    }
    let debug_camera = params.position.map(|position| DebugCameraRequest {
        position,
        look_at: params.look_at.unwrap_or([0.0, 0.0, 0.0]),
    });
    deferred.push(PendingScreenshot {
        response_tx,
        frames_remaining: params.delay_frames,
        required_render_epoch: None,
        with_gizmos: params.gizmos,
        gizmo_restore: None,
        max_width: params.max_width.or(Some(DEFAULT_SCREENSHOT_MAX_WIDTH)),
        debug_camera,
        hide_ui: params.hide_ui,
        entity: params.entity,
        response_kind: CaptureResponseKind::Stats(options),
        extra_response: None,
    });
}

pub fn push_pending_timeline(
    params: CaptureTimelineParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    // capture_count == 0 makes compute_schedule emit a single frame while
    // total_captures stays 0, so the completion check never matches and the
    // request hangs until timeout; above the max the contact sheet is unusable.
    if params.capture_count < 1 || params.capture_count > MAX_TIMELINE_CAPTURES {
        let _ = response_tx.send(Err(ControlError::invalid_params(format!(
            "capture_count must be between 1 and {MAX_TIMELINE_CAPTURES}"
        ))));
        return;
    }

    let mut schedule = compute_schedule(params.total_frames, params.capture_count);
    // Let one post-suppression frame reach both window and headless targets.
    if let Some(first) = schedule.front_mut() {
        *first += 1;
    }

    let debug_cleanup = if let Some(pos) = params.position {
        let debug_req = DebugCameraRequest {
            position: pos,
            look_at: params.look_at.unwrap_or([0.0, 0.0, 0.0]),
        };
        if let Some(first) = schedule.front_mut() {
            *first += 2;
        }
        match setup_debug_camera(world, &debug_req) {
            Ok(cleanup) => Some(cleanup),
            Err(error) => {
                let _ = response_tx.send(Err(error));
                return;
            }
        }
    } else {
        None
    };

    // Prepare visibility before any zero-delay capture can be scheduled.
    let (ui_restore, gizmo_restore) =
        prepare_capture_visibility(world, params.hide_ui, params.gizmos);
    let headless_sequence = headless_frame_sequence(world);

    let mut pending = world.get_resource_or_insert_with(PendingTimelines::default);
    let id = pending.next_id;
    pending.next_id += 1;
    pending.active.insert(
        id,
        ActiveTimeline {
            response_tx: Some(response_tx),
            // A contact sheet composes capture_count frames at this width, so
            // an unclamped default produces a payload many times the size of a
            // single screenshot. capture_turnaround already defaults the same
            // way; capture_screenshot clamps to DEFAULT_SCREENSHOT_MAX_WIDTH.
            max_width: Some(params.max_width.unwrap_or(DEFAULT_TIMELINE_MAX_WIDTH)),
            columns: params.columns,
            debug_cleanup,
            schedule,
            total_captures: params.capture_count,
            next_capture_index: 0,
            collected: Vec::new(),
            overlay_suppressed: true,
            hide_ui: params.hide_ui,
            with_gizmos: params.gizmos,
            ui_restore,
            gizmo_restore,
            headless_sequence,
            stall_frames: 0,
        },
    );
}

pub fn push_pending_reload(
    params: ReloadParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    // Surface validation errors (e.g. an out-of-range time_scale) directly.
    // Otherwise the caller would silently drop the bad param and still receive
    // a false "reload_completed" from the deferred response waiter below.
    if let Err(e) =
        handlers::reload::trigger_reload(world, params.mode, params.pause, params.time_scale)
    {
        let _ = response_tx.send(Err(e));
        return;
    }

    let mut pending = world.get_resource_or_insert_with(PendingReloadResponses::default);
    pending.pending.push(PendingReloadResponse {
        response_tx,
        frames_remaining: 5,
        mode: params.mode,
        awaiting_fetch: false,
        fetch_deadline: None,
    });
}

pub fn push_pending_reload_and_capture(
    params: ReloadAndCaptureParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    // Surface validation errors (e.g. an out-of-range time_scale) directly
    // instead of dropping the bad param and reporting a false success later.
    if let Err(e) =
        handlers::reload::trigger_reload(world, params.mode, params.pause, params.time_scale)
    {
        let _ = response_tx.send(Err(e));
        return;
    }

    let mut pending = world.get_resource_or_insert_with(PendingReloadAndCaptures::default);
    pending.pending.push(PendingReloadAndCapture {
        response_tx,
        mode: params.mode,
        reload_frames_remaining: 5,
        awaiting_fetch: false,
        screenshot_delay_frames: params.delay_frames.unwrap_or(30),
        max_width: params.max_width,
        position: params.position,
        look_at: params.look_at,
        hide_ui: params.hide_ui.unwrap_or(true),
    });
}

pub fn push_pending_turnaround(
    params: CaptureTurnaroundParams,
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    world: &mut World,
) {
    if params
        .distance
        .is_some_and(|distance| !distance.is_finite() || distance <= 0.0)
    {
        let _ = response_tx.send(Err(ControlError::invalid_params(
            "distance must be > 0 and finite",
        )));
        return;
    }

    let vc = params.view_count.unwrap_or(6);
    if !(1..=MAX_TURNAROUND_VIEWS).contains(&vc) {
        let _ = response_tx.send(Err(ControlError::invalid_params(format!(
            "view_count must be between 1 and {MAX_TURNAROUND_VIEWS}"
        ))));
        return;
    }

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
        view_staged: false,
        hide_ui: params.hide_ui.unwrap_or(true),
        ui_restore: None,
        overlay_suppressed: false,
        debug_cleanup: None,
        look_at: final_look_at,
        pending_screenshot_entity: None,
        headless_sequence: None,
        stall_frames: 0,
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
            required_render_epoch: None,
            with_gizmos: false,
            gizmo_restore: None,
            max_width: mw,
            debug_camera: dc,
            hide_ui: hu,
            entity: None,
            response_kind: CaptureResponseKind::UnretainedScreenshot,
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
                        let with_gizmos = p.gizmos;
                        push_pending_screenshot(
                            p,
                            with_gizmos,
                            request.response_tx,
                            &mut deferred_screenshots,
                        );
                    }
                    ControlOperation::CaptureStats(p) => {
                        push_pending_stats(p, request.response_tx, &mut deferred_screenshots);
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
        for screenshot in &mut deferred_screenshots {
            crate::handlers::screenshot::prepare_pending_screenshot_gizmos(world, screenshot);
        }
        let mut pending = world.get_resource_or_insert_with(PendingScreenshots::default);
        pending.pending.extend(deferred_screenshots);
    }

    // Put the receiver back
    world.insert_resource(receiver);

    // Broadcast new system errors via SSE
    if let Some(last_error) = world.get_resource::<pybevy_core::LastSystemError>()
        && let Some(ref msg) = last_error.error
    {
        let error_ts = last_error.timestamp_secs;
        let msg = msg.clone();
        let traceback = last_error.traceback.clone();

        let mut tracker = world.get_resource_or_insert_with(LastBroadcastedErrorTimestamp::default);

        if tracker.timestamp_secs != Some(error_ts) {
            tracker.timestamp_secs = Some(error_ts);

            // SSE broadcast (for clients that subscribe to /api/v1/sse)
            if let Some(broadcaster) = world.get_resource::<SseEventBroadcaster>() {
                broadcaster.send(&crate::protocol::SseEvent::Error {
                    message: msg,
                    traceback,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use bevy::{
        ecs::entity::Entity,
        gizmos::config::{DefaultGizmoConfigGroup, GizmoConfig, GizmoConfigStore},
        prelude::{Camera2d, Transform},
    };
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
    fn control_poll_suppresses_gizmos_before_queuing_capture() {
        let mut world = World::new();
        let mut store = GizmoConfigStore::default();
        store.insert(GizmoConfig::default(), DefaultGizmoConfigGroup);
        world.insert_resource(store);

        let (sender, receiver) = create_channel();
        world.insert_resource(receiver);
        let (response_tx, _response_rx) = oneshot::channel();
        sender
            .tx
            .send(ControlRequest {
                operation: ControlOperation::CaptureScreenshot(serde_json::from_str("{}").unwrap()),
                response_tx,
            })
            .unwrap();

        control_poll_system(&mut world);

        let store = world.resource::<GizmoConfigStore>();
        let (config, _) = store.config::<DefaultGizmoConfigGroup>();
        assert!(!config.enabled);
        let pending = world.resource::<PendingScreenshots>();
        assert_eq!(pending.pending[0].gizmo_restore, Some(true));
    }

    #[test]
    fn capture_screenshot_gizmos_flag_survives_direct_dispatch() {
        let mut world = World::new();
        let mut store = GizmoConfigStore::default();
        store.insert(GizmoConfig::default(), DefaultGizmoConfigGroup);
        world.insert_resource(store);

        let (sender, receiver) = create_channel();
        world.insert_resource(receiver);
        let (response_tx, _response_rx) = oneshot::channel();
        sender
            .tx
            .send(ControlRequest {
                operation: ControlOperation::CaptureScreenshot(
                    serde_json::from_str(r#"{"gizmos": true}"#).unwrap(),
                ),
                response_tx,
            })
            .unwrap();

        control_poll_system(&mut world);

        let pending = world.resource::<PendingScreenshots>();
        assert!(pending.pending[0].with_gizmos);
        let store = world.resource::<GizmoConfigStore>();
        let (config, _) = store.config::<DefaultGizmoConfigGroup>();
        assert!(config.enabled, "gizmos must stay enabled for the capture");
    }

    #[test]
    fn push_pending_stats_preserves_analysis_options() {
        let (tx, _rx) = oneshot::channel();
        let params: CaptureStatsParams = serde_json::from_str(
            r#"{"grid": 3, "region": [1, 2, 9, 6], "sample_points": [[2, 3]]}"#,
        )
        .unwrap();
        let mut deferred = Vec::new();
        push_pending_stats(params, tx, &mut deferred);

        assert_eq!(deferred[0].max_width, Some(DEFAULT_SCREENSHOT_MAX_WIDTH));
        let CaptureResponseKind::Stats(options) = &deferred[0].response_kind else {
            panic!("capture_stats must queue a stats response");
        };
        assert_eq!(options.grid, 3);
        assert_eq!(options.region, Some([1, 2, 9, 6]));
        assert_eq!(options.sample_points, Some(vec![[2, 3]]));
    }

    #[test]
    fn push_pending_stats_rejects_invalid_options_before_capture() {
        let (tx, rx) = oneshot::channel();
        let params: CaptureStatsParams = serde_json::from_str(r#"{"grid": 0}"#).unwrap();
        let mut deferred = Vec::new();
        push_pending_stats(params, tx, &mut deferred);

        assert!(deferred.is_empty());
        let error = rx.blocking_recv().unwrap().unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(error.message.contains("grid"));
    }

    #[test]
    fn push_pending_depth_returns_structured_coordinate_error() {
        let (tx, rx) = oneshot::channel();
        let params: CaptureDepthParams = serde_json::from_str(
            r#"{"sample_points": [[99999, -50], [10, 10]], "include_rgb": false}"#,
        )
        .unwrap();
        let mut deferred = Vec::new();
        let mut world = World::new();

        push_pending_depth(params, tx, &mut deferred, &mut world);

        assert!(deferred.is_empty());
        let error = rx.blocking_recv().unwrap().unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert_eq!(
            error.message,
            "sample_points[0] must be within [0, 800) on both axes (got [99999, -50])"
        );
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
    fn list_systems_defaults_to_scene_only() {
        let operation: ControlOperation =
            serde_json::from_str(r#"{"tool":"get_system_list"}"#).unwrap();
        assert!(matches!(
            operation,
            ControlOperation::ListSystems {
                include_internal: false
            }
        ));
    }

    #[test]
    fn list_systems_accepts_include_internal() {
        let operation: ControlOperation =
            serde_json::from_str(r#"{"tool":"get_system_list","include_internal":true}"#).unwrap();
        assert!(matches!(
            operation,
            ControlOperation::ListSystems {
                include_internal: true
            }
        ));
    }

    #[test]
    fn exclusive_execution_guard_is_app_local_and_raii_scoped() {
        let (sender_a, _receiver_a) = create_channel();
        let (sender_b, _receiver_b) = create_channel();
        let state_a = sender_a.exclusive_execution();
        let state_b = sender_b.exclusive_execution();

        assert!(!state_a.is_active());
        let guard = state_a.try_enter().expect("first entry should succeed");
        assert!(state_a.is_active());
        assert!(state_a.try_enter().is_none());
        assert!(!state_b.is_active());

        drop(guard);
        assert!(!state_a.is_active());
        assert!(state_a.try_enter().is_some());
    }

    #[test]
    fn exclusive_execution_guard_clears_on_unwind() {
        let (sender, _receiver) = create_channel();
        let state = sender.exclusive_execution();
        let unwind_state = state.clone();

        let result = std::panic::catch_unwind(move || {
            let _guard = unwind_state.try_enter().expect("entry should succeed");
            panic!("test unwind");
        });

        assert!(result.is_err());
        assert!(!state.is_active());
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

    fn timeline_params(capture_count: u32) -> CaptureTimelineParams {
        CaptureTimelineParams {
            total_frames: 6,
            capture_count,
            max_width: None,
            columns: 3,
            position: None,
            look_at: None,
            hide_ui: true,
            gizmos: false,
        }
    }

    #[test]
    fn push_pending_timeline_rejects_out_of_range_capture_count() {
        // Regression: capture_count == 0 made compute_schedule emit one frame
        // while total_captures stayed 0, so the completion check never matched
        // and the request hung until the 120s timeout. Out-of-range counts must
        // be rejected up front with no timeline queued.
        for bad in [0, MAX_TIMELINE_CAPTURES + 1] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            push_pending_timeline(timeline_params(bad), tx, &mut world);
            let result = rx.try_recv().expect("caller should receive a response");
            let err = result.expect_err("out-of-range capture_count must be an error");
            assert!(err.message.contains("between 1 and"));
            let queued = world
                .get_resource::<PendingTimelines>()
                .map(|p| p.active.len())
                .unwrap_or(0);
            assert_eq!(queued, 0);
        }
    }

    #[test]
    fn push_pending_timeline_accepts_valid_capture_count() {
        let mut world = World::new();
        let (tx, mut rx) = oneshot::channel();
        let mut params = timeline_params(6);
        params.hide_ui = false;
        params.gizmos = true;
        push_pending_timeline(params, tx, &mut world);
        assert!(
            rx.try_recv().is_err(),
            "response is deferred, not immediate"
        );
        let pending = world.resource::<PendingTimelines>();
        assert_eq!(pending.active.len(), 1);
        let timeline = pending.active.values().next().unwrap();
        assert!(!timeline.hide_ui);
        assert!(timeline.with_gizmos);
        assert!(timeline.overlay_suppressed);
        assert_eq!(timeline.schedule.front(), Some(&1));
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
    }

    #[test]
    fn push_pending_timeline_rejects_camera2d_position_override() {
        let mut world = World::new();
        world.spawn(Camera2d::default());
        let (tx, mut rx) = oneshot::channel();
        let mut params = timeline_params(6);
        params.position = Some([0.0, 0.0, 500.0]);

        push_pending_timeline(params, tx, &mut world);

        let error = rx.try_recv().unwrap().unwrap_err();
        assert!(error.message.contains("require a Camera3d"));
        assert!(!world.contains_resource::<PendingTimelines>());
    }

    #[test]
    fn push_pending_timeline_clamps_the_default_sheet_width() {
        // A contact sheet composes capture_count frames, so an unclamped
        // default produces a payload many times a single screenshot and can
        // exceed the transport's inline image bound. capture_turnaround
        // already defaults the same way.
        let mut world = World::new();
        let (tx, _rx) = oneshot::channel();
        push_pending_timeline(timeline_params(6), tx, &mut world);
        let width = world
            .get_resource::<PendingTimelines>()
            .and_then(|pending| pending.active.values().next().map(|t| t.max_width))
            .expect("timeline was queued");
        assert_eq!(width, Some(DEFAULT_TIMELINE_MAX_WIDTH));
    }

    #[test]
    fn push_pending_timeline_keeps_an_explicit_sheet_width() {
        let mut world = World::new();
        let (tx, _rx) = oneshot::channel();
        let mut params = timeline_params(6);
        params.max_width = Some(2400);
        push_pending_timeline(params, tx, &mut world);
        let width = world
            .get_resource::<PendingTimelines>()
            .and_then(|pending| pending.active.values().next().map(|t| t.max_width))
            .expect("timeline was queued");
        assert_eq!(width, Some(2400));
    }

    fn turnaround_params(distance: Option<f32>) -> CaptureTurnaroundParams {
        CaptureTurnaroundParams {
            look_at: None,
            distance,
            elevation: None,
            view_count: None,
            include_top: None,
            columns: None,
            max_width: None,
            hide_ui: None,
        }
    }

    #[test]
    fn push_pending_turnaround_rejects_invalid_distance() {
        for distance in [0.0, -1.0, f32::NEG_INFINITY, f32::INFINITY, f32::NAN] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            push_pending_turnaround(turnaround_params(Some(distance)), tx, &mut world);

            let error = rx
                .try_recv()
                .expect("caller should receive a response")
                .expect_err("invalid distance must be an error");
            assert_eq!(error.code, ErrorCode::InvalidParams);
            assert_eq!(error.message, "distance must be > 0 and finite");
            assert!(!world.contains_resource::<PendingTurnarounds>());
        }
    }

    #[test]
    fn push_pending_turnaround_accepts_automatic_and_positive_distance() {
        for distance in [None, Some(1.0)] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            push_pending_turnaround(turnaround_params(distance), tx, &mut world);

            assert!(rx.try_recv().is_err(), "response should remain deferred");
            assert_eq!(world.resource::<PendingTurnarounds>().active.len(), 1);
        }
    }

    #[test]
    fn push_pending_turnaround_rejects_out_of_range_view_count() {
        for view_count in [0, MAX_TURNAROUND_VIEWS + 1] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            let mut params = turnaround_params(None);
            params.view_count = Some(view_count);
            push_pending_turnaround(params, tx, &mut world);

            let error = rx
                .try_recv()
                .expect("caller should receive a response")
                .expect_err("out-of-range view count must be an error");
            assert_eq!(error.code, ErrorCode::InvalidParams);
            assert_eq!(error.message, "view_count must be between 1 and 20");
            assert!(!world.contains_resource::<PendingTurnarounds>());
        }
    }

    #[test]
    fn push_pending_turnaround_accepts_boundary_view_counts() {
        for view_count in [1, MAX_TURNAROUND_VIEWS] {
            let mut world = World::new();
            let (tx, mut rx) = oneshot::channel();
            let mut params = turnaround_params(None);
            params.view_count = Some(view_count);
            push_pending_turnaround(params, tx, &mut world);

            assert!(rx.try_recv().is_err(), "response should remain deferred");
            assert_eq!(world.resource::<PendingTurnarounds>().active.len(), 1);
        }
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
