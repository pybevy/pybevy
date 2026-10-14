use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::{
    protocol::{
        CaptureDepthParams, CaptureTimelineParams, CaptureTurnaroundParams, CheckAllOverlapsParams,
        CompareFramesParams, ControlOperation, EntityRef, GetComponentParams, GetResourceParams,
        QueryEntitiesParams, ReloadAndCaptureParams, ReloadParams, RemoveComponentParams,
        SeekTimeParams, SetComponentParams, SetResourceParams,
    },
    schedule::ScheduleRequest,
    transport::server::{
        AppState, BatchBody, CheckOverlapsBody, ErrorResponse, ExecuteBody, MutateAssetBody,
        NeighborhoodBody, OperationBody, ScreenshotBody, SpatialQueryBody, SpawnBody, StatsBody,
        TimeScaleBody, error_response, parse_entity_ref, respond, schedule_status,
        screenshot_operation, send_operation,
    },
};

type Arguments = Map<String, Value>;

/// A tool call answered without reaching the engine.
pub(super) enum Rejection {
    Error(ErrorResponse),
    Json(JsonRejection),
    NoRoute,
}

impl From<ErrorResponse> for Rejection {
    fn from(error: ErrorResponse) -> Self {
        Self::Error(error)
    }
}

impl From<JsonRejection> for Rejection {
    fn from(rejection: JsonRejection) -> Self {
        Self::Json(rejection)
    }
}

