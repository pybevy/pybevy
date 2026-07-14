//! The shared run scaffold: one function owning the per-run sequence both
//! backends execute, with the interpreter- and reload-specific steps left to
//! the caller.
//!
//! Both the pyo3 and the rustpython `DynamicSystem::run_unsafe` bodies follow
//! the same skeleton: a generation guard, a change-tick advance, a
//! `ValidityFlag` fenced by an RAII guard, a local `CommandQueue`, the actual
//! interpreter call, then an end-of-run tick re-read. Only the middle (enter
//! the interpreter runtime, build the arguments, call the function, route the
//! error) touches types the shared crate cannot see (`Py<PyAny>` vs
//! `PyObjectRef`, the reload/profiler resources), so that middle is passed in
//! as `run_interpreted`. Keeping the ordering here means it cannot drift
//! between the two backends the way the access-declaration logic did before the
//! earlier extraction steps.
//!
//! The scaffold owns the local `CommandQueue` but not the `Commands`/`PyCommands`
//! wrappers: the pyo3 backend builds a Bevy `Commands` from the queue while the
//! rustpython backend hands a raw queue pointer to its lazy `PyCommands`, so the
//! queue is the shared unit and each backend wraps it its own way.

use std::time::Duration;

use bevy::{
    ecs::{
        change_detection::Tick,
        world::{CommandQueue, unsafe_world_cell::UnsafeWorldCell},
    },
    platform::time::Instant,
};
use pybevy_storage::{ValidityFlag, ValidityGuard};

/// The change-detection window for one system run: `last_run` opens it and
/// `this_run` is the freshly advanced world tick every query, view, and
/// write-back observes.
#[derive(Clone, Copy)]
pub struct RunTicks {
    pub last_run: Tick,
    pub this_run: Tick,
}

/// Inputs the scaffold hands to the interpreter body for one run.
pub struct RunCtx<'w> {
    /// This run's world cell (valid for the duration of `run_interpreted`).
    pub world: UnsafeWorldCell<'w>,
    /// The run's change-detection ticks.
    pub ticks: RunTicks,
    /// The current hot-reload generation, or `None` when reload is inactive.
    /// The pyo3 backend uses it to decide whether to re-import the function;
    /// the rustpython backend ignores it.
    pub current_generation: Option<u32>,
}

/// What the scaffold produces for the caller's epilogue to finish with.
pub enum RunScaffoldResult {
    /// The generation guard tripped: this system belongs to a superseded hot
    /// reload and did not run.
    SkippedStaleGeneration,
    /// The system ran.
    Ran {
        /// The interpreted boolean return value (meaningful for run conditions;
        /// ignored by plain systems).
        bool_result: bool,
        /// Wall-clock time spent in `run_interpreted`, for the profiler epilogue.
        duration: Duration,
        /// The change tick after the run: the next `last_run`. Re-read at the
        /// end (not captured at the start) because pybevy stamps live ticks, so
        /// a start-of-run read would make a system re-observe its own writes.
        end_tick: Tick,
        /// Commands queued during the run, to be appended to the system's
        /// persistent `CommandQueue`.
        local_queue: CommandQueue,
    },
}

