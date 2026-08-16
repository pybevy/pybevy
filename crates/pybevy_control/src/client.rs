use std::{error::Error, fmt, time::Duration};

use serde_json::{Map, Value, json};

const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 64 * 1024;
const IMAGE_DELIVERY_HEADER: (&str, &str) = ("x-pybevy-image-delivery", "mcp");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Method {
    fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RequestSpec {
    method: Method,
    path: String,
    body: Option<Value>,
    headers: Vec<(&'static str, &'static str)>,
}

impl RequestSpec {
    fn get(path: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            path: path.into(),
            body: None,
            headers: Vec::new(),
        }
    }

    fn delete(path: impl Into<String>) -> Self {
        Self {
            method: Method::Delete,
            path: path.into(),
            body: None,
            headers: Vec::new(),
        }
    }

    fn json(method: Method, path: impl Into<String>, body: Value) -> Self {
        Self {
            method,
            path: path.into(),
            body: Some(body),
            headers: Vec::new(),
        }
    }

    fn with_image_delivery(mut self) -> Self {
        self.headers.push(IMAGE_DELIVERY_HEADER);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientErrorKind {
    Timeout,
    Connect,
    HttpStatus,
    Protocol,
    Validation,
    Transport,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientError {
    kind: ClientErrorKind,
    message: String,
    status: Option<u16>,
    response_body: Option<String>,
}

impl ClientError {
    fn new(kind: ClientErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            status: None,
            response_body: None,
        }
    }

    fn status(status: u16, response_body: String) -> Self {
        let message = status_message(status, &response_body);
        Self {
            kind: ClientErrorKind::HttpStatus,
            message,
            status: Some(status),
            response_body: Some(response_body),
        }
    }

    pub fn kind(&self) -> ClientErrorKind {
        self.kind
    }

    pub fn status_code(&self) -> Option<u16> {
        self.status
    }

    pub fn response_body(&self) -> Option<&str> {
        self.response_body.as_deref()
    }
}

impl fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ClientError {}

#[derive(Debug)]
struct RawResponse {
    status: u16,
    body: Vec<u8>,
}

pub fn request_tool(
    port: u16,
    tool_name: &str,
    arguments: Value,
    timeout: Duration,
) -> Result<Value, ClientError> {
    let arguments = arguments.as_object().ok_or_else(|| {
        ClientError::new(
            ClientErrorKind::Validation,
            "tool arguments must be a JSON object",
        )
    })?;

    if tool_name == "get_component"
        && let Some(Value::String(entity_name)) = arguments.get("entity")
    {
        if entity_name.is_empty() {
            return Err(ClientError::new(
                ClientErrorKind::Validation,
                "get_component requires a non-empty entity name or numeric ID",
            ));
        }
        let component =
            non_empty_string(arguments, "component", "get_component", "component name")?;
        let lookup = request_json(
            port,
            &RequestSpec::get(format!(
                "/api/v1/entities/{}",
                encode_path_segment(entity_name)
            )),
            timeout,
        )?;
        let entity_id = lookup.get("id").and_then(Value::as_u64).ok_or_else(|| {
            ClientError::new(
                ClientErrorKind::Validation,
                "Entity lookup response did not contain a valid numeric id",
            )
        })?;
        return request_json(
            port,
            &RequestSpec::get(format!(
                "/api/v1/entities/{entity_id}/components/{}",
                encode_path_segment(component)
            )),
            timeout,
        );
    }

    let request = build_tool_request(tool_name, arguments)?;
    request_json(port, &request, timeout)
}

pub fn request_scene_resource(
    port: u16,
    uri: &str,
    timeout: Duration,
) -> Result<Value, ClientError> {
    let request = build_scene_resource_request(uri)?;
    request_json(port, &request, timeout)
}

fn build_scene_resource_request(uri: &str) -> Result<RequestSpec, ClientError> {
    let path = match uri {
        "scene://entities" => "/api/v1/entities".to_string(),
        "scene://resources" => "/api/v1/resources".to_string(),
        "scene://systems" => "/api/v1/systems".to_string(),
        "scene://systems/all" => "/api/v1/systems?include_internal=true".to_string(),
        "scene://debug" => "/api/v1/performance".to_string(),
        "scene://components" => "/api/v1/debug/registry".to_string(),
        _ => {
            let entity_prefix = "scene://entity/";
            let Some(entity) = uri.strip_prefix(entity_prefix) else {
                return Err(ClientError::new(
                    ClientErrorKind::Validation,
                    format!("Unknown scene resource: {uri}"),
                ));
            };
            if entity == "{name_or_id}" {
                return Err(ClientError::new(
                    ClientErrorKind::Validation,
                    "Resource template must be expanded with an entity name or numeric ID",
                ));
            }
            if entity.is_empty() {
                return Err(ClientError::new(
                    ClientErrorKind::Validation,
                    "scene://entity/ requires an entity name or numeric ID",
                ));
            }
            format!("/api/v1/entities/{}", encode_path_segment(entity))
        }
    };
    Ok(RequestSpec::get(path))
}

pub fn control_health(port: u16, timeout: Duration) -> Result<bool, ClientError> {
    let response = execute(port, &RequestSpec::get("/health"), timeout)?;
    Ok(response.status == 200)
}

pub fn control_last_error(port: u16, timeout: Duration) -> Result<Value, ClientError> {
    request_json(port, &RequestSpec::get("/api/v1/error"), timeout)
}

fn build_tool_request(
    tool_name: &str,
    arguments: &Map<String, Value>,
) -> Result<RequestSpec, ClientError> {
    let arguments_value = Value::Object(arguments.clone());
    let image_delivery = matches!(
        tool_name,
        "capture_screenshot"
            | "capture_timeline"
            | "capture_turnaround"
            | "capture_depth"
            | "reload_and_capture"
            | "schedule_actions"
            | "get_schedule_result"
    );

    let mut request = match tool_name {
        "get_component" => {
            let component =
                non_empty_string(arguments, "component", "get_component", "component name")?;
            let entity = arguments
                .get("entity")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    ClientError::new(
                        ClientErrorKind::Validation,
                        "get_component requires an entity name or numeric ID",
                    )
                })?;
            RequestSpec::get(format!(
                "/api/v1/entities/{entity}/components/{}",
                encode_path_segment(component)
            ))
        }
        "get_component_schema" => RequestSpec::get(format!(
            "/api/v1/components/{}/schema",
            encode_path_segment(&argument_string(arguments, "name"))
        )),
        "get_resource" => {
            let resource =
                non_empty_string(arguments, "resource_type", "get_resource", "resource_type")?;
            RequestSpec::get(format!(
                "/api/v1/resources/{}",
                encode_path_segment(resource)
            ))
        }
        "get_system_list" => RequestSpec::get(format!(
            "/api/v1/systems?include_internal={}",
            arguments
                .get("include_internal")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        )),
        "despawn_entity" => RequestSpec::delete(format!(
            "/api/v1/entities/{}",
            encode_path_segment(&argument_string(arguments, "entity"))
        )),
        "set_component" => RequestSpec::json(
            Method::Put,
            format!(
                "/api/v1/entities/{}/components/{}",
                encode_path_segment(&argument_string(arguments, "entity")),
                encode_path_segment(&argument_string(arguments, "component"))
            ),
            json!({"fields": arguments.get("fields").cloned().unwrap_or_else(|| json!({}))}),
        ),
        "remove_component" => RequestSpec::delete(format!(
            "/api/v1/entities/{}/components/{}",
            encode_path_segment(&argument_string(arguments, "entity")),
            encode_path_segment(&argument_string(arguments, "component"))
        )),
        "get_bounding_box" => RequestSpec::get(format!(
            "/api/v1/entities/{}/bounding_box",
            encode_path_segment(&argument_string(arguments, "entity"))
        )),
        "set_resource" => RequestSpec::json(
            Method::Put,
            format!(
                "/api/v1/resources/{}",
                encode_path_segment(&argument_string(arguments, "resource_type"))
            ),
            json!({"value": arguments.get("value").cloned().unwrap_or_else(|| json!({}))}),
        ),
        "remove_resource" => RequestSpec::delete(format!(
            "/api/v1/resources/{}",
            encode_path_segment(&argument_string(arguments, "resource_type"))
        )),
        "set_asset" => RequestSpec::json(
            Method::Post,
            "/api/v1/assets/mutate",
            json!({
                "entity": arguments.get("entity").cloned().unwrap_or(Value::Null),
                "component": argument_string(arguments, "component"),
                "asset_type": argument_string(arguments, "asset_type"),
                "fields": arguments.get("fields").cloned().unwrap_or_else(|| json!({})),
            }),
        ),
        "get_schedule_result" => RequestSpec::get(format!(
            "/api/v1/schedule/{}",
            encode_path_segment(&argument_string(arguments, "schedule_id"))
        )),
        "query_spatial" => {
            if let Some(radius) = arguments.get("radius") {
                let mut body = Map::from_iter([
                    (
                        "entity".to_string(),
                        arguments.get("entity").cloned().unwrap_or(Value::Null),
                    ),
                    ("radius".to_string(), radius.clone()),
                ]);
                copy_if_present(arguments, &mut body, &["max_results"]);
                RequestSpec::json(
                    Method::Post,
                    "/api/v1/spatial/neighborhood",
                    Value::Object(body),
                )
            } else {
                RequestSpec::json(
                    Method::Post,
                    "/api/v1/spatial/query",
                    json!({
                        "entity_a": arguments.get("entity_a").cloned().unwrap_or(Value::Null),
                        "entity_b": arguments.get("entity_b").cloned().unwrap_or(Value::Null),
                    }),
                )
            }
        }
        "check_overlaps" => {
            if arguments
                .get("entity")
                .is_some_and(|value| !value.is_null())
            {
                let mut body = Map::from_iter([(
                    "entity".to_string(),
                    arguments.get("entity").cloned().unwrap_or(Value::Null),
                )]);
                copy_if_present(
                    arguments,
                    &mut body,
                    &["include_siblings", "max_float_gap", "ground_y"],
                );
                RequestSpec::json(
                    Method::Post,
                    "/api/v1/spatial/overlaps",
                    Value::Object(body),
                )
            } else {
                RequestSpec::json(
                    Method::Post,
                    "/api/v1/spatial/overlaps/all",
                    Value::Object(overlap_all_body(arguments)),
                )
            }
        }
        "query_spatial_neighborhood" => {
            let mut body = Map::from_iter([
                (
                    "entity".to_string(),
                    arguments.get("entity").cloned().unwrap_or(Value::Null),
                ),
                (
                    "radius".to_string(),
                    arguments.get("radius").cloned().unwrap_or(Value::Null),
                ),
            ]);
            copy_if_present(arguments, &mut body, &["max_results"]);
            RequestSpec::json(
                Method::Post,
                "/api/v1/spatial/neighborhood",
                Value::Object(body),
            )
        }
        "check_all_overlaps" => RequestSpec::json(
            Method::Post,
            "/api/v1/spatial/overlaps/all",
            Value::Object(overlap_all_body(arguments)),
        ),
        "capture_screenshot" => {
            let mut body = arguments.clone();
            let gizmos = body
                .remove("gizmos")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            RequestSpec::json(
                Method::Post,
                if gizmos {
                    "/api/v1/screenshot/gizmos"
                } else {
                    "/api/v1/screenshot"
                },
                Value::Object(body),
            )
        }
        _ => {
            let Some((method, path)) = simple_tool_route(tool_name) else {
                return Err(ClientError::new(
                    ClientErrorKind::Validation,
                    format!("Unknown tool: {tool_name}"),
                ));
            };
            match method {
                Method::Get => RequestSpec::get(path),
                Method::Post | Method::Put => RequestSpec::json(method, path, arguments_value),
                Method::Delete => RequestSpec::delete(path),
            }
        }
    };

    if image_delivery {
        request = request.with_image_delivery();
    }
    Ok(request)
}

