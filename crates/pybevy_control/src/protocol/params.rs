use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

use super::EntityRef;
use crate::capture::{
    analysis::DEFAULT_COMPARE_EPSILON,
    depth::{MAX_DEPTH_GRID_DENSITY, MAX_DEPTH_SAMPLE_POINTS},
    screenshot::MAX_CAPTURE_DELAY_FRAMES,
    timeline::{MAX_TIMELINE_CAPTURES, MAX_TIMELINE_FRAMES},
};

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
    /// Packed entity ID from an MCP response, or Name
    pub entity: EntityRef,
    /// Component name (e.g. 'Transform', 'PointLight')
    pub component: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetResourceParams {
    /// Exact resource name from get_registry.resource_names (e.g. 'ClearColor', '_TimeVirtual', 'State[game.Phase]')
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
pub(crate) fn json_object_array_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "array",
        "items": { "type": "object" }
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetComponentParams {
    /// Packed entity ID from an MCP response, or Name
    pub entity: EntityRef,
    /// Component name
    pub component: String,
    /// Fields to update. Enum values use {variant: name, ...}; named payload fields keep their native names and a single tuple payload uses value. A one-key {Variant: payload} shorthand may switch the active variant. Top-level enum component new_values contain the complete stored payload, and writing only the active variant preserves it. Read and new_values objects can be replayed unchanged. Vec3 accepts [x, y, z] or {x, y, z}; coordinate objects require all three finite numeric fields. Asset-handle components accept only the engine-issued fields.handle.asset_ref object returned by get_component; references are typed, app-session scoped, and valid while the named source still owns the same live asset handle.
    #[schemars(schema_with = "json_object_schema")]
    pub fields: serde_json::Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveComponentParams {
    /// Packed entity ID from an MCP response, or Name
    pub entity: EntityRef,
    /// Component to remove
    pub component: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetResourceParams {
    /// Resource type name
    pub resource_type: String,
    /// Fields to update as JSON. Enum values use {variant: name, ...}; named payload fields keep their native names and a single tuple payload uses value. Read and new_values enum objects can be replayed unchanged. Custom resources must declare annotated fields, typically with `@resource` above `@dataclass`; attributes created only in `__init__` are not editable here.
    #[schemars(schema_with = "json_object_schema")]
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeekTimeParams {
    /// Target elapsed time in seconds, at most 1e12
    #[schemars(range(min = 0.0, max = 1e12))]
    pub seconds: f64,
    /// Pause after seeking (default true). `false` does not resume a paused scene.
    #[serde(default = "default_true")]
    pub pause: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureScreenshotParams {
    /// Optional entity Name or packed ID from an MCP response to isolate, including its descendants.
    pub entity: Option<EntityRef>,
    /// Frames to wait before capture (default 2)
    #[serde(default = "default_2", deserialize_with = "capture_delay")]
    #[schemars(range(max = MAX_CAPTURE_DELAY_FRAMES))]
    pub delay_frames: u32,
    /// Max image width in pixels (default 768). Use 1280 for detail.
    #[serde(default, deserialize_with = "capture_width")]
    pub max_width: Option<u32>,
    /// Camera position [x, y, z]. If set, temporarily re-poses a Camera3d and restores it after capture; creates one only when the scene has no cameras, rendering into a 768x432 image when there is no window.
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
    /// Optional entity Name or packed ID from an MCP response to isolate, including its descendants.
    pub entity: Option<EntityRef>,
    /// Divide the region into an NxN grid (default 1, max 16). Payload grows
    /// quadratically: about 4 KB at 4 and 57 KB at 16.
    #[serde(default = "default_1")]
    #[schemars(range(min = 1, max = 16))]
    pub grid: u32,
    /// Optional [x, y, width, height] sub-rectangle in resized output pixels.
    pub region: Option<[i64; 4]>,
    /// Optional [x, y] points in resized-capture pixels. Max 256.
    #[schemars(extend("maxItems" = 256))]
    pub sample_points: Option<Vec<[i64; 2]>>,
    /// Frames to wait before capture (default 2).
    #[serde(default = "default_2", deserialize_with = "capture_delay")]
    #[schemars(range(max = MAX_CAPTURE_DELAY_FRAMES))]
    pub delay_frames: u32,
    /// Maximum analyzed width in pixels (default 768).
    #[schemars(range(min = 1))]
    #[serde(default, deserialize_with = "capture_width")]
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
    #[schemars(range(max = MAX_TIMELINE_FRAMES))]
    pub total_frames: u32,
    /// Number of captures (max 20)
    #[serde(default = "default_6")]
    #[schemars(range(min = 1, max = MAX_TIMELINE_CAPTURES))]
    pub capture_count: u32,
    /// Max composite width in pixels (default 1200)
    #[schemars(extend("default" = 1200))]
    #[schemars(range(min = 1))]
    #[serde(default, deserialize_with = "capture_width")]
    pub max_width: Option<u32>,
    /// Grid columns (default 3, max 20)
    #[serde(default = "default_3")]
    #[schemars(range(min = 1, max = MAX_TIMELINE_CAPTURES))]
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
    /// Camera elevation in degrees (default 25); must be finite.
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
    #[schemars(range(min = 1))]
    #[serde(default, deserialize_with = "capture_width")]
    pub max_width: Option<u32>,
    /// Hide UI during capture (default true)
    #[schemars(extend("default" = true))]
    pub hide_ui: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureDepthParams {
    /// Override position for the selected active Camera3d [x, y, z].
    pub position: Option<[f32; 3]>,
    /// Override look-at point for the selected active Camera3d [x, y, z].
    pub look_at: Option<[f32; 3]>,
    /// Normalized ray-cast coordinates on a fixed 800x800 scale, using the
    /// selected Camera3d projection and viewport. Grid if omitted. Max 1024.
    #[schemars(extend("maxItems" = MAX_DEPTH_SAMPLE_POINTS))]
    pub sample_points: Option<Vec<[i64; 2]>>,
    /// Auto-generate NxN sample grid (default 8 if no sample_points, max 32)
    #[schemars(extend("default" = 8))]
    #[schemars(range(min = 1, max = MAX_DEPTH_GRID_DENSITY))]
    pub grid_density: Option<u32>,
    /// Include RGB screenshot (default true)
    #[schemars(extend("default" = true))]
    pub include_rgb: Option<bool>,
    /// Frames to wait before capture (default 2)
    #[schemars(extend("default" = 2))]
    #[schemars(range(max = MAX_CAPTURE_DELAY_FRAMES))]
    pub delay_frames: Option<u32>,
    /// Hide UI during capture (default true)
    #[schemars(extend("default" = true))]
    pub hide_ui: Option<bool>,
    /// Max screenshot width (default 768)
    #[schemars(extend("default" = 768))]
    #[schemars(range(min = 1))]
    #[serde(default, deserialize_with = "capture_width")]
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
    #[schemars(range(max = MAX_CAPTURE_DELAY_FRAMES))]
    pub delay_frames: Option<u32>,
    /// Max screenshot width (default 768)
    #[schemars(range(min = 1))]
    #[serde(default, deserialize_with = "capture_width")]
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
    /// First entity (packed ID from an MCP response, or Name)
    pub entity_a: EntityRef,
    /// Second entity (packed ID from an MCP response, or Name)
    pub entity_b: EntityRef,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuerySpatialNeighborhoodParams {
    /// Center entity (packed ID from an MCP response, or Name)
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
    /// Entity to check (packed ID from an MCP response, or Name)
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
    /// Minimum overlap depth to report (default 0.001), the same threshold `check_all_overlaps` uses. Faces placed exactly flush penetrate by 0.0 and are a touch, not an overlap.
    #[schemars(extend("default" = 0.001))]
    pub min_penetration: Option<f32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckAllOverlapsParams {
    /// Minimum overlap depth to report (default 0.001)
    #[schemars(extend("default" = 0.001))]
    pub min_penetration: Option<f32>,
    /// Max entries in each returned list - overlapping pairs, floating entities and sunken entities (default 100). Every `*_count` is the scene's total whatever this is set to, and the matching `*_truncated` flag says whether the list beside it was cut.
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
    /// Packed entity ID from an MCP response, or Name
    pub entity: EntityRef,
    /// Handle component: MeshMaterial3d, Mesh3d, MeshMaterial2d, AudioPlayer
    pub component: String,
    /// Asset type: StandardMaterial, Mesh, ColorMaterial, AudioSource
    pub asset_type: String,
    /// Fields to update on the asset. Enum values use {variant: name, ...}; named payload fields keep their native names and a single tuple payload uses value. Color and LinearRgba fields accept [r, g, b, a]; LinearRgba also accepts {red, green, blue, alpha}.
    #[schemars(schema_with = "json_object_schema")]
    pub fields: serde_json::Value,
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

fn capture_delay<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let frames = i64::deserialize(deserializer)?;
    if frames < 0 {
        return Err(D::Error::custom(
            "delay_frames must be a non-negative integer",
        ));
    }
    u32::try_from(frames).map_err(|_| D::Error::custom("delay_frames must not exceed 4294967295"))
}

fn capture_width<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u32>, D::Error> {
    let Some(width) = Option::<i64>::deserialize(deserializer)? else {
        return Ok(None);
    };
    if width <= 0 {
        return Err(D::Error::custom("max_width must be a positive integer"));
    }
    u32::try_from(width)
        .map(Some)
        .map_err(|_| D::Error::custom("max_width must not exceed 4294967295"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn capture_params_default_and_reject_out_of_range_integers() {
        let params: CaptureScreenshotParams =
            serde_json::from_value(serde_json::json!({"position": [1.0, 2.0, 3.0]})).unwrap();
        assert_eq!((params.delay_frames, params.max_width), (2, None));
        assert!(params.hide_ui);
        for (body, message) in [
            (
                serde_json::json!({"delay_frames": -5}),
                "delay_frames must be a non-negative integer",
            ),
            (
                serde_json::json!({"delay_frames": 4_294_967_296_i64}),
                "delay_frames must not exceed 4294967295",
            ),
            (
                serde_json::json!({"max_width": 0}),
                "max_width must be a positive integer",
            ),
        ] {
            let screenshot = serde_json::from_value::<CaptureScreenshotParams>(body.clone());
            let stats = serde_json::from_value::<CaptureStatsParams>(body);
            assert_eq!(screenshot.unwrap_err().to_string(), message);
            assert_eq!(stats.unwrap_err().to_string(), message);
        }
    }

    #[test]
    fn reload_mode_deserializes_snake_case_and_rejects_unknown() {
        assert_eq!(
            serde_json::from_value::<ReloadMode>(json!("partial")).unwrap(),
            ReloadMode::Partial
        );
        assert!(serde_json::from_value::<ReloadMode>(json!("unknown_mode")).is_err());
    }
}