impl IntoResponse for Rejection {
    fn into_response(self) -> Response {
        match self {
            Self::Error(error) => error.into_response(),
            Self::Json(rejection) => rejection.into_response(),
            Self::NoRoute => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

/// `POST /api/v1/tools/{name}`: one MCP tool call with its arguments object.
///
/// Arguments follow the MCP tool schema and resolve to the same operation,
/// status and body as the tool's REST route.
pub(super) async fn call_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Json(arguments): Json<Arguments>,
) -> Result<Response, Rejection> {
    let operation = match name.as_str() {
        "get_component" => return get_component(&state, &arguments, &headers).await,
        "get_schedule_result" => {
            let id = last_segment(&arguments, "schedule_id")?;
            return Ok(schedule_status(&state, &id, &headers).into_response());
        }
        "get_component_schema" => ControlOperation::GetComponentSchema {
            name: argument_string(&arguments, "name"),
        },
        "get_resource" => {
            let resource_type =
                non_empty_string(&arguments, "resource_type", "get_resource", "resource_type")?;
            ControlOperation::GetResource(GetResourceParams { resource_type })
        }
        "get_system_list" => ControlOperation::ListSystems {
            include_internal: arguments
                .get("include_internal")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        "despawn_entity" => ControlOperation::DespawnEntity {
            entity: parse_entity_ref(&last_segment(&arguments, "entity")?),
        },
        "set_component" => {
            let entity = entity_segment(&arguments);
            let component = last_segment(&arguments, "component")?;
            ControlOperation::SetComponent(SetComponentParams {
                entity,
                component,
                fields: object_or_empty(&arguments, "fields"),
            })
        }
        "remove_component" => ControlOperation::RemoveComponent(RemoveComponentParams {
            entity: entity_segment(&arguments),
            component: last_segment(&arguments, "component")?,
        }),
        "get_bounding_box" => ControlOperation::GetBoundingBox {
            entity: entity_segment(&arguments),
        },
        "set_resource" => {
            let resource_type = last_segment(&arguments, "resource_type")?;
            ControlOperation::SetResource(SetResourceParams {
                resource_type,
                value: object_or_empty(&arguments, "value"),
            })
        }
        "remove_resource" => ControlOperation::RemoveResource {
            resource_type: last_segment(&arguments, "resource_type")?,
        },
        "set_asset" => operation(MutateAssetBody {
            entity: argument_or_null(&arguments, "entity"),
            component: argument_string(&arguments, "component"),
            asset_type: argument_string(&arguments, "asset_type"),
            fields: object_or_empty(&arguments, "fields"),
        })?,
        "query_spatial" if arguments.contains_key("radius") => {
            body_operation::<NeighborhoodBody>(neighborhood_arguments(&arguments))?
        }
        "query_spatial" => operation(SpatialQueryBody {
            entity_a: argument_or_null(&arguments, "entity_a"),
            entity_b: argument_or_null(&arguments, "entity_b"),
        })?,
        "query_spatial_neighborhood" => {
            body_operation::<NeighborhoodBody>(neighborhood_arguments(&arguments))?
        }
        "check_overlaps"
            if arguments
                .get("entity")
                .is_some_and(|value| !value.is_null()) =>
        {
            let mut body =
                Map::from_iter([("entity".to_string(), argument_or_null(&arguments, "entity"))]);
            copy_if_present(
                &arguments,
                &mut body,
                &[
                    "include_siblings",
                    "max_float_gap",
                    "ground_y",
                    "min_penetration",
                ],
            );
            body_operation::<CheckOverlapsBody>(body)?
        }
        "check_overlaps" | "check_all_overlaps" => {
            body_operation::<CheckAllOverlapsParams>(overlap_all_arguments(&arguments))?
        }
        "capture_screenshot" => {
            let mut body = arguments;
            let gizmos = body
                .remove("gizmos")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let body: ScreenshotBody = parse_body(body)?;
            screenshot_operation(body, gizmos)?
        }
        "get_reload_status" => ControlOperation::GetReloadStatus,
        "get_last_error" => ControlOperation::GetLastError,
        "get_scene_summary" => ControlOperation::GetSceneSummary,
        "pause_time" => ControlOperation::PauseTime,
        "resume_time" => ControlOperation::ResumeTime,
        "get_time_status" => ControlOperation::GetTimeStatus,
        "get_performance" => ControlOperation::GetPerformance,
        "get_registry" => ControlOperation::GetRegistry,
        "query_entities" => body_operation::<QueryEntitiesParams>(arguments)?,
        "capture_timeline" => body_operation::<CaptureTimelineParams>(arguments)?,
        "capture_turnaround" => body_operation::<CaptureTurnaroundParams>(arguments)?,
        "capture_depth" => body_operation::<CaptureDepthParams>(arguments)?,
        "capture_stats" => body_operation::<StatsBody>(arguments)?,
        "compare_frames" => body_operation::<CompareFramesParams>(arguments)?,
        "reload" => body_operation::<ReloadParams>(arguments)?,
        "reload_and_capture" => body_operation::<ReloadAndCaptureParams>(arguments)?,
        "spawn_entity" => body_operation::<SpawnBody>(arguments)?,
        "batch" => body_operation::<BatchBody>(arguments)?,
        "set_time_scale" => body_operation::<TimeScaleBody>(arguments)?,
        "seek_time" => body_operation::<SeekTimeParams>(arguments)?,
        "run_code" => body_operation::<ExecuteBody>(arguments)?,
        "schedule_actions" => body_operation::<ScheduleRequest>(arguments)?,
        _ => return Err(unknown_tool(&name)),
    };
    Ok(respond(&state, operation, &headers).await)
}

/// A named entity is first resolved to its numeric ID, as the MCP client did.
async fn get_component(
    state: &AppState,
    arguments: &Arguments,
    headers: &HeaderMap,
) -> Result<Response, Rejection> {
    let entity = match arguments.get("entity") {
        Some(Value::String(entity_name)) => {
            if entity_name.is_empty() {
                return Err(invalid_argument(
                    "get_component requires a non-empty entity name or numeric ID",
                ));
            }
            None
        }
        entity => Some(entity),
    };
    let component = non_empty_string(arguments, "component", "get_component", "component name")?;
    let entity = match entity {
        Some(entity) => entity.and_then(Value::as_u64).ok_or_else(|| {
            invalid_argument("get_component requires an entity name or numeric ID")
        })?,
        None => resolve_entity_id(state, &argument_string(arguments, "entity")).await?,
    };
    let params = GetComponentParams {
        entity: EntityRef::Id(entity),
        component,
    };
    Ok(respond(state, ControlOperation::GetComponent(params), headers).await)
}

async fn resolve_entity_id(state: &AppState, entity_name: &str) -> Result<u64, Rejection> {
    let lookup = ControlOperation::GetEntity {
        entity: parse_entity_ref(entity_name),
    };
    let lookup = send_operation(&state.sender, lookup).await?;
    let id = lookup.get("id").and_then(Value::as_u64).ok_or_else(|| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Entity lookup response did not contain a valid numeric id",
        )
    })?;
    Ok(id)
}

fn operation<B: OperationBody>(body: B) -> Result<ControlOperation, Rejection> {
    Ok(body.operation()?)
}

fn body_operation<B: OperationBody>(arguments: Arguments) -> Result<ControlOperation, Rejection> {
    operation(parse_body::<B>(arguments)?)
}

/// Parses like the route's `Json` extractor, so rejections keep their status and text.
fn parse_body<B: DeserializeOwned>(arguments: Arguments) -> Result<B, Rejection> {
    let bytes = serde_json::to_vec(&arguments)
        .map_err(|error| error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()))?;
    let Json(body) = Json::<B>::from_bytes(&bytes)?;
    Ok(body)
}

