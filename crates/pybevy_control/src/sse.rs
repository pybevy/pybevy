use crate::{bridge::SseEventBroadcaster, protocol::SseEvent};

impl SseEventBroadcaster {
    pub fn log(&self, message: &str, level: &str) {
        self.send(&SseEvent::Log {
            message: message.to_string(),
            level: level.to_string(),
        });
    }

    pub fn error(&self, message: &str, traceback: Option<&str>) {
        self.send(&SseEvent::Error {
            message: message.to_string(),
            traceback: traceback.map(String::from),
        });
    }

    pub fn reload_started(&self, mode: &str, generation: u32) {
        self.send(&SseEvent::ReloadStarted {
            mode: mode.to_string(),
            generation,
        });
    }

    pub fn reload_completed(&self, mode: &str, generation: u32) {
        self.send(&SseEvent::ReloadCompleted {
            mode: mode.to_string(),
            generation,
        });
    }
}
