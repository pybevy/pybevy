use schemars::JsonSchema;
use serde::Deserialize;

use super::{
    CaptureDepthParams, CaptureScreenshotParams, CaptureStatsParams, CaptureTimelineParams,
    CaptureTurnaroundParams, CheckAllOverlapsParams, CheckOverlapsParams, CompareFramesParams,
    EntityRef, GetComponentParams, GetResourceParams, QueryEntitiesParams,
    QuerySpatialNeighborhoodParams, QuerySpatialParams, ReloadAndCaptureParams, ReloadParams,
    RemoveComponentParams, SeekTimeParams, SetAssetParams, SetComponentParams, SetResourceParams,
    params::{json_object_array_schema, json_object_schema},
};
use crate::schedule::ScheduleRequest;

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
        /// Packed entity ID from an MCP response, or Name
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
    /// Get field names, types, defaults, and a JSON spawn example for a World-registered component.
    GetComponentSchema {
        /// Component name (e.g. 'Transform')
        name: String,
    },
    /// Get live field values for a specific component on an entity.
    GetComponent(GetComponentParams),
    /// Get the presence and live field values of a World-registered resource.
    GetResource(GetResourceParams),
    /// Query entities by registered With/Without component filters. Returns at most 100 records by default; use limit up to 1000 and inspect total_count/truncated.
    QueryEntities(QueryEntitiesParams),
    /// Get a grouped entity inventory with counts and a packed representative entity ID for every group. Custom-component labels prefer the least prevalent live scene component, then lexical order. Faster than query_entities for understanding scene composition.
    GetSceneSummary,
    /// Get the axis-aligned bounding box (AABB) of an entity, both local and world-space. Requires entity to have a mesh.
    GetBoundingBox {
        /// Packed entity ID from an MCP response, or Name
        entity: EntityRef,
    },
    /// Show bridge registry state, exact addressable resource names, entity count, and component detection status.
    GetRegistry,

    /// Capture a screenshot. Default 768px wide. UI elements are hidden by default. Use position/look_at to temporarily re-pose a 3D camera; its authored state is restored after capture.
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
    /// Capture RGB and ray-AABB depth samples from one camera context. Selects the highest-order active Camera3d, retains its identity across the delay, and reports its viewport and exact-target pass provenance.
    #[schemars(extend("x-feature-gate" = "screenshot"))]
    CaptureDepth(CaptureDepthParams),

    /// Hot-reload the scene (full or partial). Waits for reload to complete before responding. Errors include a traceback and may include an advisory, message-specific hint.
    Reload(ReloadParams),
    /// Primary iteration tool. Reload and capture in one round-trip. Hot-reloads the scene, waits for completion, checks for errors, then captures a screenshot. Errors include a traceback and may include an advisory, message-specific hint.
    #[schemars(extend("x-feature-gate" = "screenshot"))]
    ReloadAndCapture(ReloadAndCaptureParams),
    /// Get current reload generation, mode, and enabled state.
    GetReloadStatus,
    /// Get the last Python system error traceback, plus `wgpu_validation` (null, or the count, first and last message with pipeline label, and `last_seen_secs_ago`) for GPU validation errors, such as a failed prepass pipeline that renders the whole frame black. A successful reload clears both.
    GetLastError,

    /// Spawn a new entity with World-registered components specified as JSON.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    SpawnEntity {
        /// Component name -> field values (e.g. {"Transform": {"translation": [0, 5, 0]}}). Enum values use {variant: name, ...}; named payload fields keep their native names and a single tuple payload uses value. To copy an existing asset handle, replay a live component's engine-issued fields.handle.asset_ref object; raw asset IDs are not accepted.
        #[schemars(schema_with = "json_object_schema")]
        components: serde_json::Value,
    },
    /// Remove an entity by ID or Name.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    DespawnEntity {
        /// Packed entity ID from an MCP response, or Name
        entity: EntityRef,
    },
    /// Update specific fields on a World-registered component without replacing it. If the entity lacks the component, it is inserted from the given fields and the response reports `inserted: true`.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    SetComponent(SetComponentParams),
    /// Remove a component from an entity.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    RemoveComponent(RemoveComponentParams),
    /// Insert or update a World-registered resource on a running scene. Register an unused custom class first with world.register_resource(Type) in run_code, or insert an instance in scene code. The response returns the type under `resource` and whether a new value was `inserted`. Custom resources must declare annotated fields, typically with `@resource` above `@dataclass`; attributes created only in `__init__` are not editable here.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    SetResource(SetResourceParams),
    /// Remove a resource from the world.
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    RemoveResource {
        /// Resource type name
        resource_type: String,
    },
    /// Execute multiple mutation operations in a single round-trip. Each operation runs independently - failures don't abort the batch. Actions: set_component, spawn, despawn, remove_component. Returns `total` and the three counts that partition it: `succeeded`, `failed` (the op was rejected outright), and `partial` (the op applied some fields and reported the rest in its own `errors` array).
    #[schemars(extend("x-feature-gate" = "manipulation"))]
    Batch {
        /// Array of flat operation objects. Exact shapes: set_component uses {action, entity, component, fields}; spawn uses {action, components}; despawn uses {action, entity}; remove_component uses {action, entity, component}. Entity accepts a packed integer ID returned by MCP or a string Name; a Python Entity.index() is not an ID. Unsupported keys are errors.
        #[schemars(schema_with = "json_object_array_schema")]
        operations: Vec<serde_json::Value>,
    },
    /// Update asset properties (material color, mesh settings) live without code reload. The entity only selects the asset through its handle, so every entity sharing that handle changes with it. Color and LinearRgba fields accept [r, g, b, a]; LinearRgba arrays are linear and may contain HDR values above 1.
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

    /// Spatial query between two entities: distance, direction, AABB overlap. Direction words are world-axis labels (-Z forward, +Z behind), not either entity's local orientation. For finding all entities within a radius, use query_spatial_neighborhood instead.
    QuerySpatial(QuerySpatialParams),
    /// Find all entities within radius of a center entity. Direction words are world-axis labels (-Z forward, +Z behind), not the center entity's local orientation.
    QuerySpatialNeighborhood(QuerySpatialNeighborhoodParams),
    /// Detect AABB overlaps for a single entity against all others.
    CheckOverlaps(CheckOverlapsParams),
    /// Detect all AABB overlaps scene-wide.
    CheckAllOverlaps(CheckAllOverlapsParams),

    /// Run arbitrary Python code with World context. Returns captured stdout and stderr.
    #[schemars(extend("x-feature-gate" = "execute_python"))]
    RunCode {
        /// Python code to execute
        code: String,
    },
    /// Get full diagnostics: FPS, frame times, CPU/GPU/RAM/VRAM usage, entity/asset counts, system profiling times, build identity. fps_average is frames over summed real frame time (last 60 frames); fps_current is 1 / median frame time of the last 5 frames; frame_time_p50_ms/frame_time_p95_ms cover the last 60 frames. A full reload restarts the window.
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

