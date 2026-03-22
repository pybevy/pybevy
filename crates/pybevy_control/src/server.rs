use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query as AxumQuery, State},
    http::StatusCode,
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use tokio::sync::oneshot;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;

use crate::{
    api_index::ApiIndex,
    bridge::{
        ControlOperation, ControlRequest, ControlSender, EntityRef, SharedLatestError,
        SseEventBroadcaster,
    },
};

/// Shared state for the Axum server
#[derive(Clone)]
pub struct AppState {
    pub sender: ControlSender,
    pub sse_broadcaster: SseEventBroadcaster,
    pub api_index: Arc<ApiIndex>,
    pub config: ServerConfig,
    pub latest_error: SharedLatestError,
    pub schedule_registry: crate::handlers::schedule::SharedScheduleRegistry,
}

impl AppState {
    pub fn new(
        sender: ControlSender,
        sse_broadcaster: SseEventBroadcaster,
        api_index: Arc<ApiIndex>,
        config: ServerConfig,
        latest_error: SharedLatestError,
        schedule_registry: crate::handlers::schedule::SharedScheduleRegistry,
    ) -> Self {
        Self {
            sender,
            sse_broadcaster,
            api_index,
            config,
            latest_error,
            schedule_registry,
        }
    }
}

#[derive(Clone)]
pub struct ServerConfig {
    pub screenshot_enabled: bool,
    pub manipulation_enabled: bool,
    pub execute_python_enabled: bool,
    pub api_discovery_enabled: bool,
}

// ── Helper: send operation to Bevy world and await response ──────────────

async fn send_operation(
    sender: &ControlSender,
    operation: ControlOperation,
) -> Result<serde_json::Value, (StatusCode, Json<serde_json::Value>)> {
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
                -32001 => StatusCode::NOT_FOUND,
                -32602 => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
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

fn parse_entity_ref(entity: &str) -> EntityRef {
    match entity.parse::<u64>() {
        Ok(id) => EntityRef::Id(id),
        Err(_) => EntityRef::Name(entity.to_string()),
    }
}

// ── Router ───────────────────────────────────────────────────────────────

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
        // Custom tools
        .route("/api/v1/tools/{name}", post(call_custom_tool))
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
        .layer(CorsLayer::permissive())
        .with_state(state)
}

// ── Health & SSE ─────────────────────────────────────────────────────────

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

// ── Entity routes ────────────────────────────────────────────────────────

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
struct SpawnBody {
    components: serde_json::Value,
}