fn simple_tool_route(tool_name: &str) -> Option<(Method, &'static str)> {
    Some(match tool_name {
        "query_entities" => (Method::Post, "/api/v1/query"),
        "capture_timeline" => (Method::Post, "/api/v1/screenshot/timeline"),
        "capture_turnaround" => (Method::Post, "/api/v1/screenshot/turnaround"),
        "capture_depth" => (Method::Post, "/api/v1/screenshot/depth"),
        "capture_stats" => (Method::Post, "/api/v1/screenshot/stats"),
        "compare_frames" => (Method::Post, "/api/v1/screenshot/compare"),
        "reload" => (Method::Post, "/api/v1/reload"),
        "get_reload_status" => (Method::Get, "/api/v1/reload/status"),
        "get_last_error" => (Method::Get, "/api/v1/error"),
        "spawn_entity" => (Method::Post, "/api/v1/entities"),
        "batch" => (Method::Post, "/api/v1/batch"),
        "get_scene_summary" => (Method::Get, "/api/v1/scene/summary"),
        "reload_and_capture" => (Method::Post, "/api/v1/reload/capture"),
        "pause_time" => (Method::Post, "/api/v1/time/pause"),
        "resume_time" => (Method::Post, "/api/v1/time/resume"),
        "set_time_scale" => (Method::Post, "/api/v1/time/scale"),
        "get_time_status" => (Method::Get, "/api/v1/time"),
        "seek_time" => (Method::Post, "/api/v1/time/seek"),
        "run_code" => (Method::Post, "/api/v1/execute"),
        "get_performance" => (Method::Get, "/api/v1/performance"),
        "get_registry" => (Method::Get, "/api/v1/debug/registry"),
        "schedule_actions" => (Method::Post, "/api/v1/schedule"),
        _ => return None,
    })
}