/// The final path segment, stringified as the MCP client did; an empty one
/// matched no route.
fn last_segment(arguments: &Arguments, key: &str) -> Result<String, Rejection> {
    let value = argument_string(arguments, key);
    if value.is_empty() {
        return Err(Rejection::NoRoute);
    }
    Ok(value)
}

fn entity_segment(arguments: &Arguments) -> EntityRef {
    parse_entity_ref(&argument_string(arguments, "entity"))
}

fn argument_string(arguments: &Arguments, key: &str) -> String {
    match arguments.get(key) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(value)) => value.clone(),
        Some(Value::Bool(value)) => {
            if *value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Some(value) => value.to_string(),
    }
}

fn object_or_empty(arguments: &Arguments, key: &str) -> Value {
    arguments.get(key).cloned().unwrap_or_else(|| json!({}))
}

fn argument_or_null(arguments: &Arguments, key: &str) -> Value {
    arguments.get(key).cloned().unwrap_or(Value::Null)
}

fn non_empty_string(
    arguments: &Arguments,
    key: &str,
    operation: &str,
    description: &str,
) -> Result<String, Rejection> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid_argument(&format!("{operation} requires a non-empty {description}")))
}

fn neighborhood_arguments(arguments: &Arguments) -> Arguments {
    let mut body = Map::from_iter([
        ("entity".to_string(), argument_or_null(arguments, "entity")),
        ("radius".to_string(), argument_or_null(arguments, "radius")),
    ]);
    copy_if_present(arguments, &mut body, &["max_results"]);
    body
}

fn overlap_all_arguments(arguments: &Arguments) -> Arguments {
    let mut body = Map::new();
    copy_if_present(
        arguments,
        &mut body,
        &[
            "min_penetration",
            "max_results",
            "max_float_gap",
            "ground_y",
            "include_siblings",
        ],
    );
    body
}

fn copy_if_present(source: &Arguments, destination: &mut Arguments, keys: &[&str]) {
    for key in keys {
        if let Some(value) = source.get(*key) {
            destination.insert((*key).to_string(), value.clone());
        }
    }
}

fn invalid_argument(message: &str) -> Rejection {
    error_response(StatusCode::BAD_REQUEST, message).into()
}

fn unknown_tool(name: &str) -> Rejection {
    error_response(StatusCode::NOT_FOUND, &format!("Unknown tool: {name}")).into()
}