async fn spawn_entity(
    State(state): State<AppState>,
    Json(body): Json<SpawnBody>,
) -> impl IntoResponse {
    if !state.config.manipulation_enabled {
        return error_response(StatusCode::FORBIDDEN, "Manipulation disabled");
    }
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
    if !state.config.manipulation_enabled {
        return error_response(StatusCode::FORBIDDEN, "Manipulation disabled");
    }
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
        ControlOperation::GetComponent {
            entity: parse_entity_ref(&entity),
            component,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
struct SetComponentBody {
    fields: serde_json::Value,
}

async fn set_component(
    State(state): State<AppState>,
    Path((entity, component)): Path<(String, String)>,
    Json(body): Json<SetComponentBody>,
) -> impl IntoResponse {
    if !state.config.manipulation_enabled {
        return error_response(StatusCode::FORBIDDEN, "Manipulation disabled");
    }
    match send_operation(
        &state.sender,
        ControlOperation::SetComponent {
            entity: parse_entity_ref(&entity),
            component,
            fields: body.fields,
        },
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
    if !state.config.manipulation_enabled {
        return error_response(StatusCode::FORBIDDEN, "Manipulation disabled");
    }
    match send_operation(
        &state.sender,
        ControlOperation::RemoveComponent {
            entity: parse_entity_ref(&entity),
            component,
        },
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

// ── Query ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct QueryEntitiesBody {
    #[serde(default)]
    with: Vec<String>,
    #[serde(default)]
    without: Vec<String>,
}

async fn query_entities(
    State(state): State<AppState>,
    Json(body): Json<QueryEntitiesBody>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::QueryEntities {
            with: body.with,
            without: body.without,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn scene_summary(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::SceneSummary).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

// ── Resources ────────────────────────────────────────────────────────────

async fn list_resources(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::ListResources).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
struct InsertResourceBody {
    value: serde_json::Value,
}

async fn insert_resource(
    State(state): State<AppState>,
    Path(resource_type): Path<String>,
    Json(body): Json<InsertResourceBody>,
) -> impl IntoResponse {
    if !state.config.manipulation_enabled {
        return error_response(StatusCode::FORBIDDEN, "Manipulation disabled");
    }
    match send_operation(
        &state.sender,
        ControlOperation::InsertResource {
            resource_type,
            value: body.value,
        },
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
    if !state.config.manipulation_enabled {
        return error_response(StatusCode::FORBIDDEN, "Manipulation disabled");
    }
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

// ── Systems & Component schema ───────────────────────────────────────────

async fn list_systems(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::ListSystems).await {
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

// ── Screenshot routes ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ScreenshotBody {
    #[serde(default = "default_delay_frames")]
    delay_frames: u32,
    max_width: Option<u32>,
    position: Option<[f32; 3]>,
    look_at: Option<[f32; 3]>,
    #[serde(default)]
    hide_ui: bool,
}

fn default_delay_frames() -> u32 {
    2
}

async fn capture_screenshot(
    State(state): State<AppState>,
    Json(body): Json<ScreenshotBody>,
) -> impl IntoResponse {
    if !state.config.screenshot_enabled {
        return error_response(StatusCode::FORBIDDEN, "Screenshot disabled");
    }
    match send_operation(
        &state.sender,
        ControlOperation::CaptureScreenshot {
            delay_frames: body.delay_frames,
            max_width: body.max_width,
            position: body.position,
            look_at: body.look_at,
            hide_ui: body.hide_ui,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn capture_with_gizmos(
    State(state): State<AppState>,
    Json(body): Json<ScreenshotBody>,
) -> impl IntoResponse {
    if !state.config.screenshot_enabled {
        return error_response(StatusCode::FORBIDDEN, "Screenshot disabled");
    }
    match send_operation(
        &state.sender,
        ControlOperation::CaptureWithGizmos {
            delay_frames: body.delay_frames,
            max_width: body.max_width,
            position: body.position,
            look_at: body.look_at,
            hide_ui: body.hide_ui,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
struct TimelineBody {
    #[serde(default = "default_total_frames")]
    total_frames: u32,
    #[serde(default = "default_capture_count")]
    capture_count: u32,
    max_width: Option<u32>,
    #[serde(default = "default_columns")]
    columns: u32,
    position: Option<[f32; 3]>,
    look_at: Option<[f32; 3]>,
}

fn default_total_frames() -> u32 {
    120
}
fn default_capture_count() -> u32 {
    6
}
fn default_columns() -> u32 {
    3
}

async fn capture_timeline(
    State(state): State<AppState>,
    Json(body): Json<TimelineBody>,
) -> impl IntoResponse {
    if !state.config.screenshot_enabled {
        return error_response(StatusCode::FORBIDDEN, "Screenshot disabled");
    }
    match send_operation(
        &state.sender,
        ControlOperation::CaptureTimeline {
            total_frames: body.total_frames,
            capture_count: body.capture_count,
            max_width: body.max_width,
            columns: body.columns,
            position: body.position,
            look_at: body.look_at,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
struct TurnaroundBody {
    look_at: Option<[f32; 3]>,
    distance: Option<f32>,
    elevation: Option<f32>,
    view_count: Option<u32>,
    include_top: Option<bool>,
    columns: Option<u32>,
    max_width: Option<u32>,
    hide_ui: Option<bool>,
}

async fn capture_turnaround(
    State(state): State<AppState>,
    Json(body): Json<TurnaroundBody>,
) -> impl IntoResponse {
    if !state.config.screenshot_enabled {
        return error_response(StatusCode::FORBIDDEN, "Screenshot disabled");
    }
    match send_operation(
        &state.sender,
        ControlOperation::CaptureTurnaround {
            look_at: body.look_at,
            distance: body.distance,
            elevation: body.elevation,
            view_count: body.view_count,
            include_top: body.include_top,
            columns: body.columns,
            max_width: body.max_width,
            hide_ui: body.hide_ui,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
struct DepthBody {
    position: Option<[f32; 3]>,
    look_at: Option<[f32; 3]>,
    sample_points: Option<Vec<[u32; 2]>>,
    grid_density: Option<u32>,
    include_rgb: Option<bool>,
    delay_frames: Option<u32>,
    hide_ui: Option<bool>,
    max_width: Option<u32>,
}

async fn capture_depth(
    State(state): State<AppState>,
    Json(body): Json<DepthBody>,
) -> impl IntoResponse {
    if !state.config.screenshot_enabled {
        return error_response(StatusCode::FORBIDDEN, "Screenshot disabled");
    }
    match send_operation(
        &state.sender,
        ControlOperation::CaptureDepth {
            position: body.position,
            look_at: body.look_at,
            sample_points: body.sample_points,
            grid_density: body.grid_density,
            include_rgb: body.include_rgb,
            delay_frames: body.delay_frames,
            hide_ui: body.hide_ui,
            max_width: body.max_width,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

// ── Reload routes ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ReloadBody {
    #[serde(default = "default_reload_mode")]
    mode: String,
    #[serde(default)]
    pause: bool,
    time_scale: Option<f32>,
}

fn default_reload_mode() -> String {
    "full".to_string()
}

async fn trigger_reload(
    State(state): State<AppState>,
    Json(body): Json<ReloadBody>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::TriggerReload {
            mode: body.mode,
            pause: body.pause,
            time_scale: body.time_scale,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

async fn get_reload_status(State(state): State<AppState>) -> impl IntoResponse {
    match send_operation(&state.sender, ControlOperation::GetReloadStatus).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
struct ReloadAndCaptureBody {
    #[serde(default = "default_reload_mode")]
    mode: String,
    #[serde(default)]
    pause: bool,
    time_scale: Option<f32>,
    delay_frames: Option<u32>,
    max_width: Option<u32>,
    position: Option<[f32; 3]>,
    look_at: Option<[f32; 3]>,
    hide_ui: Option<bool>,
}

async fn reload_and_capture(
    State(state): State<AppState>,
    Json(body): Json<ReloadAndCaptureBody>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::ReloadAndCapture {
            mode: body.mode,
            pause: body.pause,
            time_scale: body.time_scale,
            delay_frames: body.delay_frames,
            max_width: body.max_width,
            position: body.position,
            look_at: body.look_at,
            hide_ui: body.hide_ui,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

// ── Execute Python ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ExecuteBody {
    code: String,
}

async fn execute_python(
    State(state): State<AppState>,
    Json(body): Json<ExecuteBody>,
) -> impl IntoResponse {
    if !state.config.execute_python_enabled {
        return error_response(StatusCode::FORBIDDEN, "Python execution disabled");
    }
    match send_operation(
        &state.sender,
        ControlOperation::ExecutePython { code: body.code },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

// ── Time control routes ──────────────────────────────────────────────────

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

#[derive(Deserialize)]
struct SeekTimeBody {
    seconds: f64,
    #[serde(default)]
    pause: bool,
}

async fn seek_time(
    State(state): State<AppState>,
    Json(body): Json<SeekTimeBody>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::SeekTime {
            seconds: body.seconds,
            pause: body.pause,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

// ── Asset mutation ───────────────────────────────────────────────────────

#[derive(Deserialize)]
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
    if !state.config.manipulation_enabled {
        return error_response(StatusCode::FORBIDDEN, "Manipulation disabled");
    }
    let entity: EntityRef = match serde_json::from_value(body.entity) {
        Ok(e) => e,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid entity reference"),
    };
    match send_operation(
        &state.sender,
        ControlOperation::MutateAsset {
            entity,
            component: body.component,
            asset_type: body.asset_type,
            fields: body.fields,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

// ── Spatial queries ──────────────────────────────────────────────────────

#[derive(Deserialize)]
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
        ControlOperation::QuerySpatial { entity_a, entity_b },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
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
        ControlOperation::QuerySpatialNeighborhood {
            entity,
            radius: body.radius,
            max_results: body.max_results,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
struct CheckOverlapsBody {
    entity: serde_json::Value,
    #[serde(default)]
    include_siblings: bool,
    #[serde(default)]
    max_float_gap: f32,
    ground_y: Option<f32>,
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
        ControlOperation::CheckOverlaps {
            entity,
            include_siblings: body.include_siblings,
            max_float_gap: body.max_float_gap,
            ground_y: body.ground_y,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

#[derive(Deserialize)]
struct CheckAllOverlapsBody {
    min_penetration: Option<f32>,
    max_results: Option<usize>,
    #[serde(default)]
    max_float_gap: f32,
    ground_y: Option<f32>,
    #[serde(default)]
    include_siblings: bool,
}

async fn check_all_overlaps(
    State(state): State<AppState>,
    Json(body): Json<CheckAllOverlapsBody>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::CheckAllOverlaps {
            min_penetration: body.min_penetration,
            max_results: body.max_results,
            max_float_gap: body.max_float_gap,
            ground_y: body.ground_y,
            include_siblings: body.include_siblings,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

// ── Performance, error, debug ────────────────────────────────────────────

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
    match send_operation(&state.sender, ControlOperation::DebugRegistry).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

// ── Batch ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct BatchBody {
    operations: Vec<serde_json::Value>,
}

async fn batch_mutate(
    State(state): State<AppState>,
    Json(body): Json<BatchBody>,
) -> impl IntoResponse {
    if !state.config.manipulation_enabled {
        return error_response(StatusCode::FORBIDDEN, "Manipulation disabled");
    }
    match send_operation(
        &state.sender,
        ControlOperation::BatchMutate {
            operations: body.operations,
        },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

// ── Custom tools ─────────────────────────────────────────────────────────

// ── Schedule ─────────────────────────────────────────────────────────────

async fn submit_schedule(
    State(state): State<AppState>,
    Json(body): Json<crate::handlers::schedule::ScheduleRequest>,
) -> impl IntoResponse {
    // Validate before sending to engine
    if let Err(e) = crate::handlers::schedule::validate_schedule(&body) {
        return error_response(StatusCode::BAD_REQUEST, &e);
    }

    // For sync mode, compute a dynamic timeout from max `at` value
    let is_async = body.mode == "async";

    match send_operation(
        &state.sender,
        ControlOperation::SubmitSchedule { request: body },
    )
    .await
    {
        Ok(v) => {
            if is_async {
                (StatusCode::ACCEPTED, Json(v))
            } else {
                (StatusCode::OK, Json(v))
            }
        }
        Err(e) => e,
    }
}

async fn get_schedule_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.schedule_registry.get(&id) {
        Some(status) => {
            let json = serde_json::to_value(&status).unwrap_or_default();
            (StatusCode::OK, Json(json))
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

// ── Custom tools ────────────────────────────────────────────────────────

async fn call_custom_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(arguments): Json<serde_json::Value>,
) -> impl IntoResponse {
    match send_operation(
        &state.sender,
        ControlOperation::CallCustomTool { name, arguments },
    )
    .await
    {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => e,
    }
}

// ── Plugin configs ───────────────────────────────────────────────────────

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

// ── API discovery (served directly, no World access) ─────────────────────

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn stubs_index(State(state): State<AppState>) -> impl IntoResponse {
    if !state.config.api_discovery_enabled {
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
    if !state.config.api_discovery_enabled {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "API discovery disabled" })),
        );
    }
    let results = state.api_index.search(&params.q);
    (
        StatusCode::OK,
        Json(serde_json::to_value(&results).unwrap_or_default()),
    )
}

async fn stubs_module(
    State(state): State<AppState>,
    Path(module): Path<String>,
) -> impl IntoResponse {
    if !state.config.api_discovery_enabled {
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
    if !state.config.api_discovery_enabled {
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
            Json(serde_json::json!({ "error": format!("Type '{type_name}' not found") })),
        ),
    }
}

async fn stubs_type_structured(
    State(state): State<AppState>,
    Path(type_name): Path<String>,
) -> impl IntoResponse {
    if !state.config.api_discovery_enabled {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "API discovery disabled" })),
        );
    }
    match state.api_index.get_type_definition_structured(&type_name) {
        Some(structured) => (StatusCode::OK, Json(structured)),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Type '{type_name}' not found") })),
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

// ── Server startup ───────────────────────────────────────────────────────

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