fn request_json(port: u16, request: &RequestSpec, timeout: Duration) -> Result<Value, ClientError> {
    let response = execute(port, request, timeout)?;
    if !(200..300).contains(&response.status) {
        return Err(ClientError::status(
            response.status,
            String::from_utf8_lossy(&response.body).into_owned(),
        ));
    }
    parse_json_body(response.body)
}

fn execute(
    port: u16,
    request: &RequestSpec,
    timeout: Duration,
) -> Result<RawResponse, ClientError> {
    validate_path(&request.path)?;
    let body = request
        .body
        .as_ref()
        .map(serde_json::to_vec)
        .transpose()
        .map_err(|error| {
            ClientError::new(
                ClientErrorKind::Protocol,
                format!("failed to serialize request JSON: {error}"),
            )
        })?
        .unwrap_or_default();

    let uri = format!("http://127.0.0.1:{port}{}", request.path);
    let mut builder = ureq::http::Request::builder()
        .method(request.method.as_str())
        .uri(uri)
        .header("Accept", "application/json");
    if request.body.is_some() {
        builder = builder.header("Content-Type", "application/json");
    }
    for (name, value) in &request.headers {
        if name.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
            return Err(ClientError::new(
                ClientErrorKind::Validation,
                "request headers must not contain newlines",
            ));
        }
        builder = builder.header(*name, *value);
    }

    let request = builder.body(body).map_err(|error| {
        ClientError::new(
            ClientErrorKind::Validation,
            format!("failed to construct loopback HTTP request: {error}"),
        )
    })?;
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .timeout_connect(Some(timeout))
        .timeout_send_request(Some(timeout))
        .timeout_send_body(Some(timeout))
        .timeout_recv_response(Some(timeout))
        .timeout_recv_body(Some(timeout))
        .http_status_as_error(false)
        .max_redirects(0)
        .max_redirects_will_error(false)
        .max_response_header_size(MAX_HEADER_BYTES)
        .no_delay(true)
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let mut response = agent.run(request).map_err(map_ureq_error)?;
    let status = response.status().as_u16();
    if response
        .body()
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(response_limit_error());
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE_BYTES as u64)
        .read_to_vec()
        .map_err(map_ureq_error)?;
    Ok(RawResponse { status, body })
}

