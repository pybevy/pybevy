use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query as AxumQuery, State},
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use tokio::sync::oneshot;
use tokio_stream::StreamExt;

use crate::{
    discovery::ApiIndex,
    engine::channel::{ControlRequest, ControlSender},
    plugin::ControlConfig,
    protocol::{
        CaptureDepthParams, CaptureScreenshotParams, CaptureStatsParams, CaptureTimelineParams,
        CaptureTurnaroundParams, CheckAllOverlapsParams, CheckOverlapsParams, CompareFramesParams,
        ControlOperation, EntityRef, ErrorCode, GetComponentParams, GetResourceParams,
        QueryEntitiesParams, QuerySpatialNeighborhoodParams, QuerySpatialParams,
        ReloadAndCaptureParams, ReloadParams, RemoveComponentParams, SeekTimeParams,
        SetAssetParams, SetComponentParams, SetResourceParams, default_max_float_gap,
    },
    schedule::{ScheduleMode, ScheduleRequest, SharedScheduleRegistry, validate_schedule},
    transport::{
        image_preview::{
            MCP_IMAGE_DELIVERY_HEADER, MCP_IMAGE_DELIVERY_VALUE, prepare_mcp_response_images,
        },
        sse::SseEventBroadcaster,
    },
};

/// Shared state for the Axum server
#[derive(Clone)]
pub struct AppState {
    pub sender: ControlSender,
    pub sse_broadcaster: SseEventBroadcaster,
    pub api_index: Arc<ApiIndex>,
    pub config: ControlConfig,
    pub schedule_registry: SharedScheduleRegistry,
}

impl AppState {
    pub fn new(
        sender: ControlSender,
        sse_broadcaster: SseEventBroadcaster,
        api_index: Arc<ApiIndex>,
        config: ControlConfig,
        schedule_registry: SharedScheduleRegistry,
    ) -> Self {
        Self {
            sender,
            sse_broadcaster,
            api_index,
            config,
            schedule_registry,
        }
    }
}

async fn send_operation(
    sender: &ControlSender,
    operation: ControlOperation,
) -> Result<serde_json::Value, (StatusCode, Json<serde_json::Value>)> {
    if sender.exclusive_execution().is_active() {
        return Err(reentrant_control_response());
    }

    let (response_tx, response_rx) = oneshot::channel();
    let request = ControlRequest {
        operation,
        response_tx,
    };
    sender
        .tx
        .send(request)
        .map_err(|_| error_response(StatusCode::SERVICE_UNAVAILABLE, "Engine not running"))?;

    match response_rx.await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(e)) => {
            let status = match e.code {
                ErrorCode::NotFound => StatusCode::NOT_FOUND,
                ErrorCode::Forbidden => StatusCode::FORBIDDEN,
                ErrorCode::InvalidParams => StatusCode::BAD_REQUEST,
                ErrorCode::Internal | ErrorCode::NotSupported => StatusCode::INTERNAL_SERVER_ERROR,
            };
            Err(error_response(status, &e.message))
        }
        Err(_) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Operation cancelled",
        )),
    }
}

fn error_response(status: StatusCode, message: &str) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({ "error": message })))
}

fn image_response(
    status: StatusCode,
    mut value: serde_json::Value,
    headers: &HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    let wants_mcp_delivery = headers
        .get(MCP_IMAGE_DELIVERY_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case(MCP_IMAGE_DELIVERY_VALUE));
    if wants_mcp_delivery {
        prepare_mcp_response_images(&mut value);
    }
    (status, Json(value))
}

fn reentrant_control_response() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({
            "code": "reentrant-control-call",
            "error": "Control API calls are unavailable while run_code holds exclusive World access; use the injected world object for in-engine inspection",
        })),
    )
}

fn parse_entity_ref(entity: &str) -> EntityRef {
    match entity.parse::<u64>() {
        Ok(id) => EntityRef::Id(id),
        Err(_) => EntityRef::Name(entity.to_string()),
    }
}
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Health
        .route("/health", get(health))
        // SSE
        .route("/api/v1/sse", get(handle_sse))
        // Entities
        .route("/api/v1/entities", get(list_entities))
        .route("/api/v1/entities", post(spawn_entity))
        .route("/api/v1/entities/{entity}", get(get_entity))
        .route("/api/v1/entities/{entity}", delete(despawn_entity))
        .route(
            "/api/v1/entities/{entity}/components/{component}",
            get(get_component),
        )
        .route(
            "/api/v1/entities/{entity}/components/{component}",
            put(set_component),
        )
        .route(
            "/api/v1/entities/{entity}/components/{component}",
            delete(remove_component),
        )
        .route(
            "/api/v1/entities/{entity}/bounding_box",
            get(get_bounding_box),
        )
        // Query
        .route("/api/v1/query", post(query_entities))
        .route("/api/v1/scene/summary", get(scene_summary))
        // Resources
        .route("/api/v1/resources", get(list_resources))
        .route("/api/v1/resources/{resource_type}", get(get_resource))
        .route("/api/v1/resources/{resource_type}", put(insert_resource))
        .route("/api/v1/resources/{resource_type}", delete(remove_resource))
        // Systems
        .route("/api/v1/systems", get(list_systems))
        // Component schema
        .route(
            "/api/v1/components/{name}/schema",
            get(get_component_schema),
        )
        // Screenshot
        .route("/api/v1/screenshot", post(capture_screenshot))
        .route("/api/v1/screenshot/gizmos", post(capture_with_gizmos))
        .route("/api/v1/screenshot/timeline", post(capture_timeline))
        .route("/api/v1/screenshot/turnaround", post(capture_turnaround))
        .route("/api/v1/screenshot/depth", post(capture_depth))
        .route("/api/v1/screenshot/stats", post(capture_stats))
        .route("/api/v1/screenshot/compare", post(compare_frames))
        // Reload
        .route("/api/v1/reload", post(trigger_reload))
        .route("/api/v1/reload/status", get(get_reload_status))
        .route("/api/v1/reload/capture", post(reload_and_capture))
        // Execute
        .route("/api/v1/execute", post(execute_python))
        // Time control
        .route("/api/v1/time", get(get_time_status))
        .route("/api/v1/time/pause", post(pause_time))
        .route("/api/v1/time/resume", post(resume_time))
        .route("/api/v1/time/scale", post(set_time_scale))
        .route("/api/v1/time/seek", post(seek_time))
        // Asset mutation
        .route("/api/v1/assets/mutate", post(mutate_asset))
        // Spatial queries
        .route("/api/v1/spatial/query", post(query_spatial))
        .route(
            "/api/v1/spatial/neighborhood",
            post(query_spatial_neighborhood),
        )
        .route("/api/v1/spatial/overlaps", post(check_overlaps))
        .route("/api/v1/spatial/overlaps/all", post(check_all_overlaps))
        // Performance
        .route("/api/v1/performance", get(get_performance))
        // Error
        .route("/api/v1/error", get(get_last_error))
        // Batch
        .route("/api/v1/batch", post(batch_mutate))
        // Debug
        .route("/api/v1/debug/registry", get(debug_registry))
        // Schedule
        .route("/api/v1/schedule", post(submit_schedule))
        .route(
            "/api/v1/schedule/{id}",
            get(get_schedule_status).delete(cancel_schedule),
        )
        // Plugin configs
        .route("/api/v1/config", get(list_configs))
        .route("/api/v1/config/{key}", get(get_config))
        // API discovery (served directly from ApiIndex, no World access needed)
        .route("/api/v1/stubs", get(stubs_index))
        .route("/api/v1/stubs/search", get(stubs_search))
        .route("/api/v1/stubs/module/{module}", get(stubs_module))
        .route("/api/v1/stubs/type/{type_name}", get(stubs_type))
        .route(
            "/api/v1/stubs/type/{type_name}/structured",
            get(stubs_type_structured),
        )
        .route("/api/v1/guides", get(guides_index))
        .route("/api/v1/guides/{name}", get(guides_get))
        .route("/api/v1/instructions", get(get_instructions))
        .route("/api/v1/tools", get(list_tools))
        .with_state(state)
}
async fn health() -> &'static str {
    "ok"
}

