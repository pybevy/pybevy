use bevy::{
    ecs::{
        resource::Resource,
        schedule::{ScheduleCleanupPolicy, Schedules},
        world::World,
    },
    input::{ButtonInput, keyboard::KeyCode},
    platform::time::Instant,
    time::{Real, Time},
};
use pybevy_reload::{
    HotReloadStats, MemoryOverlayVisible, ReloadGenerationSet, ReloadMode, ReloadProgress,
    ReloadProgressPhase, StartPaused, emit_reload_progress, is_verbose, perform_reload,
};
use pyo3::prelude::*;

use super::{
    runtime_pyo3::Pyo3ReloadRuntime,
    state::{HotReloadResource, HotReloadState},
};

/// Retired generations whose Bevy schedule nodes must be physically removed.
/// This is processed in a dedicated schedule after `Last`, never while a
/// target schedule is executing.
#[derive(Resource, Default)]
pub(crate) struct PendingScheduleCompaction(Vec<u32>);

pub(crate) fn queue_schedule_compaction(world: &mut World, generations: Vec<u32>) {
    if generations.is_empty() {
        return;
    }
    let mut pending = world.get_resource_or_insert_with(PendingScheduleCompaction::default);
    for generation in generations {
        if !pending.0.contains(&generation) {
            pending.0.push(generation);
        }
    }
}

pub(crate) fn compact_retired_generation_systems(world: &mut World) {
    let generations = world
        .get_resource_mut::<PendingScheduleCompaction>()
        .map(|mut pending| std::mem::take(&mut pending.0))
        .unwrap_or_default();
    if generations.is_empty() {
        return;
    }

    let labels: Vec<_> = world
        .resource::<Schedules>()
        .iter()
        .map(|(_, schedule)| schedule.label())
        .collect();
    for label in labels {
        world.schedule_scope(label, |world, schedule| {
            for generation in &generations {
                // A generation may not have registered in each schedule.
                let _ = schedule.remove_systems_in_set(
                    ReloadGenerationSet(*generation),
                    world,
                    ScheduleCleanupPolicy::RemoveSetAndSystems,
                );
            }
            let _ = schedule.initialize(world);
        });
    }
}

fn publish_reload_error(world: &mut World, message: String) {
    publish_reload_diagnostic(world, message, None);
}

fn publish_reload_diagnostic(world: &mut World, message: String, traceback: Option<String>) {
    let timestamp = world
        .get_resource::<Time<Real>>()
        .map(|time| time.elapsed_secs_f64())
        .unwrap_or(0.0);
    {
        let mut last_error =
            world.get_resource_or_insert_with(pybevy_core::LastSystemError::default);
        last_error.error = Some(message.clone());
        last_error.traceback = traceback.clone();
        last_error.timestamp_secs = timestamp;
    }
    let mut result = world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
    result.failed = true;
    result.failure_reason = Some(message);
    result.failure_traceback = traceback;
    result.running_previous_generation = true;
}

fn run_definition_reload_attempt(
    world: &mut World,
    loader_func: Py<PyAny>,
    mode: ReloadMode,
    error_state: std::sync::Arc<std::sync::Mutex<Vec<PyErr>>>,
    hot_reload_state: HotReloadState,
) {
    let mut runtime = Pyo3ReloadRuntime::new(loader_func, error_state);
    if let Err(error) = perform_reload(world, &mut runtime, mode, &hot_reload_state) {
        let traceback = error.traceback.clone();
        publish_reload_diagnostic(world, error.message, traceback);
    }
}

