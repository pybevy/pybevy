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
    #[serde(rename = "tool_registered")]
    ToolRegistered { name: String },
}
