use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

use bevy::prelude::Resource;
use tokio::sync::{mpsc, oneshot};

use crate::protocol::{ControlError, ControlOperation};

/// A request sent from the HTTP server thread to the Bevy exclusive system
pub struct ControlRequest {
    pub operation: ControlOperation,
    pub response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
}

/// Bevy resource holding the receive end of the control channel
#[derive(Resource)]
pub struct ControlReceiver {
    pub rx: mpsc::UnboundedReceiver<ControlRequest>,
}

/// App-local gate shared by the HTTP server and the exclusive control system.
///
/// While `run_code` owns the gate, another control request cannot be serviced:
/// the engine thread is executing Python with exclusive World access. HTTP
/// admission checks this state before queueing so a callback into the same
/// server fails immediately instead of waiting on the World it already owns.
#[derive(Resource, Clone, Default)]
pub(crate) struct SharedExclusiveExecution {
    active: Arc<AtomicBool>,
}

impl SharedExclusiveExecution {
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub(crate) fn try_enter(&self) -> Option<ExclusiveExecutionGuard> {
        self.active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| ExclusiveExecutionGuard {
                state: self.clone(),
            })
    }
}

pub(crate) struct ExclusiveExecutionGuard {
    state: SharedExclusiveExecution,
}

impl Drop for ExclusiveExecutionGuard {
    fn drop(&mut self) {
        self.state.active.store(false, Ordering::Release);
    }
}

/// When the main loop last started and finished a frame, read by the HTTP
/// thread to tell a busy main thread from an idle loop.
#[derive(Resource, Clone)]
pub struct FrameHeartbeat {
    inner: Arc<HeartbeatInner>,
}

struct HeartbeatInner {
    epoch: Instant,
    in_frame: AtomicBool,
    started_ms: AtomicU64,
    finished_ms: AtomicU64,
}

/// Seconds the main thread has spent in the current frame, or since its last one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameActivity {
    InFrame { secs: f64 },
    BetweenFrames { secs: f64 },
}

impl Default for FrameHeartbeat {
    fn default() -> Self {
        Self {
            inner: Arc::new(HeartbeatInner {
                epoch: Instant::now(),
                in_frame: AtomicBool::new(false),
                started_ms: AtomicU64::new(0),
                finished_ms: AtomicU64::new(0),
            }),
        }
    }
}

impl FrameHeartbeat {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.inner.epoch.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    pub fn mark_started(&self) {
        self.inner
            .started_ms
            .store(self.now_ms(), Ordering::Release);
        self.inner.in_frame.store(true, Ordering::Release);
    }

    pub fn mark_finished(&self) {
        self.inner
            .finished_ms
            .store(self.now_ms(), Ordering::Release);
        self.inner.in_frame.store(false, Ordering::Release);
    }

    pub fn activity(&self) -> FrameActivity {
        let in_frame = self.inner.in_frame.load(Ordering::Acquire);
        let now = self.now_ms();
        let secs =
            |since: &AtomicU64| now.saturating_sub(since.load(Ordering::Acquire)) as f64 / 1000.0;
        if in_frame {
            FrameActivity::InFrame {
                secs: secs(&self.inner.started_ms),
            }
        } else {
            FrameActivity::BetweenFrames {
                secs: secs(&self.inner.finished_ms),
            }
        }
    }
}

/// Clonable sender for the control channel (stored in HTTP server state)
#[derive(Clone)]
pub struct ControlSender {
    pub tx: mpsc::UnboundedSender<ControlRequest>,
    exclusive_execution: SharedExclusiveExecution,
    heartbeat: FrameHeartbeat,
}

impl ControlSender {
    pub(crate) fn exclusive_execution(&self) -> SharedExclusiveExecution {
        self.exclusive_execution.clone()
    }

    pub fn heartbeat(&self) -> FrameHeartbeat {
        self.heartbeat.clone()
    }
}

/// Create the mpsc channel pair
pub fn create_channel() -> (ControlSender, ControlReceiver) {
    let (tx, rx) = mpsc::unbounded_channel();
    (
        ControlSender {
            tx,
            exclusive_execution: SharedExclusiveExecution::default(),
            heartbeat: FrameHeartbeat::default(),
        },
        ControlReceiver { rx },
    )
}

#[cfg(test)]
mod tests {
    use std::panic;

    use tokio::sync::oneshot;

    use super::*;

    #[test]
    fn create_channel_works() {
        let (sender, mut receiver) = create_channel();
        let (tx, _rx) = oneshot::channel();

        sender
            .tx
            .send(ControlRequest {
                operation: ControlOperation::ListEntities,
                response_tx: tx,
            })
            .unwrap();

        let req = receiver.rx.try_recv().unwrap();
        assert!(matches!(req.operation, ControlOperation::ListEntities));
    }
    #[test]
    fn exclusive_execution_guard_is_app_local_and_raii_scoped() {
        let (sender_a, _receiver_a) = create_channel();
        let (sender_b, _receiver_b) = create_channel();
        let state_a = sender_a.exclusive_execution();
        let state_b = sender_b.exclusive_execution();

        assert!(!state_a.is_active());
        let guard = state_a.try_enter().expect("first entry should succeed");
        assert!(state_a.is_active());
        assert!(state_a.try_enter().is_none());
        assert!(!state_b.is_active());

        drop(guard);
        assert!(!state_a.is_active());
        assert!(state_a.try_enter().is_some());
    }

    #[test]
    fn heartbeat_tells_a_frame_in_progress_from_an_idle_loop() {
        let heartbeat = FrameHeartbeat::default();
        assert!(matches!(
            heartbeat.activity(),
            FrameActivity::BetweenFrames { .. }
        ));
        std::thread::sleep(std::time::Duration::from_millis(5));
        heartbeat.mark_started();
        assert!(matches!(
            heartbeat.activity(),
            FrameActivity::InFrame { .. }
        ));
        std::thread::sleep(std::time::Duration::from_millis(5));
        heartbeat.mark_finished();
        assert!(matches!(
            heartbeat.activity(),
            FrameActivity::BetweenFrames { .. }
        ));
    }

    #[test]
    fn exclusive_execution_guard_clears_on_unwind() {
        let (sender, _receiver) = create_channel();
        let state = sender.exclusive_execution();
        let unwind_state = state.clone();

        let result = panic::catch_unwind(move || {
            let _guard = unwind_state.try_enter().expect("entry should succeed");
            panic!("test unwind");
        });

        assert!(result.is_err());
        assert!(!state.is_active());
    }
}
