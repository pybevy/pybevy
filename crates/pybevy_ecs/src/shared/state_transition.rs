//! Interpreter-neutral state-transition ordering and pass admission.
//!
//! Python values, hashing, schedule lookup, and cleanup remain backend leaves.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// The two transition shapes supported by Bevy-style states.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateTransitionKind {
    InitialEnter,
    Change,
}

/// One observable step in a state transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateTransitionStep {
    CommitNew,
    RunExit,
    CleanupExited,
    RunTransition,
    RunEnter,
    CleanupEntered,
}

const INITIAL_ENTER_STEPS: &[StateTransitionStep] = &[
    StateTransitionStep::RunEnter,
    StateTransitionStep::CleanupEntered,
];

const CHANGE_STEPS: &[StateTransitionStep] = &[
    StateTransitionStep::CommitNew,
    StateTransitionStep::RunExit,
    StateTransitionStep::CleanupExited,
    StateTransitionStep::RunTransition,
    StateTransitionStep::RunEnter,
    StateTransitionStep::CleanupEntered,
];

/// A backend-neutral, immutable transition execution plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateTransitionPlan {
    kind: StateTransitionKind,
}

impl StateTransitionPlan {
    pub const fn initial_enter() -> Self {
        Self {
            kind: StateTransitionKind::InitialEnter,
        }
    }

    pub const fn change() -> Self {
        Self {
            kind: StateTransitionKind::Change,
        }
    }

    pub const fn kind(self) -> StateTransitionKind {
        self.kind
    }

    pub const fn steps(self) -> &'static [StateTransitionStep] {
        match self.kind {
            StateTransitionKind::InitialEnter => INITIAL_ENTER_STEPS,
            StateTransitionKind::Change => CHANGE_STEPS,
        }
    }

    /// Execute each step in order, stopping at the first adapter error.
    pub fn run<E>(
        self,
        mut execute: impl FnMut(StateTransitionStep) -> Result<(), E>,
    ) -> Result<(), E> {
        for step in self.steps() {
            execute(*step)?;
        }
        Ok(())
    }
}

/// App-local admission fence for an exclusive transition pass.
#[derive(Clone, Debug, Default)]
pub struct StateTransitionGate {
    processing: Arc<AtomicBool>,
}

impl StateTransitionGate {
    pub fn try_enter(&self) -> Option<StateTransitionPassGuard> {
        self.processing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| StateTransitionPassGuard {
                processing: Arc::clone(&self.processing),
            })
    }
}

/// Releases its owning gate even when an adapter returns early or unwinds.
#[derive(Debug)]
pub struct StateTransitionPassGuard {
    processing: Arc<AtomicBool>,
}

impl Drop for StateTransitionPassGuard {
    fn drop(&mut self) {
        self.processing.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_pin_exact_bevy_and_pybevy_order() {
        assert_eq!(
            StateTransitionPlan::initial_enter().steps(),
            [
                StateTransitionStep::RunEnter,
                StateTransitionStep::CleanupEntered,
            ]
        );
        assert_eq!(
            StateTransitionPlan::change().steps(),
            [
                StateTransitionStep::CommitNew,
                StateTransitionStep::RunExit,
                StateTransitionStep::CleanupExited,
                StateTransitionStep::RunTransition,
                StateTransitionStep::RunEnter,
                StateTransitionStep::CleanupEntered,
            ]
        );
    }

    #[test]
    fn adapter_error_stops_later_steps() {
        let mut seen = Vec::new();
        let result = StateTransitionPlan::change().run(|step| {
            seen.push(step);
            if step == StateTransitionStep::RunTransition {
                Err("transition failed")
            } else {
                Ok(())
            }
        });

        assert_eq!(result, Err("transition failed"));
        assert_eq!(
            seen,
            [
                StateTransitionStep::CommitNew,
                StateTransitionStep::RunExit,
                StateTransitionStep::CleanupExited,
                StateTransitionStep::RunTransition,
            ]
        );
    }

    #[test]
    fn gates_are_reentrant_per_app_and_release_on_drop() {
        let first = StateTransitionGate::default();
        let second = StateTransitionGate::default();

        let first_guard = first.try_enter().expect("first pass starts");
        assert!(first.try_enter().is_none());
        let second_guard = second.try_enter().expect("another App remains independent");

        drop(first_guard);
        assert!(first.try_enter().is_some());
        drop(second_guard);
    }
}