fn parse_json_body(body: Vec<u8>) -> Result<Value, ClientError> {
    serde_json::from_slice(&body).map_err(|error| {
        ClientError::new(
            ClientErrorKind::Protocol,
            format!("invalid JSON response: {error}"),
        )
    })
}

fn status_message(status: u16, response_body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(response_body)
        && let Some(error) = value.get("error")
    {
        return error
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| error.to_string());
    }
    let trimmed = response_body.trim();
    if trimmed.is_empty() {
        format!("HTTP status {status}")
    } else {
        trimmed.to_string()
    }
}

fn non_empty_string<'a>(
    arguments: &'a Map<String, Value>,
    key: &str,
    operation: &str,
    description: &str,
) -> Result<&'a str, ClientError> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ClientError::new(
                ClientErrorKind::Validation,
                format!("{operation} requires a non-empty {description}"),
            )
        })
}

fn argument_string(arguments: &Map<String, Value>, key: &str) -> String {
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

fn overlap_all_body(arguments: &Map<String, Value>) -> Map<String, Value> {
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

fn copy_if_present(
    source: &Map<String, Value>,
    destination: &mut Map<String, Value>,
    keys: &[&str],
) {
    for key in keys {
        if let Some(value) = source.get(*key) {
            destination.insert((*key).to_string(), value.clone());
        }
    }
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}

fn validate_path(path: &str) -> Result<(), ClientError> {
    if !path.starts_with('/') || path.contains(['\r', '\n', ' ']) {
        return Err(ClientError::new(
            ClientErrorKind::Validation,
            "HTTP request path is invalid",
        ));
    }
    Ok(())
}

fn map_ureq_error(error: ureq::Error) -> ClientError {
    match error {
        ureq::Error::Timeout(_) => {
            ClientError::new(ClientErrorKind::Timeout, "loopback HTTP request timed out")
        }
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => ClientError::new(
            ClientErrorKind::Connect,
            format!("failed to connect to loopback HTTP server: {error}"),
        ),
        ureq::Error::Io(error) => map_ureq_io_error(error),
        ureq::Error::BodyExceedsLimit(_) => response_limit_error(),
        ureq::Error::LargeResponseHeader(_, _) => ClientError::new(
            ClientErrorKind::Protocol,
            "HTTP response headers exceed the configured limit",
        ),
        ureq::Error::Protocol(_) => ClientError::new(
            ClientErrorKind::Protocol,
            format!("invalid loopback HTTP response: {error}"),
        ),
        ureq::Error::BadUri(_) | ureq::Error::Http(_) => ClientError::new(
            ClientErrorKind::Validation,
            format!("failed to construct loopback HTTP request: {error}"),
        ),
        _ => ClientError::new(
            ClientErrorKind::Transport,
            format!("loopback HTTP transport error: {error}"),
        ),
    }
}

fn map_ureq_io_error(error: std::io::Error) -> ClientError {
    match error.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
            ClientError::new(ClientErrorKind::Timeout, "loopback HTTP request timed out")
        }
        std::io::ErrorKind::ConnectionRefused
        | std::io::ErrorKind::ConnectionAborted
        | std::io::ErrorKind::NotConnected
        | std::io::ErrorKind::AddrNotAvailable => ClientError::new(
            ClientErrorKind::Connect,
            format!("failed to connect to loopback HTTP server: {error}"),
        ),
        _ => ClientError::new(
            ClientErrorKind::Transport,
            format!("loopback HTTP transport error: {error}"),
        ),
    }
}