macro_rules! operation_tags {
    ($($variant:ident => $name:literal,)*) => {
        /// Payload-free identity of a [`ControlOperation`] variant.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum OperationTag {
            $($variant,)*
        }

        impl OperationTag {
            pub(crate) const ALL: &[Self] = &[$(Self::$variant,)*];

            /// The serde `tool` name of the variant.
            pub(crate) fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }
        }

        impl ControlOperation {
            pub(crate) fn tag(&self) -> OperationTag {
                match self {
                    $(Self::$variant { .. } => OperationTag::$variant,)*
                }
            }
        }
    };
}

operation_tags! {
    ListEntities => "list_entities",
    GetEntity => "get_entity",
    ListResources => "list_resources",
    ListSystems => "get_system_list",
    GetComponentSchema => "get_component_schema",
    GetComponent => "get_component",
    GetResource => "get_resource",
    QueryEntities => "query_entities",
    GetSceneSummary => "get_scene_summary",
    GetBoundingBox => "get_bounding_box",
    GetRegistry => "get_registry",
    CaptureScreenshot => "capture_screenshot",
    CaptureStats => "capture_stats",
    CompareFrames => "compare_frames",
    CaptureWithGizmos => "capture_with_gizmos",
    CaptureTimeline => "capture_timeline",
    CaptureTurnaround => "capture_turnaround",
    CaptureDepth => "capture_depth",
    Reload => "reload",
    ReloadAndCapture => "reload_and_capture",
    GetReloadStatus => "get_reload_status",
    GetLastError => "get_last_error",
    SpawnEntity => "spawn_entity",
    DespawnEntity => "despawn_entity",
    SetComponent => "set_component",
    RemoveComponent => "remove_component",
    SetResource => "set_resource",
    RemoveResource => "remove_resource",
    Batch => "batch",
    SetAsset => "set_asset",
    PauseTime => "pause_time",
    ResumeTime => "resume_time",
    SetTimeScale => "set_time_scale",
    GetTimeStatus => "get_time_status",
    SeekTime => "seek_time",
    QuerySpatial => "query_spatial",
    QuerySpatialNeighborhood => "query_spatial_neighborhood",
    CheckOverlaps => "check_overlaps",
    CheckAllOverlaps => "check_all_overlaps",
    RunCode => "run_code",
    GetPerformance => "get_performance",
    ScheduleActions => "schedule_actions",
    GetConfig => "get_config",
    ListConfigs => "list_configs",
}

