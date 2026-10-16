use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
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

/// Clonable sender for the control channel (stored in HTTP server state)
#[derive(Clone)]
pub struct ControlSender {
    pub tx: mpsc::UnboundedSender<ControlRequest>,
    exclusive_execution: SharedExclusiveExecution,
}

impl ControlSender {
    pub(crate) fn exclusive_execution(&self) -> SharedExclusiveExecution {
        self.exclusive_execution.clone()
    }
}

/// Create the mpsc channel pair
pub fn create_channel() -> (ControlSender, ControlReceiver) {
    let (tx, rx) = mpsc::unbounded_channel();
    (
        ControlSender {
            tx,
            exclusive_execution: SharedExclusiveExecution::default(),
        },
        ControlReceiver { rx },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_channel_works() {
        let (sender, mut receiver) = create_channel();
        let (tx, _rx) = tokio::sync::oneshot::channel();

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
    fn exclusive_execution_guard_clears_on_unwind() {
        let (sender, _receiver) = create_channel();
        let state = sender.exclusive_execution();
        let unwind_state = state.clone();

        let result = std::panic::catch_unwind(move || {
            let _guard = unwind_state.try_enter().expect("entry should succeed");
            panic!("test unwind");
        });

        assert!(result.is_err());
        assert!(!state.is_active());
    }
}
