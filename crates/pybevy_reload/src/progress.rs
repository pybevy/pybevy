use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use bevy::{ecs::world::World, prelude::Resource};

use crate::state::ReloadMode;

/// Stable, backend-neutral phases of a hot-reload attempt.
///
/// A Full reload emits every phase. A Partial reload skips cleanup and
/// Startup unless it is escalated to Full by the orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadProgressPhase {
    DefinitionsLoading,
    DefinitionsReady,
    CleanupStarted,
    CleanupFinished,
    Registering,
    StartupStarted,
    StartupFinished,
    Complete,
}

/// Progress emitted synchronously while a reload attempt is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReloadProgress {
    pub phase: ReloadProgressPhase,
    pub generation: u32,
    pub mode: ReloadMode,
}

impl ReloadProgress {
    pub const fn new(phase: ReloadProgressPhase, generation: u32, mode: ReloadMode) -> Self {
        Self {
            phase,
            generation,
            mode,
        }
    }
}

/// Receives progress synchronously, allowing hosts to update UI while a
/// reload is still executing.
pub trait ReloadProgressSink: Send + Sync + 'static {
    fn report(&self, progress: ReloadProgress);
}

impl<F> ReloadProgressSink for F
where
    F: Fn(ReloadProgress) + Send + Sync + 'static,
{
    fn report(&self, progress: ReloadProgress) {
        self(progress);
    }
}

/// App-scoped progress destination.
///
/// Cloning the resource only clones the sink's `Arc`. The orchestrator clones
/// that `Arc` and releases the World borrow before invoking user code, so a
/// callback never runs under a global lock or a resource borrow.
#[derive(Resource, Clone)]
pub struct ReloadProgressReporter {
    sink: Arc<dyn ReloadProgressSink>,
}

impl ReloadProgressReporter {
    pub fn new(sink: impl ReloadProgressSink) -> Self {
        Self {
            sink: Arc::new(sink),
        }
    }

    pub fn from_shared(sink: Arc<dyn ReloadProgressSink>) -> Self {
        Self { sink }
    }

    fn shared_sink(&self) -> Arc<dyn ReloadProgressSink> {
        self.sink.clone()
    }
}

pub(crate) fn emit_reload_progress(world: &World, progress: ReloadProgress) {
    let sink = world
        .get_resource::<ReloadProgressReporter>()
        .map(ReloadProgressReporter::shared_sink);

    if let Some(sink) = sink {
        // Progress reporting is observational and must never abort or roll
        // back the reload it is observing.
        let _ = catch_unwind(AssertUnwindSafe(|| sink.report(progress)));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    fn progress(phase: ReloadProgressPhase) -> ReloadProgress {
        ReloadProgress::new(phase, 7, ReloadMode::Full)
    }

    #[test]
    fn reporter_is_scoped_to_its_world() {
        let first_events = Arc::new(Mutex::new(Vec::new()));
        let second_events = Arc::new(Mutex::new(Vec::new()));
        let mut first_world = World::new();
        let mut second_world = World::new();

        let first_capture = first_events.clone();
        first_world.insert_resource(ReloadProgressReporter::new(move |event| {
            first_capture.lock().unwrap().push(event);
        }));
        let second_capture = second_events.clone();
        second_world.insert_resource(ReloadProgressReporter::new(move |event| {
            second_capture.lock().unwrap().push(event);
        }));

        emit_reload_progress(
            &first_world,
            progress(ReloadProgressPhase::DefinitionsLoading),
        );
        emit_reload_progress(
            &second_world,
            progress(ReloadProgressPhase::DefinitionsReady),
        );

        assert_eq!(
            *first_events.lock().unwrap(),
            vec![progress(ReloadProgressPhase::DefinitionsLoading)]
        );
        assert_eq!(
            *second_events.lock().unwrap(),
            vec![progress(ReloadProgressPhase::DefinitionsReady)]
        );
    }

    #[test]
    fn reporter_panic_does_not_escape() {
        let mut world = World::new();
        world.insert_resource(ReloadProgressReporter::new(|_| panic!("reporter failed")));

        emit_reload_progress(&world, progress(ReloadProgressPhase::Complete));
    }

    #[test]
    fn missing_reporter_is_a_noop() {
        emit_reload_progress(
            &World::new(),
            progress(ReloadProgressPhase::DefinitionsLoading),
        );
    }
}
