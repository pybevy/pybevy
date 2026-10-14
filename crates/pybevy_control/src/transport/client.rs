use std::{error::Error, fmt, time::Duration};

use serde_json::Value;

use crate::{
    catalog::tools::is_engine_tool_name,
    transport::image_delivery::{MCP_IMAGE_DELIVERY_HEADER, MCP_IMAGE_DELIVERY_VALUE},
};

const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    Get,
    Post,
}

impl Method {
    fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
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

    fn post(path: impl Into<String>, body: Value) -> Self {
        Self {
            method: Method::Post,
            path: path.into(),
            body: Some(body),
            headers: Vec::new(),
        }
    }

    fn with_image_delivery(mut self) -> Self {
        self.headers
            .push((MCP_IMAGE_DELIVERY_HEADER, MCP_IMAGE_DELIVERY_VALUE));
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
    let request = build_tool_request(tool_name, arguments)?;
    request_json(port, &request, timeout)
}

fn build_tool_request(tool_name: &str, arguments: Value) -> Result<RequestSpec, ClientError> {
    if !arguments.is_object() {
        return Err(ClientError::new(
            ClientErrorKind::Validation,
            "tool arguments must be a JSON object",
        ));
    }
    if !is_engine_tool_name(tool_name) {
        return Err(ClientError::new(
            ClientErrorKind::Validation,
            format!("Unknown tool: {tool_name}"),
        ));
    }
    Ok(RequestSpec::post(
        format!("/api/v1/tools/{}", encode_path_segment(tool_name)),
        arguments,
    )
    .with_image_delivery())
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

    use serde_json::json;

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

    #[test]
    fn percent_encodes_path_segments() {
        assert_eq!(
            encode_path_segment("../named entity?/雪"),
            "..%2Fnamed%20entity%3F%2F%E9%9B%AA"
        );
    }

    #[test]
    fn posts_tool_calls_to_the_tools_endpoint_with_image_delivery() {
        let request =
            build_tool_request("get_schedule_result", json!({"schedule_id": "s/1"})).unwrap();
        assert_eq!(
            request,
            RequestSpec::post(
                "/api/v1/tools/get_schedule_result",
                json!({"schedule_id": "s/1"})
            )
            .with_image_delivery()
        );
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
    fn validates_tool_calls_before_connecting() {
        for (tool, arguments, message) in [
            ("unknown", json!({}), "Unknown tool: unknown"),
            ("get_entity", json!({}), "Unknown tool: get_entity"),
            (
                "pause_time",
                json!([]),
                "tool arguments must be a JSON object",
            ),
        ] {
            let error = request_tool(1, tool, arguments, Duration::from_millis(1)).unwrap_err();
            assert_eq!(error.kind(), ClientErrorKind::Validation);
            assert_eq!(error.to_string(), message);
        }
    }

    #[test]
    fn sends_arguments_to_the_tools_endpoint() {
        let (port, requests) = serve_once(
            b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{\"component\":\"Spot Light\"}",
        );

        let result = request_tool(
            port,
            "get_component",
            json!({"entity": "Lighthouse", "component": "Spot Light"}),
            Duration::from_secs(2),
        )
        .unwrap();

        assert_eq!(result["component"], "Spot Light");
        let request = requests.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("post /api/v1/tools/get_component http/1.1"));
        assert!(request.contains("x-pybevy-image-delivery: mcp"));
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
