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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_sends_event() {
        let broadcaster = SseEventBroadcaster::new();
        let mut rx = broadcaster.tx.subscribe();
        broadcaster.log("test message", "info");
        let msg = rx.try_recv().unwrap();
        assert!(msg.contains("test message"));
        assert!(msg.contains("info"));
    }

    #[test]
    fn error_sends_event() {
        let broadcaster = SseEventBroadcaster::new();
        let mut rx = broadcaster.tx.subscribe();
        broadcaster.error("oops", Some("traceback here"));
        let msg = rx.try_recv().unwrap();
        assert!(msg.contains("oops"));
        assert!(msg.contains("traceback here"));
    }

    #[test]
    fn error_without_traceback() {
        let broadcaster = SseEventBroadcaster::new();
        let mut rx = broadcaster.tx.subscribe();
        broadcaster.error("oops", None);
        let msg = rx.try_recv().unwrap();
        assert!(msg.contains("oops"));
    }

    #[test]
    fn reload_started_sends_event() {
        let broadcaster = SseEventBroadcaster::new();
        let mut rx = broadcaster.tx.subscribe();
        broadcaster.reload_started("full", 5);
        let msg = rx.try_recv().unwrap();
        assert!(msg.contains("reload_started"));
        assert!(msg.contains("full"));
    }

    #[test]
    fn reload_completed_sends_event() {
        let broadcaster = SseEventBroadcaster::new();
        let mut rx = broadcaster.tx.subscribe();
        broadcaster.reload_completed("partial", 7);
        let msg = rx.try_recv().unwrap();
        assert!(msg.contains("reload_completed"));
        assert!(msg.contains("partial"));
    }
}