/// Built-in system that checks for F5/F6 keypress and triggers reload or mode toggle
/// This runs automatically when hot reload is enabled
pub(crate) fn handle_f5_reload_system(world: &mut World) {
    // Read all key states upfront, then drop the immutable borrow
    let (f5_pressed, f6_pressed, f7_pressed, space_pressed) = {
        let Some(input) = world.get_resource::<ButtonInput<KeyCode>>() else {
            return;
        };
        (
            input.just_pressed(KeyCode::F5),
            input.just_pressed(KeyCode::F6),
            input.just_pressed(KeyCode::F7),
            input.just_pressed(KeyCode::Space),
        )
    };

    // Check if Space was pressed while in paused mode
    if space_pressed {
        let is_paused = world.get_resource::<StartPaused>().is_some_and(|p| p.0);
        if is_paused {
            eprintln!("▶ Space pressed: loading scene...");
            if let Some(mut paused) = world.get_resource_mut::<StartPaused>() {
                paused.0 = false;
            }
            if let Some(reload_res) = world.get_resource::<HotReloadResource>() {
                reload_res.state.request_reload(ReloadMode::Full);
            }
        }
    }

    // Check if F5 was just pressed (full reload)
    if f5_pressed {
        // Enforce a frame-based cooldown to let the render pipeline finish processing
        // entity despawns before spawning new ones. Without this, rapid reloads can
        // corrupt GPU buffer state and permanently degrade FPS.
        const RELOAD_COOLDOWN_FRAMES: u32 = 5;

        let current_frame = world
            .get_resource::<bevy::diagnostic::FrameCount>()
            .map(|f| f.0)
            .unwrap_or(0);
        let last_reload_frame = world
            .get_resource::<HotReloadStats>()
            .map(|s| s.last_reload_frame)
            .unwrap_or(0);
        let frames_since = current_frame.saturating_sub(last_reload_frame);

        if last_reload_frame > 0 && frames_since < RELOAD_COOLDOWN_FRAMES {
            // Skip: render pipeline still syncing
        } else {
            if is_verbose() {
                eprintln!("🔄 F5 pressed! Triggering full reload...");
            }
            // Clear paused state if active
            if let Some(mut paused) = world.get_resource_mut::<StartPaused>()
                && paused.0
            {
                paused.0 = false;
            }
            if let Some(reload_res) = world.get_resource::<HotReloadResource>() {
                reload_res.state.request_reload(ReloadMode::Full);
            }
        }
    }

    // Check if F6 was just pressed (toggle default mode)
    if f6_pressed {
        // Toggle mode in both HotReloadStats (for display) and HotReloadResource (for CLI access)
        let new_mode = if let Some(mut stats) = world.get_resource_mut::<HotReloadStats>() {
            stats.default_mode = match stats.default_mode {
                ReloadMode::Full => ReloadMode::Partial,
                ReloadMode::Partial => ReloadMode::Full,
            };
            Some(stats.default_mode)
        } else {
            None
        };

        // Sync the mode to HotReloadResource so CLI can access it
        if let (Some(mode), Some(reload_res)) =
            (new_mode, world.get_resource::<HotReloadResource>())
        {
            reload_res.state.set_default_mode(mode);

            if is_verbose() {
                eprintln!("🔄 F6 pressed! Default mode toggled to: {:?}", mode);
            }
        }
    }

    // Check if F7 was just pressed (toggle memory overlay)
    if f7_pressed && let Some(mut visible) = world.get_resource_mut::<MemoryOverlayVisible>() {
        visible.0 = !visible.0;
        if is_verbose() {
            eprintln!(
                "📊 F7 pressed! Memory overlay: {}",
                if visible.0 { "ON" } else { "OFF" }
            );
        }
    }
}

/// System that runs each frame to check for hot reload requests
pub fn check_hot_reload_system(world: &mut World) {
    // Check for MCP reload requests (cross-crate mailbox from pybevy_core)
    if let Some(mut mcp_request) = world.get_resource_mut::<pybevy_core::PendingReloadRequest>()
        && let Some(mcp_mode) = mcp_request.mode.take()
        && let Some(reload_res) = world.get_resource::<HotReloadResource>()
    {
        let mode = match mcp_mode {
            pybevy_core::ReloadRequestMode::Full => ReloadMode::Full,
            pybevy_core::ReloadRequestMode::Partial => ReloadMode::Partial,
        };
        reload_res.state.request_reload(mode);
    }

    // First check if reload is pending WITHOUT acquiring GIL (fast path)
    let is_reload_pending = {
        let reload_res = match world.get_resource::<HotReloadResource>() {
            Some(res) => res,
            None => return, // No reload resource, skip
        };

        reload_res.state.is_reload_pending()
    };

    // If no reload pending, return early (99.99% of frames)
    if !is_reload_pending {
        return;
    }

    // Reload is pending - now acquire GIL briefly to get the loader func and mode
    let reload_data = Python::attach(|py| {
        // Take the pending reload (consumes the flag)
        let reload_res = world.resource::<HotReloadResource>();
        let reload_info = reload_res.state.take_pending_reload(py);

        let (loader_func, mode) = match reload_info {
            Some((func, mode)) => (func, mode),
            None => return None, // Already consumed by another thread
        };

        // Get the error state and state for passing to reload
        let error_state = reload_res.error_state.clone();
        let hot_reload_state = reload_res.state.clone();

        Some((loader_func, mode, error_state, hot_reload_state))
    });

    // If we got the loader func, construct runtime and perform reload
    if let Some((loader_func, mode, error_state, hot_reload_state)) = reload_data {
        run_definition_reload_attempt(world, loader_func, mode, error_state, hot_reload_state);
    }
}