async fn handle_sse(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.sse_broadcaster.tx.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(data) => Some(Ok(Event::default().data(data))),
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
async fn list_entities(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::ListEntities).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn get_entity(
    State(state): State<AppState>,
    Path(entity): Path<String>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::GetEntity {
            entity: parse_entity_ref(&entity),
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpawnBody {
    components: serde_json::Value,
}

async fn spawn_entity(
    State(state): State<AppState>,
    Json(body): Json<SpawnBody>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::SpawnEntity {
            components: body.components,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::CREATED, Json(v)),
        Err(e) => e,
    }
}

async fn despawn_entity(
    State(state): State<AppState>,
    Path(entity): Path<String>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::DespawnEntity {
            entity: parse_entity_ref(&entity),
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn get_component(
    State(state): State<AppState>,
    Path((entity, component)): Path<(String, String)>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::GetComponent(GetComponentParams {
            entity: parse_entity_ref(&entity),
            component,
        }),
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetComponentBody {
    fields: serde_json::Value,
}

async fn set_component(
    State(state): State<AppState>,
    Path((entity, component)): Path<(String, String)>,
    Json(body): Json<SetComponentBody>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::SetComponent(SetComponentParams {
            entity: parse_entity_ref(&entity),
            component,
            fields: body.fields,
        }),
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn remove_component(
    State(state): State<AppState>,
    Path((entity, component)): Path<(String, String)>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::RemoveComponent(RemoveComponentParams {
            entity: parse_entity_ref(&entity),
            component,
        }),
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn get_bounding_box(
    State(state): State<AppState>,
    Path(entity): Path<String>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::GetBoundingBox {
            entity: parse_entity_ref(&entity),
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}
async fn query_entities(
    State(state): State<AppState>,
    Json(params): Json<QueryEntitiesParams>,
) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::QueryEntities(params)).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn scene_summary(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::GetSceneSummary).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}
async fn list_resources(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::ListResources).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn get_resource(
    State(state): State<AppState>,
    Path(resource_type): Path<String>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::GetResource(GetResourceParams { resource_type }),
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InsertResourceBody {
    value: serde_json::Value,
}

async fn insert_resource(
    State(state): State<AppState>,
    Path(resource_type): Path<String>,
    Json(body): Json<InsertResourceBody>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::SetResource(SetResourceParams {
            resource_type,
            value: body.value,
        }),
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn remove_resource(
    State(state): State<AppState>,
    Path(resource_type): Path<String>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::RemoveResource { resource_type },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}
#[derive(Default, Deserialize)]
struct ListSystemsQuery {
    #[serde(default)]
    include_internal: bool,
}

async fn list_systems(
    State(state): State<AppState>,
    AxumQuery(query): AxumQuery<ListSystemsQuery>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::ListSystems {
            include_internal: query.include_internal,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn get_component_schema(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::GetComponentSchema { name }).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScreenshotBody {
    entity: Option<EntityRef>,
    #[serde(default = "default_delay_frames")]
    delay_frames: i64,
    max_width: Option<i64>,
    position: Option<[f32; 3]>,
    look_at: Option<[f32; 3]>,
    #[serde(default = "default_true")]
    hide_ui: bool,
    #[serde(default)]
    gizmos: bool,
}

fn default_delay_frames() -> i64 {
    2
}

fn default_true() -> bool {
    true
}

fn screenshot_params(
    body: ScreenshotBody,
    force_gizmos: bool,
) -> Result<CaptureScreenshotParams, (StatusCode, Json<serde_json::Value>)> {
    if body.delay_frames < 0 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "delay_frames must be a non-negative integer",
        ));
    }
    let delay_frames = u32::try_from(body.delay_frames).map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "delay_frames must not exceed 4294967295",
        )
    })?;
    let max_width = match body.max_width {
        Some(value) if value <= 0 => {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "max_width must be a positive integer",
            ));
        }
        Some(value) => Some(u32::try_from(value).map_err(|_| {
            error_response(
                StatusCode::BAD_REQUEST,
                "max_width must not exceed 4294967295",
            )
        })?),
        None => None,
    };

    Ok(CaptureScreenshotParams {
        entity: body.entity,
        delay_frames,
        max_width,
        position: body.position,
        look_at: body.look_at,
        hide_ui: body.hide_ui,
        gizmos: force_gizmos || body.gizmos,
    })
}

async fn capture_screenshot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ScreenshotBody>,
) -> impl IntoResponse {
    let params = match screenshot_params(body, false) {
        Ok(params) => params,
        Err(response) => return response,
    };
    let op = if params.gizmos {
        ControlOperation::CaptureWithGizmos(params)
    } else {
        ControlOperation::CaptureScreenshot(params)
    };
    match send_operation(&state.sender, op).await {
        Ok(value) => image_response(StatusCode::OK, value, &headers),
        Err(e) => e,
    }
}

async fn capture_with_gizmos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ScreenshotBody>,
) -> impl IntoResponse {
    let params = match screenshot_params(body, true) {
        Ok(params) => params,
        Err(response) => return response,
    };
    match send_operation(&state.sender, ControlOperation::CaptureWithGizmos(params)).await {
        Ok(value) => image_response(StatusCode::OK, value, &headers),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatsBody {
    #[serde(default = "default_stats_grid")]
    grid: u32,
    region: Option<[i64; 4]>,
    sample_points: Option<Vec<[i64; 2]>>,
    entity: Option<EntityRef>,
    #[serde(default = "default_delay_frames")]
    delay_frames: i64,
    max_width: Option<i64>,
    position: Option<[f32; 3]>,
    look_at: Option<[f32; 3]>,
    #[serde(default = "default_true")]
    hide_ui: bool,
    #[serde(default)]
    gizmos: bool,
}

fn default_stats_grid() -> u32 {
    1
}

async fn capture_stats(
    State(state): State<AppState>,
    Json(body): Json<StatsBody>,
) -> impl IntoResponse {
    let screenshot = ScreenshotBody {
        entity: body.entity,
        delay_frames: body.delay_frames,
        max_width: body.max_width,
        position: body.position,
        look_at: body.look_at,
        hide_ui: body.hide_ui,
        gizmos: body.gizmos,
    };
    let common = match screenshot_params(screenshot, false) {
        Ok(params) => params,
        Err(response) => return response,
    };
    match send_operation(
        &state.sender,
        ControlOperation::CaptureStats(CaptureStatsParams {
            entity: common.entity,
            grid: body.grid,
            region: body.region,
            sample_points: body.sample_points,
            delay_frames: common.delay_frames,
            max_width: common.max_width,
            position: common.position,
            look_at: common.look_at,
            hide_ui: common.hide_ui,
            gizmos: common.gizmos,
        }),
    )
    .await
    {
        Ok(value) => (StatusCode::OK, Json(value)),
        Err(response) => response,
    }
}

async fn compare_frames(
    State(state): State<AppState>,
    Json(params): Json<CompareFramesParams>,
) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::CompareFrames(params)).await {
        Ok(value) => (StatusCode::OK, Json(value)),
        Err(response) => response,
    }
}