fn response_limit_error() -> ClientError {
    ClientError::new(
        ClientErrorKind::Protocol,
        "HTTP response body exceeds the configured limit",
    )
}

#[cfg(test)]
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::{Ipv4Addr, TcpListener},
        sync::mpsc,
        thread,
    };

    use super::*;

    fn serve_once(response: &'static [u8]) -> (u16, mpsc::Receiver<String>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (request_sender, request_receiver) = mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..read]);
                if read == 0 || find_bytes(&request, b"\r\n\r\n").is_some() {
                    break;
                }
            }
            let _ = request_sender.send(String::from_utf8_lossy(&request).into_owned());
            stream.write_all(response).unwrap();
        });
        (port, request_receiver)
    }

    fn serve_responses(responses: Vec<&'static [u8]>) -> (u16, mpsc::Receiver<String>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (request_sender, request_receiver) = mpsc::channel();
        thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buffer = [0_u8; 4096];
                loop {
                    let read = stream.read(&mut buffer).unwrap();
                    request.extend_from_slice(&buffer[..read]);
                    if read == 0 || find_bytes(&request, b"\r\n\r\n").is_some() {
                        break;
                    }
                }
                let _ = request_sender.send(String::from_utf8_lossy(&request).into_owned());
                stream.write_all(response).unwrap();
            }
        });
        (port, request_receiver)
    }

    #[test]
    fn percent_encodes_path_segments() {
        assert_eq!(
            encode_path_segment("../named entity?/雪"),
            "..%2Fnamed%20entity%3F%2F%E9%9B%AA"
        );
    }

    #[test]
    fn routes_tool_requests_and_image_header() {
        let request = build_tool_request(
            "capture_screenshot",
            json!({"gizmos": true, "width": 100}).as_object().unwrap(),
        )
        .unwrap();
        assert_eq!(request.method, Method::Post);
        assert_eq!(request.path, "/api/v1/screenshot/gizmos");
        assert_eq!(request.body, Some(json!({"width": 100})));
        assert_eq!(request.headers, vec![IMAGE_DELIVERY_HEADER]);
    }

    #[test]
    fn routes_every_simple_tool() {
        let routes = [
            ("query_entities", Method::Post, "/api/v1/query", false),
            (
                "capture_timeline",
                Method::Post,
                "/api/v1/screenshot/timeline",
                true,
            ),
            (
                "capture_turnaround",
                Method::Post,
                "/api/v1/screenshot/turnaround",
                true,
            ),
            (
                "capture_depth",
                Method::Post,
                "/api/v1/screenshot/depth",
                true,
            ),
            (
                "capture_stats",
                Method::Post,
                "/api/v1/screenshot/stats",
                false,
            ),
            (
                "compare_frames",
                Method::Post,
                "/api/v1/screenshot/compare",
                false,
            ),
            ("reload", Method::Post, "/api/v1/reload", false),
            (
                "get_reload_status",
                Method::Get,
                "/api/v1/reload/status",
                false,
            ),
            ("get_last_error", Method::Get, "/api/v1/error", false),
            ("spawn_entity", Method::Post, "/api/v1/entities", false),
            ("batch", Method::Post, "/api/v1/batch", false),
            (
                "get_scene_summary",
                Method::Get,
                "/api/v1/scene/summary",
                false,
            ),
            (
                "reload_and_capture",
                Method::Post,
                "/api/v1/reload/capture",
                true,
            ),
            ("pause_time", Method::Post, "/api/v1/time/pause", false),
            ("resume_time", Method::Post, "/api/v1/time/resume", false),
            ("set_time_scale", Method::Post, "/api/v1/time/scale", false),
            ("get_time_status", Method::Get, "/api/v1/time", false),
            ("seek_time", Method::Post, "/api/v1/time/seek", false),
            ("run_code", Method::Post, "/api/v1/execute", false),
            ("get_performance", Method::Get, "/api/v1/performance", false),
            ("get_registry", Method::Get, "/api/v1/debug/registry", false),
            ("schedule_actions", Method::Post, "/api/v1/schedule", true),
        ];
        let arguments = json!({"probe": true});

        for (tool, method, path, image_delivery) in routes {
            let request = build_tool_request(tool, arguments.as_object().unwrap()).unwrap();
            assert_eq!(request.method, method, "wrong method for {tool}");
            assert_eq!(request.path, path, "wrong path for {tool}");
            assert_eq!(
                request.body,
                (method == Method::Post).then(|| arguments.clone()),
                "wrong body for {tool}"
            );
            assert_eq!(
                request.headers,
                if image_delivery {
                    vec![IMAGE_DELIVERY_HEADER]
                } else {
                    Vec::new()
                },
                "wrong headers for {tool}"
            );
        }
    }

    #[test]
    fn routes_argument_dependent_tools() {
        let cases = vec![
            (
                "get_component",
                json!({"entity": 42, "component": "Spot Light"}),
                RequestSpec::get("/api/v1/entities/42/components/Spot%20Light"),
            ),
            (
                "get_component_schema",
                json!({"name": "State[game.Phase]"}),
                RequestSpec::get("/api/v1/components/State%5Bgame.Phase%5D/schema"),
            ),
            (
                "get_resource",
                json!({"resource_type": "State[game.Phase]"}),
                RequestSpec::get("/api/v1/resources/State%5Bgame.Phase%5D"),
            ),
            (
                "get_system_list",
                json!({"include_internal": true}),
                RequestSpec::get("/api/v1/systems?include_internal=true"),
            ),
            (
                "despawn_entity",
                json!({"entity": "named/entity"}),
                RequestSpec::delete("/api/v1/entities/named%2Fentity"),
            ),
            (
                "set_component",
                json!({"entity": 42, "component": "Transform", "fields": {"x": 1}}),
                RequestSpec::json(
                    Method::Put,
                    "/api/v1/entities/42/components/Transform",
                    json!({"fields": {"x": 1}}),
                ),
            ),
            (
                "remove_component",
                json!({"entity": 42, "component": "Transform"}),
                RequestSpec::delete("/api/v1/entities/42/components/Transform"),
            ),
            (
                "get_bounding_box",
                json!({"entity": "Main Camera"}),
                RequestSpec::get("/api/v1/entities/Main%20Camera/bounding_box"),
            ),
            (
                "set_resource",
                json!({"resource_type": "AmbientLight", "value": {"brightness": 1}}),
                RequestSpec::json(
                    Method::Put,
                    "/api/v1/resources/AmbientLight",
                    json!({"value": {"brightness": 1}}),
                ),
            ),
            (
                "remove_resource",
                json!({"resource_type": "AmbientLight"}),
                RequestSpec::delete("/api/v1/resources/AmbientLight"),
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
                RequestSpec::json(
                    Method::Post,
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
                RequestSpec::get("/api/v1/schedule/capture%2F1").with_image_delivery(),
            ),
            (
                "query_spatial",
                json!({"entity_a": "Cube", "entity_b": "Sphere", "ignored": 1}),
                RequestSpec::json(
                    Method::Post,
                    "/api/v1/spatial/query",
                    json!({"entity_a": "Cube", "entity_b": "Sphere"}),
                ),
            ),
            (
                "query_spatial",
                json!({"entity": "Cube", "radius": 10.0, "max_results": 5}),
                RequestSpec::json(
                    Method::Post,
                    "/api/v1/spatial/neighborhood",
                    json!({"entity": "Cube", "radius": 10.0, "max_results": 5}),
                ),
            ),
            (
                "query_spatial_neighborhood",
                json!({"entity": "Cube", "radius": 10.0, "max_results": 5}),
                RequestSpec::json(
                    Method::Post,
                    "/api/v1/spatial/neighborhood",
                    json!({"entity": "Cube", "radius": 10.0, "max_results": 5}),
                ),
            ),
            (
                "check_overlaps",
                json!({"entity": "Cube", "include_siblings": true, "ground_y": 0.0}),
                RequestSpec::json(
                    Method::Post,
                    "/api/v1/spatial/overlaps",
                    json!({"entity": "Cube", "include_siblings": true, "ground_y": 0.0}),
                ),
            ),
            (
                "check_overlaps",
                json!({"min_penetration": 0.01, "max_results": 50}),
                RequestSpec::json(
                    Method::Post,
                    "/api/v1/spatial/overlaps/all",
                    json!({"min_penetration": 0.01, "max_results": 50}),
                ),
            ),
            (
                "check_all_overlaps",
                json!({"max_float_gap": 0.5, "include_siblings": false}),
                RequestSpec::json(
                    Method::Post,
                    "/api/v1/spatial/overlaps/all",
                    json!({"max_float_gap": 0.5, "include_siblings": false}),
                ),
            ),
            (
                "capture_screenshot",
                json!({"gizmos": true, "width": 100}),
                RequestSpec::json(
                    Method::Post,
                    "/api/v1/screenshot/gizmos",
                    json!({"width": 100}),
                )
                .with_image_delivery(),
            ),
        ];

        for (tool, arguments, expected) in cases {
            assert_eq!(
                build_tool_request(tool, arguments.as_object().unwrap()).unwrap(),
                expected,
                "wrong request for {tool} with {arguments}"
            );
        }
    }

    #[test]
    fn routes_and_validates_scene_resources() {
        let cases = [
            ("scene://entities", "/api/v1/entities"),
            ("scene://resources", "/api/v1/resources"),
            ("scene://systems", "/api/v1/systems"),
            (
                "scene://systems/all",
                "/api/v1/systems?include_internal=true",
            ),
            ("scene://debug", "/api/v1/performance"),
            ("scene://components", "/api/v1/debug/registry"),
            (
                "scene://entity/Main Camera/灯",
                "/api/v1/entities/Main%20Camera%2F%E7%81%AF",
            ),
        ];
        for (uri, path) in cases {
            assert_eq!(
                build_scene_resource_request(uri).unwrap(),
                RequestSpec::get(path),
                "wrong request for {uri}"
            );
        }

        for (uri, message) in [
            ("scene://unknown", "Unknown scene resource"),
            ("scene://entity/", "requires an entity name"),
            ("scene://entity/{name_or_id}", "must be expanded"),
        ] {
            assert!(
                build_scene_resource_request(uri)
                    .unwrap_err()
                    .to_string()
                    .contains(message)
            );
        }
    }

    #[test]
    fn validates_required_tool_arguments_before_connecting() {
        let cases = [
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
            ("unknown", json!({}), "Unknown tool: unknown"),
        ];
        for (tool, arguments, message) in cases {
            let error = build_tool_request(tool, arguments.as_object().unwrap()).unwrap_err();
            assert_eq!(error.kind(), ClientErrorKind::Validation);
            assert_eq!(error.to_string(), message);
        }
    }

    #[test]
    fn resolves_named_entity_before_getting_component() {
        let (port, requests) = serve_responses(vec![
            b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{\"id\":42}",
            b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{\"component\":\"Spot Light\"}",
        ]);

        let result = request_tool(
            port,
            "get_component",
            json!({"entity": "Lighthouse / 💡", "component": "Spot Light"}),
            Duration::from_secs(2),
        )
        .unwrap();

        assert_eq!(result["component"], "Spot Light");
        assert!(
            requests
                .recv()
                .unwrap()
                .starts_with("GET /api/v1/entities/Lighthouse%20%2F%20%F0%9F%92%A1 HTTP/1.1")
        );
        assert!(
            requests
                .recv()
                .unwrap()
                .starts_with("GET /api/v1/entities/42/components/Spot%20Light HTTP/1.1")
        );
    }

    #[test]
    fn performs_bounded_json_request() {
        let body = br#"{"ok":true}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            String::from_utf8_lossy(body)
        );
        let response: &'static [u8] = Box::leak(response.into_bytes().into_boxed_slice());
        let (port, captured) = serve_once(response);
        let value = request_json(
            port,
            &RequestSpec::get("/api/v1/test"),
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(value, json!({"ok": true}));
        assert!(
            captured
                .recv()
                .unwrap()
                .starts_with("GET /api/v1/test HTTP/1.1")
        );
    }

    #[test]
    fn preserves_structured_http_error_message() {
        let body = br#"{"error":"invalid component"}"#;
        let response = format!(
            "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            String::from_utf8_lossy(body)
        );
        let response: &'static [u8] = Box::leak(response.into_bytes().into_boxed_slice());
        let (port, _) = serve_once(response);
        let error = request_json(
            port,
            &RequestSpec::get("/api/v1/test"),
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ClientErrorKind::HttpStatus);
        assert_eq!(error.to_string(), "invalid component");
    }

    #[test]
    fn preserves_non_json_http_error_body() {
        let response = b"HTTP/1.1 422 Unprocessable Entity\r\nConnection: close\r\n\r\ndelay_frames: expected u32";
        let (port, _) = serve_once(response);
        let error = request_json(
            port,
            &RequestSpec::get("/api/v1/test"),
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ClientErrorKind::HttpStatus);
        assert_eq!(error.status_code(), Some(422));
        assert_eq!(error.to_string(), "delay_frames: expected u32");
    }

    #[test]
    fn does_not_follow_redirects() {
        let response = b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/external\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let (port, _) = serve_once(response);
        let error = request_json(
            port,
            &RequestSpec::get("/api/v1/test"),
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ClientErrorKind::HttpStatus);
        assert_eq!(error.status_code(), Some(302));
    }

    #[test]
    fn rejects_malformed_json_response() {
        let response = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nnot-json";
        let (port, _) = serve_once(response);
        let error = request_json(
            port,
            &RequestSpec::get("/api/v1/test"),
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ClientErrorKind::Protocol);
        assert!(error.to_string().contains("invalid JSON response"));
    }

    #[test]
    fn rejects_response_larger_than_bound() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 33554433\r\nConnection: close\r\n\r\n";
        let (port, _) = serve_once(response);
        let error = request_json(
            port,
            &RequestSpec::get("/api/v1/test"),
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ClientErrorKind::Protocol);
        assert!(error.to_string().contains("configured limit"));
    }

    #[test]
    fn classifies_response_timeout() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (release_sender, release_receiver) = mpsc::channel();
        thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            let _ = release_receiver.recv();
        });

        let error = request_json(
            port,
            &RequestSpec::get("/api/v1/test"),
            Duration::from_millis(20),
        )
        .unwrap_err();
        let _ = release_sender.send(());
        assert_eq!(error.kind(), ClientErrorKind::Timeout);
    }

    #[test]
    fn classifies_connect_timeout_error_kind() {
        let error = map_ureq_io_error(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "connect deadline",
        ));
        assert_eq!(error.kind(), ClientErrorKind::Timeout);
    }

    #[test]
    fn classifies_connection_refusal() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let error = control_health(port, Duration::from_millis(100)).unwrap_err();
        assert_eq!(error.kind(), ClientErrorKind::Connect);
    }

    #[test]
    fn decodes_chunked_response() {
        let response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n7\r\n{\"ok\": \r\n5\r\ntrue}\r\n0\r\n\r\n";
        let (port, _) = serve_once(response);
        let value =
            request_json(port, &RequestSpec::get("/chunked"), Duration::from_secs(2)).unwrap();
        assert_eq!(value, json!({"ok": true}));
    }
}