/// A `McpPlugin` switch that disables a group of control tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FeatureGate {
    Screenshot,
    Manipulation,
    ExecutePython,
}

/// How the engine services an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationKind {
    /// Answered in the batched dispatch of the frame that received it.
    Sync,
    /// Queued and answered by a later-frame capture or reload system.
    Deferred,
    /// Stored as an active schedule.
    Schedule,
}

/// Per-operation facts shared by dispatch, gating, scheduling and delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OperationSpec {
    pub kind: OperationKind,
    pub gate: Option<FeatureGate>,
    /// Accepted as a `schedule_actions` action.
    pub schedulable: bool,
    /// The response can carry images prepared for MCP delivery.
    pub image_delivery: bool,
    /// A scheduled call propagates transforms afterwards. Despawn needs no
    /// propagation because it also despawns the children.
    pub mutates_transforms: bool,
    /// A scheduled call requests a render frame before later captures.
    pub mutates_render: bool,
    /// Rejected while Python holds live asset access.
    pub structural_guard: bool,
}

impl OperationSpec {
    const READ: Self = Self {
        kind: OperationKind::Sync,
        gate: None,
        schedulable: true,
        image_delivery: false,
        mutates_transforms: false,
        mutates_render: false,
        structural_guard: false,
    };
    const CAPTURE: Self = Self {
        kind: OperationKind::Deferred,
        gate: Some(FeatureGate::Screenshot),
        ..Self::READ
    };
    const MUTATION: Self = Self {
        gate: Some(FeatureGate::Manipulation),
        mutates_render: true,
        structural_guard: true,
        ..Self::READ
    };
}

impl OperationTag {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|tag| tag.name() == name)
    }

    pub(crate) fn spec(self) -> OperationSpec {
        use OperationSpec as S;
        match self {
            Self::ListEntities
            | Self::GetEntity
            | Self::ListResources
            | Self::ListSystems
            | Self::GetComponentSchema
            | Self::GetComponent
            | Self::GetResource
            | Self::QueryEntities
            | Self::GetSceneSummary
            | Self::GetBoundingBox
            | Self::GetRegistry
            | Self::GetReloadStatus
            | Self::GetLastError
            | Self::PauseTime
            | Self::ResumeTime
            | Self::SetTimeScale
            | Self::GetTimeStatus
            | Self::SeekTime
            | Self::QuerySpatial
            | Self::QuerySpatialNeighborhood
            | Self::CheckOverlaps
            | Self::CheckAllOverlaps
            | Self::GetPerformance
            | Self::GetConfig
            | Self::ListConfigs => S::READ,
            Self::CaptureScreenshot
            | Self::CaptureWithGizmos
            | Self::CaptureTimeline
            | Self::CaptureTurnaround
            | Self::CaptureDepth => S {
                image_delivery: true,
                ..S::CAPTURE
            },
            Self::CaptureStats => S::CAPTURE,
            Self::CompareFrames => S {
                gate: Some(FeatureGate::Screenshot),
                ..S::READ
            },
            // A schedule waiting on a reload would block the app loop the
            // reload must drop and re-enter.
            Self::Reload => S {
                kind: OperationKind::Deferred,
                schedulable: false,
                ..S::READ
            },
            Self::ReloadAndCapture => S {
                schedulable: false,
                image_delivery: true,
                ..S::CAPTURE
            },
            Self::SpawnEntity | Self::SetComponent | Self::RemoveComponent | Self::Batch => S {
                mutates_transforms: true,
                ..S::MUTATION
            },
            Self::DespawnEntity | Self::SetResource | Self::RemoveResource => S::MUTATION,
            Self::SetAsset => S {
                structural_guard: false,
                ..S::MUTATION
            },
            Self::RunCode => S {
                gate: Some(FeatureGate::ExecutePython),
                ..S::READ
            },
            Self::ScheduleActions => S {
                kind: OperationKind::Schedule,
                schedulable: false,
                image_delivery: true,
                ..S::READ
            },
        }
    }
}

