use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{Map, Value};

use crate::{
    catalog::tools::is_engine_tool_name,
    transport::server::{AppState, error_response, respond_to_tool, schedule_status},
};

/// `POST /api/v1/tools/{name}`: one MCP tool call with its arguments object.
///
/// Arguments deserialize into the tool's operation exactly as a scheduled
/// action's do; malformed arguments answer 422 with the deserializer's message.
pub(super) async fn call_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Json(arguments): Json<Map<String, Value>>,
) -> Response {
    if !is_engine_tool_name(&name) {
        return error_response(StatusCode::NOT_FOUND, &format!("Unknown tool: {name}"))
            .into_response();
    }
    if name == "get_schedule_result" {
        return match arguments.get("schedule_id").and_then(Value::as_str) {
            Some(id) if !id.is_empty() => schedule_status(&state, id, &headers).into_response(),
            _ => error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "get_schedule_result requires a non-empty schedule_id",
            )
            .into_response(),
        };
    }
    respond_to_tool(&state, &name, arguments, &headers).await
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
    use serde_json::json;
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
                token: None,
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
    fn route_cases() -> Vec<(&'static str, Value, Request<Body>)> {
        vec![
            (
                "get_component",
                json!({"entity": 42, "component": "Spot Light"}),
                get("/api/v1/entities/42/components/Spot%20Light"),
            ),
            (
                "get_component",
                json!({"entity": "Lighthouse / 1", "component": "Spot Light"}),
                get("/api/v1/entities/Lighthouse%20%2F%201/components/Spot%20Light"),
            ),
            (
                "get_component",
                json!({"entity": "7", "component": "Transform"}),
                get("/api/v1/entities/7/components/Transform"),
            ),
            (
                "get_component_schema",
                json!({"name": "State[game.Phase]"}),
                get("/api/v1/components/State%5Bgame.Phase%5D/schema"),
            ),
            (
                "get_resource",
                json!({"resource_type": "State[game.Phase]"}),
                get("/api/v1/resources/State%5Bgame.Phase%5D"),
            ),
            (
                "get_system_list",
                json!({"include_internal": true}),
                get("/api/v1/systems?include_internal=true"),
            ),
            (
                "despawn_entity",
                json!({"entity": "named/entity"}),
                delete("/api/v1/entities/named%2Fentity"),
            ),
            (
                "despawn_entity",
                json!({"entity": "42"}),
                delete("/api/v1/entities/42"),
            ),
            (
                "set_component",
                json!({"entity": "Cube", "component": "Transform", "fields": {"x": 1}}),
                put(
                    "/api/v1/entities/Cube/components/Transform",
                    json!({"fields": {"x": 1}}),
                ),
            ),
            (
                "remove_component",
                json!({"entity": 42, "component": "Transform"}),
                delete("/api/v1/entities/42/components/Transform"),
            ),
            (
                "get_bounding_box",
                json!({"entity": "Main Camera"}),
                get("/api/v1/entities/Main%20Camera/bounding_box"),
            ),
            (
                "set_resource",
                json!({"resource_type": "ClearColor", "value": {"color": [1.0, 1.0, 1.0, 1.0]}}),
                put(
                    "/api/v1/resources/ClearColor",
                    json!({"value": {"color": [1.0, 1.0, 1.0, 1.0]}}),
                ),
            ),
            (
                "remove_resource",
                json!({"resource_type": "ClearColor"}),
                delete("/api/v1/resources/ClearColor"),
            ),
            (
                "set_asset",
                json!({
                    "entity": "Cube",
                    "component": "MeshMaterial3d",
                    "asset_type": "StandardMaterial",
                    "fields": {"roughness": 0.5},
                }),
                post(
                    "/api/v1/assets/mutate",
                    json!({
                        "entity": "Cube",
                        "component": "MeshMaterial3d",
                        "asset_type": "StandardMaterial",
                        "fields": {"roughness": 0.5},
                    }),
                ),
            ),
            (
                "get_schedule_result",
                json!({"schedule_id": "capture/1"}),
                get("/api/v1/schedule/capture%2F1"),
            ),
            (
                "query_spatial",
                json!({"entity_a": "Cube", "entity_b": "Sphere"}),
                post(
                    "/api/v1/spatial/query",
                    json!({"entity_a": "Cube", "entity_b": "Sphere"}),
                ),
            ),
            (
                "query_spatial",
                json!({"entity": "Cube", "radius": 10.0, "max_results": 5}),
                post(
                    "/api/v1/spatial/neighborhood",
                    json!({"entity": "Cube", "radius": 10.0, "max_results": 5}),
                ),
            ),
            (
                "check_overlaps",
                json!({"entity": "Cube", "include_siblings": true, "ground_y": 0.0}),
                post(
                    "/api/v1/spatial/overlaps",
                    json!({"entity": "Cube", "include_siblings": true, "ground_y": 0.0}),
                ),
            ),
            (
                "check_overlaps",
                json!({"entity": null, "min_penetration": 0.01, "max_results": 50}),
                post(
                    "/api/v1/spatial/overlaps/all",
                    json!({"min_penetration": 0.01, "max_results": 50}),
                ),
            ),
            (
                "check_all_overlaps",
                json!({"max_float_gap": 0.5, "include_siblings": false}),
                post(
                    "/api/v1/spatial/overlaps/all",
                    json!({"max_float_gap": 0.5, "include_siblings": false}),
                ),
            ),
            (
                "capture_screenshot",
                json!({"gizmos": true, "max_width": 100}),
                post("/api/v1/screenshot/gizmos", json!({"max_width": 100})),
            ),
            (
                "capture_stats",
                json!({"grid": 2}),
                post("/api/v1/screenshot/stats", json!({"grid": 2})),
            ),
            (
                "capture_timeline",
                json!({}),
                post("/api/v1/screenshot/timeline", json!({})),
            ),
            (
                "capture_depth",
                json!({}),
                post("/api/v1/screenshot/depth", json!({})),
            ),
            (
                "compare_frames",
                json!({"a": "f_1", "b": "f_2"}),
                post(
                    "/api/v1/screenshot/compare",
                    json!({"a": "f_1", "b": "f_2"}),
                ),
            ),
            (
                "reload",
                json!({"mode": "partial"}),
                post("/api/v1/reload", json!({"mode": "partial"})),
            ),
            (
                "reload_and_capture",
                json!({}),
                post("/api/v1/reload/capture", json!({})),
            ),
            (
                "query_entities",
                json!({"with": ["Name"]}),
                post("/api/v1/query", json!({"with": ["Name"]})),
            ),
            (
                "spawn_entity",
                json!({"components": {}}),
                post("/api/v1/entities", json!({"components": {}})),
            ),
            (
                "batch",
                json!({"operations": []}),
                post("/api/v1/batch", json!({"operations": []})),
            ),
            (
                "run_code",
                json!({"code": "print(1)"}),
                post("/api/v1/execute", json!({"code": "print(1)"})),
            ),
            (
                "set_time_scale",
                json!({"scale": 0.5}),
                post("/api/v1/time/scale", json!({"scale": 0.5})),
            ),
            (
                "seek_time",
                json!({"seconds": 1.0}),
                post("/api/v1/time/seek", json!({"seconds": 1.0})),
            ),
            (
                "pause_time",
                json!({}),
                post("/api/v1/time/pause", json!({})),
            ),
            (
                "resume_time",
                json!({}),
                post("/api/v1/time/resume", json!({})),
            ),
            (
                "schedule_actions",
                json!({"actions": [{"tool": "pause_time"}], "mode": "async"}),
                post(
                    "/api/v1/schedule",
                    json!({"actions": [{"tool": "pause_time"}], "mode": "async"}),
                ),
            ),
            ("get_reload_status", json!({}), get("/api/v1/reload/status")),
            ("get_last_error", json!({}), get("/api/v1/error")),
            ("get_scene_summary", json!({}), get("/api/v1/scene/summary")),
            ("get_time_status", json!({}), get("/api/v1/time")),
            ("get_performance", json!({}), get("/api/v1/performance")),
            ("get_registry", json!({}), get("/api/v1/debug/registry")),
        ]
    }

    #[tokio::test]
    async fn well_formed_tool_calls_match_their_rest_routes() {
        let (router, log) = engine_router();
        for (tool, arguments, route_request) in route_cases() {
            let label = format!("{tool} {arguments}");
            let via_tool = call(&router, &log, tool, arguments).await;
            let via_route = exchange(&router, &log, vec![route_request]).await;
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
            ("get_schedule_result", json!({}), 422, 0, false),
            (
                "get_component",
                json!({"entity": "Lamp", "component": "Transform"}),
                200,
                1,
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
    async fn malformed_tool_arguments_are_rejected_before_the_engine() {
        let (router, log) = engine_router();
        let entity = "an entity name or packed numeric ID";
        for (tool, arguments, message) in [
            (
                "get_component",
                json!({"entity": true, "component": "Transform"}),
                entity,
            ),
            ("despawn_entity", json!({}), "missing field `entity`"),
            ("despawn_entity", json!({"entity": 1.5}), entity),
            (
                "set_component",
                json!({"entity": 42, "component": "Transform", "extra": 1}),
                "unknown field `extra`, expected one of `entity`, `component`, `fields`",
            ),
            (
                "set_asset",
                json!({"entity": [1], "component": "C", "asset_type": "A"}),
                entity,
            ),
            (
                "query_spatial",
                json!({"entity_a": true, "entity_b": "Sphere"}),
                entity,
            ),
        ] {
            let result = call(&router, &log, tool, arguments).await;
            assert_eq!(result.status, StatusCode::UNPROCESSABLE_ENTITY, "{tool}");
            assert_eq!(
                serde_json::from_str::<Value>(&result.body).unwrap(),
                json!({"error": format!("invalid '{tool}' arguments: {message}")})
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
