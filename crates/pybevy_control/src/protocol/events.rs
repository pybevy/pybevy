use serde::Serialize;

/// SSE event types for real-time streaming to clients
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type")]
pub enum SseEvent {
    #[serde(rename = "log")]
    Log { message: String, level: String },
    #[serde(rename = "error")]
    Error {
        message: String,
        traceback: Option<String>,
    },
    #[serde(rename = "reload_started")]
    ReloadStarted { mode: String, generation: u32 },
    #[serde(rename = "reload_completed")]
    ReloadCompleted { mode: String, generation: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_event_log_serialization() {
        let event = SseEvent::Log {
            message: "hello".into(),
            level: "info".into(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "log");
        assert_eq!(json["message"], "hello");
        assert_eq!(json["level"], "info");
    }

    #[test]
    fn sse_event_error_serialization() {
        let event = SseEvent::Error {
            message: "oops".into(),
            traceback: Some("trace".into()),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "error");
        assert_eq!(json["message"], "oops");
        assert_eq!(json["traceback"], "trace");
    }

    #[test]
    fn sse_event_reload_started_serialization() {
        let event = SseEvent::ReloadStarted {
            mode: "full".into(),
            generation: 3,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "reload_started");
        assert_eq!(json["mode"], "full");
        assert_eq!(json["generation"], 3);
    }

    #[test]
    fn sse_event_reload_completed_serialization() {
        let event = SseEvent::ReloadCompleted {
            mode: "partial".into(),
            generation: 5,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "reload_completed");
        assert_eq!(json["mode"], "partial");
        assert_eq!(json["generation"], 5);
    }
}