async fn capture_timeline(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(params): Json<CaptureTimelineParams>,
) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::CaptureTimeline(params)).await {
        Ok(value) => image_response(StatusCode::OK, value, &headers),
        Err(e) => e,
    }
}

async fn capture_turnaround(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(params): Json<CaptureTurnaroundParams>,
) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::CaptureTurnaround(params)).await {
        Ok(value) => image_response(StatusCode::OK, value, &headers),
        Err(e) => e,
    }
}

async fn capture_depth(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(params): Json<CaptureDepthParams>,
) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::CaptureDepth(params)).await {
        Ok(value) => image_response(StatusCode::OK, value, &headers),
        Err(e) => e,
    }
}
async fn trigger_reload(
    State(state): State<AppState>,
    Json(params): Json<ReloadParams>,
) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::Reload(params)).await {
        Ok(value) => (StatusCode::OK, Json(value)),
        Err(e) => e,
    }
}

async fn get_reload_status(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::GetReloadStatus).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn reload_and_capture(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(params): Json<ReloadAndCaptureParams>,
) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::ReloadAndCapture(params)).await {
        Ok(value) => image_response(StatusCode::OK, value, &headers),
        Err(e) => e,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecuteBody {
    code: String,
}

async fn execute_python(
    State(state): State<AppState>,
    Json(body): Json<ExecuteBody>,
) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::RunCode { code: body.code }).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}
async fn get_time_status(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::GetTimeStatus).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn pause_time(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::PauseTime).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn resume_time(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::ResumeTime).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TimeScaleBody {
    scale: f32,
}