#[cfg(test)]
mod tests {
    use std::{
        mem,
        path::Path as FilePath,
        sync::{Arc, Mutex},
    };

    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Method, Request},
    };
    use tower::ServiceExt;

    use super::*;
    use crate::{
        catalog::tools::{is_engine_tool_name, list_tools},
        discovery::ApiIndex,
        engine::channel::create_channel,
        plugin::ControlConfig,
        schedule::SharedScheduleRegistry,
        transport::{
            image_delivery::{MCP_IMAGE_DELIVERY_HEADER, MCP_IMAGE_DELIVERY_VALUE},
            server::build_router,
            sse::SseEventBroadcaster,
        },
    };

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

    type OperationLog = Arc<Mutex<Vec<String>>>;

    struct Exchange {
        status: StatusCode,
        body: String,
        operations: Vec<String>,
    }

    fn engine_router() -> (Router, OperationLog) {
        let (sender, mut receiver) = create_channel();
        let state = AppState::new(
            sender,
            SseEventBroadcaster::new(),
            Arc::new(ApiIndex::build(FilePath::new(""))),
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
        let log = OperationLog::default();
        let engine_log = log.clone();
        tokio::spawn(async move {
            while let Some(request) = receiver.rx.recv().await {
                engine_log
                    .lock()
                    .unwrap()
                    .push(format!("{:?}", request.operation));
                let _ = request.response_tx.send(Ok(json!({
                    "id": 42,
                    "image": PNG,
                    "width": 1,
                    "height": 1,
                    "format": "png",
                    "encoding": "base64",
                })));
            }
        });
        (build_router(state), log)
    }

    fn request(method: Method, uri: &str, body: Option<Value>) -> Request<Body> {
        let builder = Request::builder()
            .method(method)
            .uri(uri)
            .header(MCP_IMAGE_DELIVERY_HEADER, MCP_IMAGE_DELIVERY_VALUE);
        match body {
            Some(body) => builder
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        }
    }

    async fn exchange(
        router: &Router,
        log: &OperationLog,
        requests: Vec<Request<Body>>,
    ) -> Exchange {
        log.lock().unwrap().clear();
        let mut last = None;
        for request in requests {
            last = Some(router.clone().oneshot(request).await.unwrap());
        }
        let response = last.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        Exchange {
            status,
            body: String::from_utf8(body.to_vec()).unwrap(),
            operations: mem::take(&mut *log.lock().unwrap()),
        }
    }

    async fn call(router: &Router, log: &OperationLog, tool: &str, arguments: Value) -> Exchange {
        let uri = format!("/api/v1/tools/{tool}");
        exchange(
            router,
            log,
            vec![request(Method::POST, &uri, Some(arguments))],
        )
        .await
    }

    fn get(uri: &str) -> Request<Body> {
        request(Method::GET, uri, None)
    }

    fn post(uri: &str, body: Value) -> Request<Body> {
        request(Method::POST, uri, Some(body))
    }

    fn put(uri: &str, body: Value) -> Request<Body> {
        request(Method::PUT, uri, Some(body))
    }

    fn delete(uri: &str) -> Request<Body> {
        request(Method::DELETE, uri, None)
    }

    /// Each case pairs an MCP tool call with the REST requests the MCP client
    /// used to send for it.
    fn route_cases() -> Vec<(&'static str, Value, Vec<Request<Body>>)> {
        vec![
            (
                "get_component",
                json!({"entity": 42, "component": "Spot Light"}),
                vec![get("/api/v1/entities/42/components/Spot%20Light")],
            ),
            (
                "get_component",
                json!({"entity": "Lighthouse / 1", "component": "Spot Light"}),
                vec![
                    get("/api/v1/entities/Lighthouse%20%2F%201"),
                    get("/api/v1/entities/42/components/Spot%20Light"),
                ],
            ),
            (
                "get_component",
                json!({"entity": "7", "component": "Transform"}),
                vec![
                    get("/api/v1/entities/7"),
                    get("/api/v1/entities/42/components/Transform"),
                ],
            ),
            (
                "get_component_schema",
                json!({"name": "State[game.Phase]"}),
                vec![get("/api/v1/components/State%5Bgame.Phase%5D/schema")],
            ),
            (
                "get_component_schema",
                json!({}),
                vec![get("/api/v1/components//schema")],
            ),
            (
                "get_resource",
                json!({"resource_type": "State[game.Phase]"}),
                vec![get("/api/v1/resources/State%5Bgame.Phase%5D")],
            ),
            (
                "get_system_list",
                json!({"include_internal": true}),
                vec![get("/api/v1/systems?include_internal=true")],
            ),
            (
                "get_system_list",
                json!({"include_internal": "yes"}),
                vec![get("/api/v1/systems?include_internal=false")],
            ),
            (
                "despawn_entity",
                json!({"entity": "named/entity"}),
                vec![delete("/api/v1/entities/named%2Fentity")],
            ),
            (
                "despawn_entity",
                json!({"entity": "42"}),
                vec![delete("/api/v1/entities/42")],
            ),
            (
                "despawn_entity",
                json!({"entity": true}),
                vec![delete("/api/v1/entities/True")],
            ),
            (
                "despawn_entity",
                json!({"entity": 1.5}),
                vec![delete("/api/v1/entities/1.5")],
            ),
            (
                "despawn_entity",
                json!({}),
                vec![delete("/api/v1/entities/")],
            ),
            (
                "get_bounding_box",
                json!({"entity": null}),
                vec![get("/api/v1/entities//bounding_box")],
            ),
            (
                "set_component",
                json!({"component": "Transform", "fields": {}}),
                vec![put(
                    "/api/v1/entities//components/Transform",
                    json!({"fields": {}}),
                )],
            ),
            (
                "remove_component",
                json!({"entity": 42}),
                vec![delete("/api/v1/entities/42/components/")],
            ),
            (
                "remove_resource",
                json!({}),
                vec![delete("/api/v1/resources/")],
            ),
            (
                "get_schedule_result",
                json!({}),
                vec![get("/api/v1/schedule/")],
            ),
            (
                "set_component",
                json!({"entity": 42, "component": "Transform", "fields": {"x": 1}, "extra": 1}),
                vec![put(
                    "/api/v1/entities/42/components/Transform",
                    json!({"fields": {"x": 1}}),
                )],
            ),
            (
                "set_component",
                json!({"entity": "Cube", "component": "Transform"}),
                vec![put(
                    "/api/v1/entities/Cube/components/Transform",
                    json!({"fields": {}}),
                )],
            ),
            (
                "remove_component",
                json!({"entity": 42, "component": "Transform"}),
                vec![delete("/api/v1/entities/42/components/Transform")],
            ),
            (
                "get_bounding_box",
                json!({"entity": "Main Camera"}),
                vec![get("/api/v1/entities/Main%20Camera/bounding_box")],
            ),
            (
                "set_resource",
                json!({"resource_type": "ClearColor", "value": {"color": [1.0, 1.0, 1.0, 1.0]}}),
                vec![put(
                    "/api/v1/resources/ClearColor",
                    json!({"value": {"color": [1.0, 1.0, 1.0, 1.0]}}),
                )],
            ),
            (
                "remove_resource",
                json!({"resource_type": "ClearColor"}),
                vec![delete("/api/v1/resources/ClearColor")],
            ),
            (
                "set_asset",
                json!({
                    "entity": "Cube",
                    "component": "MeshMaterial3d",
                    "asset_type": "StandardMaterial",
                    "fields": {"roughness": 0.5},
                    "ignored": true,
                }),
                vec![post(
                    "/api/v1/assets/mutate",
                    json!({
                        "entity": "Cube",
                        "component": "MeshMaterial3d",
                        "asset_type": "StandardMaterial",
                        "fields": {"roughness": 0.5},
                    }),
                )],
            ),
            (
                "set_asset",
                json!({"entity": [1], "component": "MeshMaterial3d", "asset_type": "StandardMaterial"}),
                vec![post(
                    "/api/v1/assets/mutate",
                    json!({
                        "entity": [1],
                        "component": "MeshMaterial3d",
                        "asset_type": "StandardMaterial",
                        "fields": {},
                    }),
                )],
            ),
            (
                "get_schedule_result",
                json!({"schedule_id": "capture/1"}),
                vec![get("/api/v1/schedule/capture%2F1")],
            ),
            (
                "query_spatial",
                json!({"entity_a": "Cube", "entity_b": "Sphere", "ignored": 1}),
                vec![post(
                    "/api/v1/spatial/query",
                    json!({"entity_a": "Cube", "entity_b": "Sphere"}),
                )],
            ),
            (
                "query_spatial",
                json!({"entity_a": true, "entity_b": "Sphere"}),
                vec![post(
                    "/api/v1/spatial/query",
                    json!({"entity_a": true, "entity_b": "Sphere"}),
                )],
            ),
            (
                "query_spatial",
                json!({"entity": "Cube", "radius": 10.0, "max_results": 5}),
                vec![post(
                    "/api/v1/spatial/neighborhood",
                    json!({"entity": "Cube", "radius": 10.0, "max_results": 5}),
                )],
            ),
            (
                "query_spatial_neighborhood",
                json!({"entity": "Cube", "radius": "far"}),
                vec![post(
                    "/api/v1/spatial/neighborhood",
                    json!({"entity": "Cube", "radius": "far"}),
                )],
            ),
            (
                "check_overlaps",
                json!({"entity": "Cube", "include_siblings": true, "ground_y": 0.0}),
                vec![post(
                    "/api/v1/spatial/overlaps",
                    json!({"entity": "Cube", "include_siblings": true, "ground_y": 0.0}),
                )],
            ),
            (
                "check_overlaps",
                json!({"entity": null, "min_penetration": 0.01, "max_results": 50}),
                vec![post(
                    "/api/v1/spatial/overlaps/all",
                    json!({"min_penetration": 0.01, "max_results": 50}),
                )],
            ),
            (
                "check_all_overlaps",
                json!({"max_float_gap": 0.5, "include_siblings": false}),
                vec![post(
                    "/api/v1/spatial/overlaps/all",
                    json!({"max_float_gap": 0.5, "include_siblings": false}),
                )],
            ),
            (
                "capture_screenshot",
                json!({"gizmos": true, "max_width": 100}),
                vec![post("/api/v1/screenshot/gizmos", json!({"max_width": 100}))],
            ),
            (
                "capture_screenshot",
                json!({"delay_frames": -5}),
                vec![post("/api/v1/screenshot", json!({"delay_frames": -5}))],
            ),
            (
                "capture_stats",
                json!({"grid": 2, "max_width": 0}),
                vec![post(
                    "/api/v1/screenshot/stats",
                    json!({"grid": 2, "max_width": 0}),
                )],
            ),
            (
                "capture_turnaround",
                json!({"views": 3}),
                vec![post("/api/v1/screenshot/turnaround", json!({"views": 3}))],
            ),
            (
                "capture_timeline",
                json!({}),
                vec![post("/api/v1/screenshot/timeline", json!({}))],
            ),
            (
                "capture_depth",
                json!({}),
                vec![post("/api/v1/screenshot/depth", json!({}))],
            ),
            (
                "compare_frames",
                json!({"a": "f_1", "b": "f_2"}),
                vec![post(
                    "/api/v1/screenshot/compare",
                    json!({"a": "f_1", "b": "f_2"}),
                )],
            ),
            (
                "reload",
                json!({"mode": "partial"}),
                vec![post("/api/v1/reload", json!({"mode": "partial"}))],
            ),
            (
                "reload_and_capture",
                json!({}),
                vec![post("/api/v1/reload/capture", json!({}))],
            ),
            (
                "query_entities",
                json!({"with": ["Name"]}),
                vec![post("/api/v1/query", json!({"with": ["Name"]}))],
            ),
            (
                "spawn_entity",
                json!({"components": {}}),
                vec![post("/api/v1/entities", json!({"components": {}}))],
            ),
            (
                "batch",
                json!({"operations": []}),
                vec![post("/api/v1/batch", json!({"operations": []}))],
            ),
            (
                "run_code",
                json!({"code": "print(1)"}),
                vec![post("/api/v1/execute", json!({"code": "print(1)"}))],
            ),
            (
                "set_time_scale",
                json!({"scale": 0.5}),
                vec![post("/api/v1/time/scale", json!({"scale": 0.5}))],
            ),
            (
                "seek_time",
                json!({"seconds": 1.0}),
                vec![post("/api/v1/time/seek", json!({"seconds": 1.0}))],
            ),
            (
                "pause_time",
                json!({"probe": true}),
                vec![post("/api/v1/time/pause", json!({"probe": true}))],
            ),
            (
                "resume_time",
                json!({}),
                vec![post("/api/v1/time/resume", json!({}))],
            ),
            (
                "schedule_actions",
                json!({"actions": [{"tool": "pause_time"}], "mode": "async"}),
                vec![post(
                    "/api/v1/schedule",
                    json!({"actions": [{"tool": "pause_time"}], "mode": "async"}),
                )],
            ),
            (
                "schedule_actions",
                json!({"actions": [], "mode": "sync"}),
                vec![post(
                    "/api/v1/schedule",
                    json!({"actions": [], "mode": "sync"}),
                )],
            ),
            (
                "get_reload_status",
                json!({}),
                vec![get("/api/v1/reload/status")],
            ),
            (
                "get_last_error",
                json!({"probe": 1}),
                vec![get("/api/v1/error")],
            ),
            (
                "get_scene_summary",
                json!({}),
                vec![get("/api/v1/scene/summary")],
            ),
            ("get_time_status", json!({}), vec![get("/api/v1/time")]),
            (
                "get_performance",
                json!({}),
                vec![get("/api/v1/performance")],
            ),
            (
                "get_registry",
                json!({}),
                vec![get("/api/v1/debug/registry")],
            ),
        ]
    }

    #[tokio::test]
    async fn tool_calls_match_the_routes_the_client_used() {
        let (router, log) = engine_router();
        for (tool, arguments, route_requests) in route_cases() {
            let label = format!("{tool} {arguments}");
            let via_tool = call(&router, &log, tool, arguments).await;
            let via_route = exchange(&router, &log, route_requests).await;
            assert_eq!(via_tool.status, via_route.status, "{label}");
            assert_eq!(via_tool.body, via_route.body, "{label}");
            assert_eq!(via_tool.operations, via_route.operations, "{label}");
        }
    }

    #[tokio::test]
    async fn tool_calls_keep_statuses_and_image_delivery() {
        let (router, log) = engine_router();
        for (tool, arguments, status, operations, image_delivery) in [
            ("spawn_entity", json!({"components": {}}), 201, 1, false),
            (
                "schedule_actions",
                json!({"actions": [{"tool": "pause_time"}], "mode": "async"}),
                202,
                1,
                true,
            ),
            ("schedule_actions", json!({"actions": []}), 400, 0, false),
            ("capture_screenshot", json!({}), 200, 1, true),
            ("capture_stats", json!({}), 200, 1, false),
            ("capture_turnaround", json!({"views": 3}), 422, 0, false),
            (
                "query_spatial",
                json!({"entity_a": [1], "entity_b": 1}),
                400,
                0,
                false,
            ),
            ("despawn_entity", json!({}), 404, 0, false),
            (
                "get_component",
                json!({"entity": "Lamp", "component": "Transform"}),
                200,
                2,
                false,
            ),
        ] {
            let result = call(&router, &log, tool, arguments).await;
            assert_eq!(result.status.as_u16(), status, "{tool}: {}", result.body);
            assert_eq!(result.operations.len(), operations, "{tool}");
            assert_eq!(
                result.body.contains("image_delivery"),
                image_delivery,
                "{tool}: {}",
                result.body
            );
        }
    }

    #[tokio::test]
    async fn tool_calls_validate_get_arguments() {
        let (router, log) = engine_router();
        for (tool, arguments, message) in [
            (
                "get_component",
                json!({"entity": "", "component": "Transform"}),
                "get_component requires a non-empty entity name or numeric ID",
            ),
            (
                "get_component",
                json!({"entity": 1, "component": ""}),
                "get_component requires a non-empty component name",
            ),
            (
                "get_component",
                json!({"entity": true, "component": "Transform"}),
                "get_component requires an entity name or numeric ID",
            ),
            (
                "get_resource",
                json!({"resource_type": ""}),
                "get_resource requires a non-empty resource_type",
            ),
        ] {
            let result = call(&router, &log, tool, arguments).await;
            assert_eq!(result.status, StatusCode::BAD_REQUEST, "{tool}");
            assert_eq!(
                serde_json::from_str::<Value>(&result.body).unwrap(),
                json!({"error": message})
            );
            assert!(result.operations.is_empty(), "{tool}");
        }
    }

    #[tokio::test]
    async fn every_engine_tool_is_served_and_others_are_unknown() {
        let (router, log) = engine_router();
        let mut names: Vec<String> = list_tools()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_owned())
            .collect();
        names.push("get_schedule_result".to_owned());
        for name in names {
            assert!(is_engine_tool_name(&name), "{name}");
            let result = call(&router, &log, &name, json!({})).await;
            assert!(
                !result.body.contains("Unknown tool"),
                "{name}: {}",
                result.body
            );
        }
        for name in [
            "get_entity",
            "list_entities",
            "capture_with_gizmos",
            "run_scene",
        ] {
            assert!(!is_engine_tool_name(name), "{name}");
            let result = call(&router, &log, name, json!({})).await;
            assert_eq!(result.status, StatusCode::NOT_FOUND, "{name}");
            assert_eq!(
                serde_json::from_str::<Value>(&result.body).unwrap(),
                json!({"error": format!("Unknown tool: {name}")})
            );
        }
    }
}