/// Own the per-run sequence shared by both backends.
///
/// The caller supplies `run_interpreted`, which enters the interpreter runtime,
/// builds the argument list (wrapping the supplied `CommandQueue` in whatever
/// `Commands` type the backend uses), calls the function, interprets the boolean
/// result, and routes any error into the backend's error channel. Everything
/// around it (generation guard, tick advance, validity, the local queue, timing,
/// and the end-of-run tick) lives here so it cannot diverge between backends.
///
/// # Safety
/// `world` must be this run's valid `UnsafeWorldCell`. `run_interpreted` must
/// not allow any built argument to outlive the run: the `ValidityGuard` created
/// here invalidates the shared `ValidityFlag` before this function returns
/// (including on a panic unwinding through the body), so any Python object that
/// captured a raw pointer derived from `world` stops reading through it.
pub unsafe fn run_scaffold<F>(
    world: UnsafeWorldCell,
    last_run: Tick,
    expected_generation: Option<u32>,
    current_generation: Option<u32>,
    run_interpreted: F,
) -> RunScaffoldResult
where
    F: FnOnce(RunCtx, &ValidityFlag, &mut CommandQueue) -> bool,
{
    // Generation guard: skip zombie systems left over from a superseded reload,
    // in addition to the schedule-level `run_if(generation_matches)`. An absent
    // generation (reload inactive) always runs.
    if let (Some(expected), Some(current)) = (expected_generation, current_generation)
        && current != expected
    {
        return RunScaffoldResult::SkippedStaleGeneration;
    }

    // Advance the change tick once per run (matching Bevy's `FunctionSystem`),
    // reading `change_tick()` AFTER incrementing so live write-backs and every
    // query observe the same freshly advanced `this_run`.
    world.increment_change_tick();
    let ticks = RunTicks {
        last_run,
        this_run: world.change_tick(),
    };

    // Validity flag + RAII guard: invalidated when this function returns
    // (including on panic), so arguments that captured raw world pointers stop
    // reading once the run is over.
    let validity = ValidityFlag::new();
    let _validity_guard = ValidityGuard::new(validity.clone());

    // The local command queue this run's `Commands` write into, appended to the
    // system's persistent queue afterwards.
    let mut local_queue = CommandQueue::default();

    // Enter the interpreter, build args, call, route errors.
    let ctx = RunCtx {
        world,
        ticks,
        current_generation,
    };
    let start = Instant::now();
    let bool_result = run_interpreted(ctx, &validity, &mut local_queue);
    let duration = start.elapsed();

    let end_tick = world.change_tick();

    RunScaffoldResult::Ran {
        bool_result,
        duration,
        end_tick,
        local_queue,
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::World;

    use super::*;

    #[test]
    fn stale_generation_skips_the_body() {
        let mut world = World::new();
        let cell = world.as_unsafe_world_cell();
        let mut ran = false;
        let result = unsafe {
            run_scaffold(cell, Tick::new(0), Some(5), Some(3), |_ctx, _v, _q| {
                ran = true;
                true
            })
        };
        assert!(matches!(result, RunScaffoldResult::SkippedStaleGeneration));
        assert!(!ran, "body must not run when the generation is stale");
    }

    #[test]
    fn matching_generation_runs_and_advances_the_tick() {
        let mut world = World::new();
        let before = world.change_tick().get();
        let cell = world.as_unsafe_world_cell();
        let result = unsafe {
            run_scaffold(cell, Tick::new(1), Some(4), Some(4), |ctx, _v, _q| {
                assert!(
                    ctx.ticks.this_run.get() > before,
                    "tick advanced before body"
                );
                true
            })
        };
        match result {
            RunScaffoldResult::Ran {
                bool_result,
                end_tick,
                ..
            } => {
                assert!(bool_result);
                assert!(end_tick.get() >= before);
            }
            RunScaffoldResult::SkippedStaleGeneration => panic!("expected Ran"),
        }
    }

    #[test]
    fn absent_generation_runs() {
        let mut world = World::new();
        let cell = world.as_unsafe_world_cell();
        let mut ran = false;
        let _ = unsafe {
            run_scaffold(cell, Tick::new(0), Some(4), None, |_c, _v, _q| {
                ran = true;
                false
            })
        };
        assert!(
            ran,
            "an absent generation resource means reload is inactive: run"
        );
    }

    #[test]
    fn body_sees_the_local_queue() {
        let mut world = World::new();
        let cell = world.as_unsafe_world_cell();
        let result = unsafe {
            run_scaffold(cell, Tick::new(0), None, None, |_ctx, _v, queue| {
                // The queue is empty and writable; the backend wraps it in its
                // own Commands type. Just prove it is handed through.
                queue.is_empty();
                true
            })
        };
        assert!(matches!(result, RunScaffoldResult::Ran { .. }));
    }
}