async fn set_time_scale(
    State(state): State<AppState>,
    Json(body): Json<TimeScaleBody>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::SetTimeScale { scale: body.scale },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn seek_time(
    State(state): State<AppState>,
    Json(params): Json<SeekTimeParams>,
) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::SeekTime(params)).await {
        Ok(value) => (StatusCode::OK, Json(value)),
        Err(e) => e,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MutateAssetBody {
    entity: serde_json::Value,
    component: String,
    asset_type: String,
    fields: serde_json::Value,
}

async fn mutate_asset(
    State(state): State<AppState>,
    Json(body): Json<MutateAssetBody>,
) -> impl IntoResponse {
    let entity: EntityRef = match serde_json::from_value(body.entity) {
        Ok(e) => e,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid entity reference"),
    };
    match send_operation(
        &state.sender,
        ControlOperation::SetAsset(SetAssetParams {
            entity,
            component: body.component,
            asset_type: body.asset_type,
            fields: body.fields,
        }),
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpatialQueryBody {
    entity_a: serde_json::Value,
    entity_b: serde_json::Value,
}

async fn query_spatial(
    State(state): State<AppState>,
    Json(body): Json<SpatialQueryBody>,
) -> impl IntoResponse {
    let entity_a: EntityRef = match serde_json::from_value(body.entity_a) {
        Ok(e) => e,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid entity_a reference"),
    };
    let entity_b: EntityRef = match serde_json::from_value(body.entity_b) {
        Ok(e) => e,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid entity_b reference"),
    };
    match send_operation(
        &state.sender,
        ControlOperation::QuerySpatial(QuerySpatialParams { entity_a, entity_b }),
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NeighborhoodBody {
    entity: serde_json::Value,
    radius: f32,
    max_results: Option<usize>,
}

async fn query_spatial_neighborhood(
    State(state): State<AppState>,
    Json(body): Json<NeighborhoodBody>,
) -> impl IntoResponse {
    let entity: EntityRef = match serde_json::from_value(body.entity) {
        Ok(e) => e,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid entity reference"),
    };
    match send_operation(
        &state.sender,
        ControlOperation::QuerySpatialNeighborhood(QuerySpatialNeighborhoodParams {
            entity,
            radius: body.radius,
            max_results: body.max_results,
        }),
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckOverlapsBody {
    entity: serde_json::Value,
    #[serde(default)]
    include_siblings: bool,
    #[serde(default = "default_max_float_gap")]
    max_float_gap: f32,
    ground_y: Option<f32>,
    min_penetration: Option<f32>,
}

async fn check_overlaps(
    State(state): State<AppState>,
    Json(body): Json<CheckOverlapsBody>,
) -> impl IntoResponse {
    let entity: EntityRef = match serde_json::from_value(body.entity) {
        Ok(e) => e,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid entity reference"),
    };
    match send_operation(
        &state.sender,
        ControlOperation::CheckOverlaps(CheckOverlapsParams {
            entity,
            include_siblings: body.include_siblings,
            max_float_gap: body.max_float_gap,
            ground_y: body.ground_y,
            min_penetration: body.min_penetration,
        }),
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn check_all_overlaps(
    State(state): State<AppState>,
    Json(params): Json<CheckAllOverlapsParams>,
) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::CheckAllOverlaps(params)).await {
        Ok(value) => (StatusCode::OK, Json(value)),
        Err(e) => e,
    }
}
async fn get_performance(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::GetPerformance).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn get_last_error(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::GetLastError).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn debug_registry(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::GetRegistry).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchBody {
    operations: Vec<serde_json::Value>,
}

async fn batch_mutate(
    State(state): State<AppState>,
    Json(body): Json<BatchBody>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::Batch {
            operations: body.operations,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn submit_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ScheduleRequest>,
) -> impl IntoResponse {
    // Validate before sending to engine
    if let Err(e) = validate_schedule(&body) {
        return error_response(StatusCode::BAD_REQUEST, &e);
    }

    let is_async = body.mode == ScheduleMode::Async;

    match send_operation(&state.sender, ControlOperation::ScheduleActions(body)).await {
        Ok(v) => {
            let status = if is_async {
                StatusCode::ACCEPTED
            } else {
                StatusCode::OK
            };
            image_response(status, v, &headers)
        }
        Err(e) => e,
    }
}

async fn get_schedule_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.schedule_registry.get(&id) {
        Some(status) => {
            let json = serde_json::to_value(&status).unwrap_or_default();
            image_response(StatusCode::OK, json, &headers)
        }
        None => error_response(
            StatusCode::NOT_FOUND,
            &format!("Schedule '{}' not found", id),
        ),
    }
}

async fn cancel_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if state.schedule_registry.cancel(&id) {
        (
            StatusCode::OK,
            Json(serde_json::json!({ "cancelled": true, "schedule_id": id })),
        )
    } else {
        error_response(
            StatusCode::NOT_FOUND,
            &format!("Schedule '{}' not found or already completed", id),
        )
    }
}
async fn list_configs(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::ListConfigs).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn get_config(State(state): State<AppState>, Path(key): Path<String>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::GetConfig { key }).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}
#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default)]
    limit: Option<usize>,
}

/// Result cap when a stub search request does not pass an explicit limit.
const DEFAULT_STUB_SEARCH_LIMIT: usize = 200;

async fn stubs_index(State(state): State<AppState>) -> impl IntoResponse {
    if !state.config.api_discovery {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "API discovery disabled" })),
        );
    }
    (
        StatusCode::OK,
        Json(serde_json::to_value(state.api_index.get_index()).unwrap_or_default()),
    )
}

async fn stubs_search(
    State(state): State<AppState>,
    AxumQuery(params): AxumQuery<SearchQuery>,
) -> impl IntoResponse {
    if !state.config.api_discovery {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "API discovery disabled" })),
        );
    }
    let limit = params.limit.unwrap_or(DEFAULT_STUB_SEARCH_LIMIT);
    let (results, total) = state.api_index.search(&params.q, Some(limit));
    let truncated = results.len() < total;
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "results": results,
            "total": total,
            "truncated": truncated,
        })),
    )
}

async fn stubs_module(
    State(state): State<AppState>,
    Path(module): Path<String>,
) -> impl IntoResponse {
    if !state.config.api_discovery {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "API discovery disabled" })),
        );
    }
    match state.api_index.get_module_content(&module) {
        Some(content) => (
            StatusCode::OK,
            Json(serde_json::json!({ "module": module, "content": content })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Module '{module}' not found") })),
        ),
    }
}

async fn stubs_type(
    State(state): State<AppState>,
    Path(type_name): Path<String>,
) -> impl IntoResponse {
    if !state.config.api_discovery {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "API discovery disabled" })),
        );
    }
    match state.api_index.get_type_definition(&type_name) {
        Some(def) => (
            StatusCode::OK,
            Json(serde_json::json!({ "type": type_name, "definition": def })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("Type '{type_name}' not found"),
                "suggestions": state.api_index.suggest_type_names(&type_name),
            })),
        ),
    }
}

async fn stubs_type_structured(
    State(state): State<AppState>,
    Path(type_name): Path<String>,
) -> impl IntoResponse {
    if !state.config.api_discovery {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "API discovery disabled" })),
        );
    }
    match state.api_index.get_type_definition_structured(&type_name) {
        Some(structured) => (StatusCode::OK, Json(structured)),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("Type '{type_name}' not found"),
                "suggestions": state.api_index.suggest_type_names(&type_name),
            })),
        ),
    }
}

async fn guides_index(State(state): State<AppState>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::to_value(state.api_index.get_guide_index()).unwrap_or_default()),
    )
}