impl ControlOperation {
    pub(crate) fn spec(&self) -> OperationSpec {
        self.tag().spec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::tools::list_tools;

    fn names_where(predicate: impl Fn(OperationSpec) -> bool) -> Vec<&'static str> {
        OperationTag::ALL
            .iter()
            .filter(|tag| predicate(tag.spec()))
            .map(|tag| tag.name())
            .collect()
    }

    #[test]
    fn tags_cover_every_schema_variant() {
        let schema = serde_json::to_value(schemars::schema_for!(ControlOperation)).unwrap();
        let variants = schema["oneOf"].as_array().unwrap().len();
        assert_eq!(OperationTag::ALL.len(), variants);
        for tool in list_tools() {
            let name = tool["name"].as_str().unwrap();
            assert!(OperationTag::from_name(name).is_some(), "{name}");
        }
    }

    #[test]
    fn tag_names_are_the_serde_tool_names() {
        for tag in OperationTag::ALL {
            let parsed =
                serde_json::from_value::<ControlOperation>(serde_json::json!({"tool": tag.name()}));
            match parsed {
                Ok(operation) => assert_eq!(operation.tag(), *tag),
                Err(error) => assert!(
                    !error.to_string().contains("unknown variant"),
                    "{}: {error}",
                    tag.name()
                ),
            }
        }
    }

    #[test]
    fn deferred_operations_are_captures_and_reloads() {
        assert_eq!(
            names_where(|spec| spec.kind == OperationKind::Deferred),
            [
                "capture_screenshot",
                "capture_stats",
                "capture_with_gizmos",
                "capture_timeline",
                "capture_turnaround",
                "capture_depth",
                "reload",
                "reload_and_capture",
            ]
        );
        assert_eq!(
            names_where(|spec| spec.kind == OperationKind::Schedule),
            ["schedule_actions"]
        );
    }

    #[test]
    fn schedules_reject_reloads_and_nested_schedules() {
        assert_eq!(
            names_where(|spec| !spec.schedulable),
            ["reload", "reload_and_capture", "schedule_actions"]
        );
    }

    #[test]
    fn mutation_flags_name_the_mutating_operations() {
        assert_eq!(
            names_where(|spec| spec.mutates_transforms),
            ["spawn_entity", "set_component", "remove_component", "batch"]
        );
        assert_eq!(
            names_where(|spec| spec.mutates_render),
            [
                "spawn_entity",
                "despawn_entity",
                "set_component",
                "remove_component",
                "set_resource",
                "remove_resource",
                "batch",
                "set_asset",
            ]
        );
        assert_eq!(
            names_where(|spec| spec.structural_guard),
            [
                "spawn_entity",
                "despawn_entity",
                "set_component",
                "remove_component",
                "set_resource",
                "remove_resource",
                "batch",
            ]
        );
    }

    #[test]
    fn image_delivery_names_the_image_responses() {
        assert_eq!(
            names_where(|spec| spec.image_delivery),
            [
                "capture_screenshot",
                "capture_with_gizmos",
                "capture_timeline",
                "capture_turnaround",
                "capture_depth",
                "reload_and_capture",
                "schedule_actions",
            ]
        );
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