async fn guides_get(State(state): State<AppState>, Path(name): Path<String>) -> impl IntoResponse {
    match state.api_index.get_guide(&name) {
        Some(content) => (
            StatusCode::OK,
            Json(serde_json::json!({ "name": name, "content": content })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Guide '{name}' not found") })),
        ),
    }
}

async fn get_instructions(State(state): State<AppState>) -> impl IntoResponse {
    match state.api_index.get_instructions() {
        Some(content) => (
            StatusCode::OK,
            Json(serde_json::json!({ "instructions": content })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "No instructions found" })),
        ),
    }
}

async fn list_tools() -> Json<Vec<serde_json::Value>> {
    Json(crate::catalog::tools::list_tools())
}

pub fn start_server(host: String, port: u16, state: AppState) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime for control server");

        rt.block_on(async move {
            let addr = format!("{host}:{port}");
            let listener = match tokio::net::TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[Control] Failed to bind to {addr}: {e}");
                    return;
                }
            };
            eprintln!("[Control] Server listening on http://{addr}");
            eprintln!("[Control] REST API: http://{addr}/api/v1/");

            let router = build_router(state);
            if let Err(e) = axum::serve(listener, router).await {
                eprintln!("[Control] Server error: {e}");
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use tower::ServiceExt;

    use super::*;
    use crate::{
        engine::channel::{ControlReceiver, create_channel},
        handlers::schedule::SharedScheduleRegistry,
        protocol::ControlError,
    };

    #[test]
    fn parse_entity_ref_numeric_string() {
        let result = parse_entity_ref("42");
        assert!(matches!(result, EntityRef::Id(42)));
    }

    #[test]
    fn parse_entity_ref_name_string() {
        let result = parse_entity_ref("Player");
        assert!(matches!(result, EntityRef::Name(ref s) if s == "Player"));
    }

    #[test]
    fn parse_entity_ref_zero() {
        let result = parse_entity_ref("0");
        assert!(matches!(result, EntityRef::Id(0)));
    }

    #[test]
    fn parse_entity_ref_large_number() {
        let result = parse_entity_ref("18446744073709551615");
        assert!(matches!(result, EntityRef::Id(u64::MAX)));
    }

    #[test]
    fn parse_entity_ref_negative_is_name() {
        // Negative numbers can't be u64, so they become names
        let result = parse_entity_ref("-1");
        assert!(matches!(result, EntityRef::Name(ref s) if s == "-1"));
    }

    #[test]
    fn parse_entity_ref_empty_string_is_name() {
        let result = parse_entity_ref("");
        assert!(matches!(result, EntityRef::Name(ref s) if s.is_empty()));
    }

    #[test]
    fn error_response_format() {
        let (status, Json(body)) = error_response(StatusCode::NOT_FOUND, "not found");
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "not found");
    }

    #[test]
    fn error_response_internal() {
        let (status, Json(body)) = error_response(StatusCode::INTERNAL_SERVER_ERROR, "crash");
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["error"], "crash");
    }

    #[test]
    fn default_delay_frames_value() {
        assert_eq!(default_delay_frames(), 2);
    }

    #[test]
    fn screenshot_body_deserialize_defaults() {
        let json = r#"{"position": [1.0, 2.0, 3.0]}"#;
        let body: ScreenshotBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.delay_frames, 2);
        assert!(body.max_width.is_none());
        assert_eq!(body.position, Some([1.0, 2.0, 3.0]));
        assert!(body.look_at.is_none());
        assert!(body.hide_ui);
    }

    #[test]
    fn screenshot_body_deserialize_all_fields() {
        let json = r#"{"delay_frames": 5, "max_width": 800, "position": [0.0, 0.0, 0.0], "look_at": [1.0, 1.0, 1.0], "hide_ui": true}"#;
        let body: ScreenshotBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.delay_frames, 5);
        assert_eq!(body.max_width, Some(800));
        assert!(body.hide_ui);
    }

    #[test]
    fn screenshot_body_preserves_explicit_hide_ui_false() {
        let body: ScreenshotBody = serde_json::from_str(r#"{"hide_ui": false}"#).unwrap();
        assert!(!body.hide_ui);
    }

    #[test]
    fn stats_body_deserializes_analysis_and_capture_defaults() {
        let body: StatsBody = serde_json::from_str(r#"{"sample_points": [[1, 2]]}"#).unwrap();
        assert_eq!(body.grid, 1);
        assert_eq!(body.sample_points, Some(vec![[1, 2]]));
        assert_eq!(body.delay_frames, 2);
        assert!(body.hide_ui);
    }

    #[test]
    fn check_overlaps_body_deserialize_defaults() {
        let json = r#"{"entity": 42}"#;
        let body: CheckOverlapsBody = serde_json::from_str(json).unwrap();
        assert!(!body.include_siblings);
        assert_eq!(body.max_float_gap, 0.1);
    }

    #[test]
    fn spawn_body_deserialize() {
        let json = r#"{"components": {"Transform": {"translation": [0, 1, 0]}}}"#;
        let body: SpawnBody = serde_json::from_str(json).unwrap();
        assert!(body.components.is_object());
    }

    #[test]
    fn set_component_body_deserialize() {
        let json = r#"{"fields": {"intensity": 100.0}}"#;
        let body: SetComponentBody = serde_json::from_str(json).unwrap();
        assert!(body.fields.is_object());
    }

    #[test]
    fn execute_body_deserialize() {
        let json = r#"{"code": "print('hi')"}"#;
        let body: ExecuteBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.code, "print('hi')");
    }

    #[test]
    fn batch_body_deserialize() {
        let json = r#"{"operations": [{"type": "spawn", "components": {}}]}"#;
        let body: BatchBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.operations.len(), 1);
    }

    #[test]
    fn insert_resource_body_deserialize() {
        let json = r#"{"value": {"difficulty": "hard"}}"#;
        let body: InsertResourceBody = serde_json::from_str(json).unwrap();
        assert!(body.value.is_object());
    }

    #[test]
    fn mutate_asset_body_deserialize() {
        let json = r#"{"entity": 42, "component": "MeshMaterial3d", "asset_type": "StandardMaterial", "fields": {"base_color": [1,0,0,1]}}"#;
        let body: MutateAssetBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.component, "MeshMaterial3d");
        assert_eq!(body.asset_type, "StandardMaterial");
    }

    #[test]
    fn spatial_query_body_deserialize() {
        let json = r#"{"entity_a": 1, "entity_b": 2}"#;
        let body: SpatialQueryBody = serde_json::from_str(json).unwrap();
        assert!(body.entity_a.is_number());
    }

    #[test]
    fn neighborhood_body_deserialize() {
        let json = r#"{"entity": "Player", "radius": 10.0}"#;
        let body: NeighborhoodBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.radius, 10.0);
        assert!(body.max_results.is_none());
    }

    #[test]
    fn time_scale_body_deserialize() {
        let json = r#"{"scale": 2.5}"#;
        let body: TimeScaleBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.scale, 2.5);
    }

    #[test]
    fn search_query_deserialize() {
        let json = r#"{"q": "Transform"}"#;
        let q: SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(q.q, "Transform");
        assert_eq!(q.limit, None);
    }

    #[test]
    fn search_query_deserialize_with_limit() {
        let json = r#"{"q": "Transform", "limit": 25}"#;
        let q: SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(q.q, "Transform");
        assert_eq!(q.limit, Some(25));
    }

    fn test_state_enabled() -> (AppState, ControlReceiver) {
        let (sender, receiver) = create_channel();
        let state = AppState::new(
            sender,
            SseEventBroadcaster::new(),
            Arc::new(ApiIndex::build(std::path::Path::new(""))),
            ControlConfig {
                port: 0,
                host: "127.0.0.1".into(),
                screenshot: true,
                manipulation: true,
                execute_python: true,
                api_discovery: true,
            },
            SharedScheduleRegistry::default(),
        );
        (state, receiver)
    }

    fn test_state_disabled() -> (AppState, ControlReceiver) {
        let (sender, receiver) = create_channel();
        let state = AppState::new(
            sender,
            SseEventBroadcaster::new(),
            Arc::new(ApiIndex::build(std::path::Path::new(""))),
            ControlConfig {
                port: 0,
                host: "127.0.0.1".into(),
                screenshot: false,
                manipulation: false,
                execute_python: false,
                api_discovery: false,
            },
            SharedScheduleRegistry::default(),
        );
        (state, receiver)
    }

    fn json_post(uri: &str, body: &str) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap()
    }

    fn mcp_json_post(uri: &str, body: &str) -> axum::http::Request<axum::body::Body> {
        let mut request = json_post(uri, body);
        request.headers_mut().insert(
            MCP_IMAGE_DELIVERY_HEADER,
            MCP_IMAGE_DELIVERY_VALUE.parse().unwrap(),
        );
        request
    }

    fn get_req(uri: &str) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .uri(uri)
            .body(axum::body::Body::empty())
            .unwrap()
    }

    fn delete_req(uri: &str) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .method("DELETE")
            .uri(uri)
            .body(axum::body::Body::empty())
            .unwrap()
    }

    /// Cross-origin browser requests must not be granted CORS approval: the
    /// control API is unauthenticated, so answering preflights or emitting
    /// allow-origin headers would let any web page drive a local engine.
    #[tokio::test]
    async fn no_cors_headers_for_cross_origin_requests() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);

        let preflight = axum::http::Request::builder()
            .method("OPTIONS")
            .uri("/api/v1/execute")
            .header("origin", "https://evil.example")
            .header("access-control-request-method", "POST")
            .header("access-control-request-headers", "content-type")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = app.clone().oneshot(preflight).await.unwrap();
        assert!(
            response
                .headers()
                .get("access-control-allow-origin")
                .is_none(),
            "preflight must not be approved"
        );

        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req.response_tx.send(Ok(serde_json::json!({"fps": 60})));
            }
        });
        let mut request = get_req("/api/v1/performance");
        request
            .headers_mut()
            .insert("origin", "https://evil.example".parse().unwrap());
        let response = app.oneshot(request).await.unwrap();
        assert!(
            response
                .headers()
                .get("access-control-allow-origin")
                .is_none(),
            "responses must not be readable cross-origin"
        );
    }

    #[tokio::test]
    async fn health_endpoint() {
        let (state, _rx) = test_state_enabled();
        let app = build_router(state);
        let response = app.oneshot(get_req("/health")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_entities_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req
                    .response_tx
                    .send(Ok(serde_json::json!({"entities": []})));
            }
        });
        let response = app.oneshot(get_req("/api/v1/entities")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_entity_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req
                    .response_tx
                    .send(Ok(serde_json::json!({"entity": 42, "components": {}})));
            }
        });
        let response = app.oneshot(get_req("/api/v1/entities/42")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_time_status_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req
                    .response_tx
                    .send(Ok(serde_json::json!({"paused": false})));
            }
        });
        let response = app.oneshot(get_req("/api/v1/time")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_performance_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req.response_tx.send(Ok(serde_json::json!({"fps": 60})));
            }
        });
        let response = app.oneshot(get_req("/api/v1/performance")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn timeline_route_forwards_the_schema_default_span() {
        let (state, mut receiver) = test_state_enabled();
        let app = build_router(state);
        let responder = tokio::spawn(async move {
            let request = receiver.rx.recv().await.unwrap();
            let ControlOperation::CaptureTimeline(params) = &request.operation else {
                panic!("expected capture_timeline, got {:?}", request.operation);
            };
            assert_eq!(params.total_frames, 60);
            let _ = request.response_tx.send(Ok(serde_json::json!({})));
        });

        let response = app
            .oneshot(json_post("/api/v1/screenshot/timeline", "{}"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        responder.await.unwrap();
    }

    #[tokio::test]
    async fn body_routes_reject_unknown_fields() {
        let (state, _receiver) = test_state_enabled();
        let app = build_router(state);
        for (uri, body) in [
            ("/api/v1/screenshot/turnaround", r#"{"views": 3}"#),
            ("/api/v1/screenshot/stats", r#"{"grid": 2, "views": 3}"#),
            ("/api/v1/entities", r#"{"components": {}, "views": 3}"#),
        ] {
            let response = app.clone().oneshot(json_post(uri, body)).await.unwrap();
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY, "{uri}");
            let text = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let text = String::from_utf8(text.to_vec()).unwrap();
            assert!(text.contains("unknown field `views`"), "{uri}: {text}");
        }
    }

    #[tokio::test]
    async fn frame_analysis_routes_forward_typed_operations() {
        let (state, mut receiver) = test_state_enabled();
        let app = build_router(state);
        let responder = tokio::spawn(async move {
            let stats = receiver.rx.recv().await.unwrap();
            assert!(matches!(
                stats.operation,
                ControlOperation::CaptureStats(CaptureStatsParams {
                    grid: 2,
                    delay_frames: 3,
                    entity: Some(EntityRef::Name(ref name)),
                    ..
                }) if name == "Target"
            ));
            let _ = stats
                .response_tx
                .send(Ok(serde_json::json!({"retained": true})));

            let compare = receiver.rx.recv().await.unwrap();
            assert!(matches!(
                compare.operation,
                ControlOperation::CompareFrames(CompareFramesParams { a, b, .. })
                    if a == "f_1" && b == "f_2"
            ));
            let _ = compare
                .response_tx
                .send(Ok(serde_json::json!({"identical": false})));
        });

        let stats_response = app
            .clone()
            .oneshot(json_post(
                "/api/v1/screenshot/stats",
                r#"{"grid": 2, "delay_frames": 3, "entity": "Target"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(stats_response.status(), StatusCode::OK);

        let compare_response = app
            .oneshot(json_post(
                "/api/v1/screenshot/compare",
                r#"{"a": "f_1", "b": "f_2"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(compare_response.status(), StatusCode::OK);
        responder.await.unwrap();
    }

    #[tokio::test]
    async fn control_request_rejected_during_exclusive_execution_then_recovers() {
        let (state, mut rx) = test_state_enabled();
        let execution = state.sender.exclusive_execution();
        let guard = execution.try_enter().expect("entry should succeed");
        let app = build_router(state);

        let conflict = app
            .clone()
            .oneshot(get_req("/api/v1/performance"))
            .await
            .unwrap();
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        let body = axum::body::to_bytes(conflict.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["code"], "reentrant-control-call");
        assert!(body["error"].as_str().unwrap().contains("injected world"));
        assert!(matches!(
            rx.rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));

        drop(guard);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req.response_tx.send(Ok(serde_json::json!({"fps": 60})));
            }
        });
        let recovered = app.oneshot(get_req("/api/v1/performance")).await.unwrap();
        assert_eq!(recovered.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn query_entities_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req
                    .response_tx
                    .send(Ok(serde_json::json!({"entities": []})));
            }
        });
        let response = app
            .oneshot(json_post(
                "/api/v1/query",
                r#"{"with": ["Transform"], "without": []}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn spawn_entity_created() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req.response_tx.send(Ok(serde_json::json!({"entity": 99})));
            }
        });
        let response = app
            .oneshot(json_post(
                "/api/v1/entities",
                r#"{"components": {"Transform": {}}}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn scene_summary_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req.response_tx.send(Ok(serde_json::json!({"groups": []})));
            }
        });
        let response = app.oneshot(get_req("/api/v1/scene/summary")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_component_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req.response_tx.send(Ok(
                    serde_json::json!({"component": "Transform", "fields": {}}),
                ));
            }
        });
        let response = app
            .oneshot(get_req("/api/v1/entities/42/components/Transform"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn set_component_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req.response_tx.send(Ok(serde_json::json!({"ok": true})));
            }
        });
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("PUT")
                    .uri("/api/v1/entities/42/components/Transform")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        r#"{"fields": {"translation": [0,1,0]}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn despawn_entity_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req
                    .response_tx
                    .send(Ok(serde_json::json!({"despawned": true})));
            }
        });
        let response = app
            .oneshot(delete_req("/api/v1/entities/42"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn remove_component_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req
                    .response_tx
                    .send(Ok(serde_json::json!({"removed": true})));
            }
        });
        let response = app
            .oneshot(delete_req("/api/v1/entities/42/components/Marker"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_resources_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req
                    .response_tx
                    .send(Ok(serde_json::json!({"resources": []})));
            }
        });
        let response = app.oneshot(get_req("/api/v1/resources")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_resource_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                assert!(matches!(
                    req.operation,
                    ControlOperation::GetResource(GetResourceParams { resource_type })
                        if resource_type == "State[game.Phase]"
                ));
                let _ = req.response_tx.send(Ok(serde_json::json!({
                    "name": "State[game.Phase]",
                    "present": true,
                    "fields": {"variant": "MENU"},
                })));
            }
        });
        let response = app
            .oneshot(get_req("/api/v1/resources/State%5Bgame.Phase%5D"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_systems_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                assert!(matches!(
                    req.operation,
                    ControlOperation::ListSystems {
                        include_internal: false
                    }
                ));
                let _ = req.response_tx.send(Ok(serde_json::json!({"systems": []})));
            }
        });
        let response = app.oneshot(get_req("/api/v1/systems")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_systems_can_include_internal_systems() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                assert!(matches!(
                    req.operation,
                    ControlOperation::ListSystems {
                        include_internal: true
                    }
                ));
                let _ = req.response_tx.send(Ok(serde_json::json!({"systems": []})));
            }
        });
        let response = app
            .oneshot(get_req("/api/v1/systems?include_internal=true"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_component_schema_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req.response_tx.send(Ok(serde_json::json!({"schema": {}})));
            }
        });
        let response = app
            .oneshot(get_req("/api/v1/components/Transform/schema"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn pause_time_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req
                    .response_tx
                    .send(Ok(serde_json::json!({"paused": true})));
            }
        });
        let response = app
            .oneshot(json_post("/api/v1/time/pause", "{}"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn resume_time_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req
                    .response_tx
                    .send(Ok(serde_json::json!({"paused": false})));
            }
        });
        let response = app
            .oneshot(json_post("/api/v1/time/resume", "{}"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_last_error_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req.response_tx.send(Ok(serde_json::json!({"error": null})));
            }
        });
        let response = app.oneshot(get_req("/api/v1/error")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn debug_registry_ok() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req
                    .response_tx
                    .send(Ok(serde_json::json!({"registry": {}})));
            }
        });
        let response = app
            .oneshot(get_req("/api/v1/debug/registry"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn guides_index_ok() {
        let (state, _rx) = test_state_enabled();
        let app = build_router(state);
        let response = app.oneshot(get_req("/api/v1/guides")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn guides_get_not_found() {
        let (state, _rx) = test_state_enabled();
        let app = build_router(state);
        let response = app
            .oneshot(get_req("/api/v1/guides/nonexistent"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn instructions_not_found_when_empty() {
        let (state, _rx) = test_state_enabled();
        let app = build_router(state);
        let response = app.oneshot(get_req("/api/v1/instructions")).await.unwrap();
        // Empty ApiIndex has no instructions
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn stubs_type_not_found() {
        let (state, _rx) = test_state_enabled();
        let app = build_router(state);
        let response = app
            .oneshot(get_req("/api/v1/stubs/type/NonExistent"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn stubs_module_not_found() {
        let (state, _rx) = test_state_enabled();
        let app = build_router(state);
        let response = app
            .oneshot(get_req("/api/v1/stubs/module/nonexistent"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn screenshot_mcp_delivery_is_opt_in_at_the_http_boundary() {
        const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

        let (state, mut receiver) = test_state_enabled();
        let app = build_router(state);
        let responder = tokio::spawn(async move {
            for _ in 0..2 {
                let request = receiver.rx.recv().await.unwrap();
                request
                    .response_tx
                    .send(Ok(serde_json::json!({
                        "image": PNG,
                        "width": 1,
                        "height": 1,
                        "format": "png",
                        "encoding": "base64",
                    })))
                    .unwrap();
            }
        });

        let rest_response = app
            .clone()
            .oneshot(json_post("/api/v1/screenshot", "{}"))
            .await
            .unwrap();
        let rest_body = to_bytes(rest_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let rest_body: serde_json::Value = serde_json::from_slice(&rest_body).unwrap();
        assert_eq!(rest_body["image"], PNG);
        assert!(rest_body.get("image_delivery").is_none());

        let mcp_response = app
            .oneshot(mcp_json_post("/api/v1/screenshot", "{}"))
            .await
            .unwrap();
        let mcp_body = to_bytes(mcp_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let mcp_body: serde_json::Value = serde_json::from_slice(&mcp_body).unwrap();
        assert_eq!(mcp_body["image"], PNG);
        assert_eq!(mcp_body["image_delivery"]["inline_mime_type"], "image/png");
        assert_eq!(mcp_body["image_delivery"]["full_resolution_width"], 1);

        responder.await.unwrap();
    }

    #[tokio::test]
    async fn engine_forbidden_error_maps_to_http_403() {
        let (state, mut receiver) = test_state_enabled();
        let app = build_router(state);
        let responder = tokio::spawn(async move {
            let request = receiver.rx.recv().await.unwrap();
            request
                .response_tx
                .send(Err(ControlError::forbidden("Manipulation disabled")))
                .unwrap();
        });

        let response = app
            .oneshot(json_post("/api/v1/entities", r#"{"components": {}}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, serde_json::json!({"error": "Manipulation disabled"}));

        responder.await.unwrap();
    }

    #[tokio::test]
    async fn stubs_index_forbidden() {
        let (state, _rx) = test_state_disabled();
        let app = build_router(state);
        let response = app.oneshot(get_req("/api/v1/stubs")).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn stubs_search_forbidden() {
        let (state, _rx) = test_state_disabled();
        let app = build_router(state);
        let response = app
            .oneshot(get_req("/api/v1/stubs/search?q=Transform"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn stubs_module_forbidden() {
        let (state, _rx) = test_state_disabled();
        let app = build_router(state);
        let response = app
            .oneshot(get_req("/api/v1/stubs/module/transform"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn stubs_type_forbidden() {
        let (state, _rx) = test_state_disabled();
        let app = build_router(state);
        let response = app
            .oneshot(get_req("/api/v1/stubs/type/Transform"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn stubs_type_structured_forbidden() {
        let (state, _rx) = test_state_disabled();
        let app = build_router(state);
        let response = app
            .oneshot(get_req("/api/v1/stubs/type/Transform/structured"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn channel_closed_returns_503() {
        let (state, rx) = test_state_enabled();
        drop(rx);
        let app = build_router(state);
        let response = app.oneshot(get_req("/api/v1/entities")).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn control_error_not_found_returns_404() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req
                    .response_tx
                    .send(Err(ControlError::not_found("missing")));
            }
        });
        let response = app.oneshot(get_req("/api/v1/entities")).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn control_error_invalid_params_returns_400() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req
                    .response_tx
                    .send(Err(ControlError::invalid_params("bad")));
            }
        });
        let response = app.oneshot(get_req("/api/v1/entities")).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn control_error_internal_returns_500() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                let _ = req.response_tx.send(Err(ControlError::internal("crash")));
            }
        });
        let response = app.oneshot(get_req("/api/v1/entities")).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn response_tx_dropped_returns_500() {
        let (state, mut rx) = test_state_enabled();
        let app = build_router(state);
        tokio::spawn(async move {
            if let Some(req) = rx.rx.recv().await {
                drop(req.response_tx); // Drop without sending
            }
        });
        let response = app.oneshot(get_req("/api/v1/entities")).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn schedule_get_not_found() {
        let (state, _rx) = test_state_enabled();
        let app = build_router(state);
        let response = app
            .oneshot(get_req("/api/v1/schedule/nonexistent"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn schedule_cancel_not_found() {
        let (state, _rx) = test_state_enabled();
        let app = build_router(state);
        let response = app
            .oneshot(delete_req("/api/v1/schedule/nonexistent"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn schedule_validate_empty_actions() {
        let (state, _rx) = test_state_enabled();
        let app = build_router(state);
        let response = app
            .oneshot(json_post(
                "/api/v1/schedule",
                r#"{"actions": [], "mode": "sync"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn schedule_validate_negative_at_frame_returns_structured_error() {
        let (state, _rx) = test_state_enabled();
        let app = build_router(state);
        let response = app
            .oneshot(json_post(
                "/api/v1/schedule",
                r#"{"actions":[{"tool":"pause_time","at_frame":-5}]}"#,
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            body["error"],
            "action[0]: 'at_frame' must be a non-negative integer"
        );
    }
}
