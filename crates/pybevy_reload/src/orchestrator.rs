use std::collections::HashSet;

use bevy::{
    app::{MainScheduleOrder, Startup},
    ecs::{entity::Entity, resource::IsResource, schedule::ScheduleLabel, world::World},
    platform::time::Instant,
    prelude::Without,
    time::{Real, Time},
};
use pybevy_core::PluginIdentity;

use crate::{
    BaseEntitySet, ReloadStartupScheduleOrder,
    cleanup::{NativeResourceSnapshot, clear_world_state},
    profiling::{HotReloadStats, MemoryProfile, SystemProfiler},
    progress::{ReloadProgress, ReloadProgressPhase, emit_reload_progress},
    runtime::{EscalationTracker, ReloadError, ReloadRuntime},
    state::{HotReloadGeneration, ReloadMode},
    tracker::PluginTracker,
    util::{count_schedule_systems, get_current_rss_mb, is_verbose},
};

/// Perform the actual reload by incrementing generation and optionally clearing entities.
///
/// Uses a validate-then-commit pattern: the runtime's load_definitions is called BEFORE
/// the generation is incremented, so if loading fails, the old generation's systems
/// keep running and the app doesn't freeze.
///
/// Generic over `ReloadRuntime` — the orchestration logic (generation tracking, entity
/// cleanup, stats, rollback) is shared. The runtime handles loading definitions,
/// registering systems/resources/messages/observers, and GC.
///
/// The `hot_reload_state` parameter provides access to generation counter manipulation
/// (increment, set, current_generation). It's kept as a trait to decouple from the
/// PyO3-specific HotReloadState.
pub fn perform_reload<R: ReloadRuntime, S: HotReloadStateAccess>(
    world: &mut World,
    runtime: &mut R,
    mode: ReloadMode,
    hot_reload_state: &S,
) -> Result<(), ReloadError> {
    if is_verbose() {
        eprintln!("🔄 [Hot Reload] Starting {:?} reload", mode);
    }

    let old_generation = hot_reload_state.current_generation();

    // Validate first: load definitions before committing.
    let next_generation = old_generation + 1;
    emit_reload_progress(
        world,
        ReloadProgress::new(
            ReloadProgressPhase::DefinitionsLoading,
            next_generation,
            mode,
        ),
    );
    let defs = match runtime.load_definitions(next_generation) {
        Ok(defs) => defs,
        Err(e) => {
            if runtime.load_error_is_deferred(&e) {
                return Err(e);
            }
            runtime.print_error(&e);

            let current_time = world
                .get_resource::<bevy::time::Time>()
                .map(|t| t.elapsed_secs_f64())
                .unwrap_or(0.0);
            let current_frame = world
                .get_resource::<bevy::diagnostic::FrameCount>()
                .map(|f| f.0)
                .unwrap_or(0);
            if let Some(mut stats) = world.get_resource_mut::<HotReloadStats>() {
                stats.reload_count += 1;
                stats.last_mode = Some(mode);
                stats.last_reload_time = current_time;
                stats.last_reload_frame = current_frame;
            }
            let mut result = world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
            result.failed = true;
            result.failure_reason = Some(e.message.clone());
            result.failure_traceback = e.traceback.clone();
            result.running_previous_generation = true;

            return Err(e);
        }
    };
    emit_reload_progress(
        world,
        ReloadProgress::new(ReloadProgressPhase::DefinitionsReady, next_generation, mode),
    );

    // Commit: increment generation (point of no return).
    hot_reload_state.increment_generation();
    let new_generation = hot_reload_state.current_generation();

    if is_verbose() {
        eprintln!("   → New generation: {}", new_generation);
    }

    {
        let mut gen_res = world.resource_mut::<HotReloadGeneration>();
        gen_res.update();
    }

    // Update stats
    if let Some(time) = world.get_resource::<bevy::time::Time>() {
        let current_time = time.elapsed_secs_f64();
        let current_frame = world
            .get_resource::<bevy::diagnostic::FrameCount>()
            .map(|f| f.0)
            .unwrap_or(0);
        if let Some(mut stats) = world.get_resource_mut::<HotReloadStats>() {
            stats.last_mode = Some(mode);
            stats.last_reload_time = current_time;
            stats.last_reload_frame = current_frame;
            stats.reload_count += 1;
        }
    }

    if let Some(profiler) = world.get_resource::<SystemProfiler>() {
        profiler.clear();
    }

    // Capture initial native resource state before first reload clears anything.
    // This records which bridged resources are Bevy-plugin defaults vs user-inserted.
    if !world.contains_resource::<NativeResourceSnapshot>() {
        let initial = runtime.snapshot_native_resources(world);
        world.insert_resource(NativeResourceSnapshot { initial });
    }

    // NOTE: BaseEntitySet is captured in add_hot_reload_system() (bindings.rs),
    // before any user Startup systems run. If it's missing here (e.g., in unit
    // tests that bypass the full init path), fall back to an empty set so that
    // all entities are eligible for despawn.
    if !world.contains_resource::<BaseEntitySet>() {
        world.insert_resource(BaseEntitySet {
            entities: HashSet::new(),
        });
    }

    if mode == ReloadMode::Full {
        emit_reload_progress(
            world,
            ReloadProgress::new(ReloadProgressPhase::CleanupStarted, new_generation, mode),
        );
        clear_world_state(world, runtime, is_verbose());
        emit_reload_progress(
            world,
            ReloadProgress::new(ReloadProgressPhase::CleanupFinished, new_generation, mode),
        );
    }

    // Reset reload result tracking
    {
        let mut result = world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
        result.escalated = false;
        result.escalation_reason = None;
        result.failed = false;
        result.failure_reason = None;
        result.failure_traceback = None;
        result.running_previous_generation = false;
        result.plugins_added = None;
        result.plugins_removed = None;
        result.systems_removed = None;
        result.actual_mode = Some(match mode {
            ReloadMode::Full => pybevy_core::ReloadRequestMode::Full,
            ReloadMode::Partial => pybevy_core::ReloadRequestMode::Partial,
        });
    }

    // Auto-escalation: Partial -> Full when the new definitions contain
    // changes only a Full reload applies (Startup re-run, resource insertion,
    // observer re-registration). Compared against the previous generation's
    // fingerprint; an unchanged file stays on the fast Partial path.
    let fingerprint = runtime.defs_fingerprint(&defs);
    let mut mode = mode;
    if mode == ReloadMode::Partial {
        let recovery_required = world
            .get_resource::<EscalationTracker>()
            .is_some_and(|tracker| tracker.full_reload_required);
        let previous = world
            .get_resource::<EscalationTracker>()
            .and_then(|tracker| tracker.last.clone());
        let reason = if recovery_required {
            Some("recovering from failed Full reload")
        } else {
            match &previous {
                Some(previous) => {
                    if fingerprint.component_layout_changed {
                        Some("custom component layout changed")
                    } else if previous.startup_code != fingerprint.startup_code {
                        Some("Startup systems changed")
                    } else if previous.resource_types != fingerprint.resource_types {
                        Some("resource definitions changed")
                    } else if previous.observer_code != fingerprint.observer_code {
                        Some("observers changed")
                    } else {
                        None
                    }
                }
                // No baseline yet: conservatively escalate if the scene defines
                // anything only a Full reload applies.
                None if fingerprint.component_layout_changed => {
                    Some("custom component layout changed")
                }
                None => {
                    match (
                        fingerprint.has_startup,
                        fingerprint.has_resources,
                        fingerprint.has_observers,
                    ) {
                        (true, _, _) => Some("no fingerprint baseline, Startup systems present"),
                        (false, true, _) => Some("no fingerprint baseline, resources present"),
                        (false, false, true) => Some("no fingerprint baseline, observers present"),
                        _ => None,
                    }
                }
            }
        };
        if let Some(reason) = reason {
            eprintln!("⬆️ [Hot Reload] Escalating Partial → Full: {reason}");
            mode = ReloadMode::Full;
            emit_reload_progress(
                world,
                ReloadProgress::new(ReloadProgressPhase::CleanupStarted, new_generation, mode),
            );
            clear_world_state(world, runtime, is_verbose());
            emit_reload_progress(
                world,
                ReloadProgress::new(ReloadProgressPhase::CleanupFinished, new_generation, mode),
            );

            if let Some(mut stats) = world.get_resource_mut::<HotReloadStats>() {
                stats.last_mode = Some(ReloadMode::Full);
            }
            if let Some(mut result) = world.get_resource_mut::<pybevy_core::ReloadResult>() {
                result.escalated = true;
                result.escalation_reason = Some(reason.to_string());
                result.actual_mode = Some(pybevy_core::ReloadRequestMode::Full);
            }
        }
    }
    // Plugin delta detection
    {
        let plugin_names = runtime.plugin_names(&defs);
        let new_plugin_set: HashSet<PluginIdentity> = plugin_names.into_iter().collect();

        let (mut added, mut removed) = {
            if let Some(mut tracker) = world.get_resource_mut::<PluginTracker>() {
                if !tracker.baseline_initialized {
                    tracker.known_plugins = new_plugin_set;
                    tracker.baseline_initialized = true;
                    (Vec::new(), Vec::new())
                } else {
                    let added = new_plugin_set
                        .difference(&tracker.known_plugins)
                        .cloned()
                        .collect();
                    let removed = tracker
                        .known_plugins
                        .difference(&new_plugin_set)
                        .cloned()
                        .collect();
                    (added, removed)
                }
            } else {
                (Vec::new(), Vec::new())
            }
        };
        added.sort();
        removed.sort();

        if !added.is_empty() || !removed.is_empty() {
            let added: Vec<String> = added
                .into_iter()
                .map(|plugin| plugin.report_name())
                .collect();
            let removed: Vec<String> = removed
                .into_iter()
                .map(|plugin| plugin.report_name())
                .collect();
            if !added.is_empty() {
                eprintln!(
                    "⚠️ [Hot Reload] New plugins detected (restart may be required): {:?}",
                    added
                );
            }
            if !removed.is_empty() {
                eprintln!(
                    "⚠️ [Hot Reload] Plugins removed (restart required to take effect): {:?}",
                    removed
                );
            }
            if let Some(mut result) = world.get_resource_mut::<pybevy_core::ReloadResult>() {
                if !added.is_empty() {
                    result.plugins_added = Some(added);
                }
                if !removed.is_empty() {
                    result.plugins_removed = Some(removed);
                }
            }
        }
    }

    emit_reload_progress(
        world,
        ReloadProgress::new(ReloadProgressPhase::Registering, new_generation, mode),
    );

    if let Err(e) = runtime.register_states(world, &defs, mode) {
        fail_registration(world, hot_reload_state, old_generation, mode, &e);
        return Err(e);
    }

    if mode == ReloadMode::Full {
        if let Err(e) = runtime.register_resources(world, &defs) {
            fail_registration(world, hot_reload_state, old_generation, mode, &e);
            return Err(e);
        }
    } else if let Err(e) = runtime.rebind_resources(world, &defs) {
        fail_registration(world, hot_reload_state, old_generation, mode, &e);
        return Err(e);
    }

    if let Err(e) = runtime.register_messages(world, &defs, new_generation) {
        fail_registration(world, hot_reload_state, old_generation, mode, &e);
        return Err(e);
    }

    if mode == ReloadMode::Full
        && let Err(e) = runtime.register_observers(world, &defs)
    {
        fail_registration(world, hot_reload_state, old_generation, mode, &e);
        return Err(e);
    }

    // System delta detection
    {
        let new_system_names = runtime.system_names(&defs);
        let no_reloadable_systems = new_system_names.is_empty();
        let removed = runtime.detect_system_delta(world, new_system_names);
        if !removed.is_empty() {
            if mode == ReloadMode::Partial && no_reloadable_systems {
                eprintln!(
                    "⚠️ [Hot Reload] Partial reload removed the last reloadable Python system; scene logic is now idle"
                );
            } else {
                eprintln!(
                    "⚠️ [Hot Reload] Systems removed/renamed (stale schedule entries remain, use run_scene to clear): {:?}",
                    removed
                );
            }
            if let Some(mut result) = world.get_resource_mut::<pybevy_core::ReloadResult>() {
                result.systems_removed = Some(removed);
            }
        }
    }

    // Register systems
    runtime.clear_param_cache();
    let system_handles = match runtime.register_systems(world, defs, new_generation) {
        Ok(handles) => handles,
        Err(e) => {
            fail_registration(world, hot_reload_state, old_generation, mode, &e);
            return Err(e);
        }
    };

    // A system or observer from the scene being replaced may have failed
    // earlier in this Main schedule, or fire during the reload's own world
    // mutations. Drain the buffer for both modes so the old generation's
    // error is not drained after the reload's clear and misattributed to the
    // candidate scene.
    if let Some(error) = runtime.take_pending_system_error(world)
        && is_verbose()
    {
        eprintln!("   → Replacing scene after system error: {error}");
    }

    // Run the startup schedule sequence with rollback on panic
    // Snapshot pre-Startup error state: both the timestamp and whether an
    // error was already present.  We need both because on the first reload
    // Time hasn't ticked yet, so timestamp is 0.0 for both pre and post,
    // making a pure timestamp comparison fail.
    let (pre_startup_error_ts, pre_startup_had_error) = world
        .get_resource::<pybevy_core::LastSystemError>()
        .map(|e| (e.timestamp_secs, e.error.is_some()))
        .unwrap_or((0.0, false));

    // Snapshot ALL entities before Startup so we can clean up on failure.
    // Snapshot all entities so we can clean up on failure
    // (catches Bevy side-effect entities spawned during a failed Startup).
    let pre_startup_entities: std::collections::HashSet<Entity> = if mode == ReloadMode::Full {
        world
            .query_filtered::<Entity, Without<IsResource>>()
            .iter(world)
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    if mode == ReloadMode::Full {
        let startup_labels = world
            .get_resource::<ReloadStartupScheduleOrder>()
            .map(|order| order.0.clone())
            .or_else(|| {
                world
                    .get_resource::<MainScheduleOrder>()
                    .map(|order| order.startup_labels.clone())
            })
            .unwrap_or_else(|| vec![Startup.intern()]);
        if is_verbose() {
            eprintln!("   → Running startup schedules");
        }

        emit_reload_progress(
            world,
            ReloadProgress::new(ReloadProgressPhase::StartupStarted, new_generation, mode),
        );
        let startup_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for label in startup_labels {
                let _ = world.try_run_schedule(label);
            }
        }));

        world.resource::<HotReloadGeneration>().mark_startup_run();

        if let Err(panic_payload) = startup_result {
            let panic_msg = if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "unknown panic".to_string()
            };

            eprintln!(
                "⚠️ [Hot Reload] Startup panicked: {} — rolling back to generation {}",
                panic_msg, old_generation
            );

            {
                let post_entities: Vec<Entity> = world
                    .query_filtered::<Entity, Without<IsResource>>()
                    .iter(world)
                    .collect();
                let mut cleaned = 0;
                for entity in post_entities {
                    if !pre_startup_entities.contains(&entity) && world.get_entity(entity).is_ok() {
                        world.despawn(entity);
                        cleaned += 1;
                    }
                }
                if is_verbose() && cleaned > 0 {
                    eprintln!(
                        "   → Cleaned up {} entities created during failed Startup",
                        cleaned
                    );
                }
            }

            hot_reload_state.set_generation(old_generation);
            {
                let mut gen_res = world.resource_mut::<HotReloadGeneration>();
                gen_res.current = old_generation;
            }
            {
                // Remove new_generation (not old_generation) since
                // mark_startup_run() inserted new_generation into the set.
                // If we leave it, the next reload that reuses this
                // generation number will skip Startup entirely.
                let gen_res = world.resource::<HotReloadGeneration>();
                gen_res.forget_startup_run(new_generation);
            }

            let error_msg = format!("Startup panicked: {}", panic_msg);
            let mut result = world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
            result.failed = true;
            result.failure_reason = Some(error_msg.clone());
            result.running_previous_generation = true;

            runtime.clear_param_cache();
            runtime.retire_handles(&system_handles);
            require_full_recovery(world);

            return Err(ReloadError {
                message: error_msg,
                traceback: None,
                is_load_failure: false,
            });
        }

        emit_reload_progress(
            world,
            ReloadProgress::new(ReloadProgressPhase::StartupFinished, new_generation, mode),
        );
    } else if is_verbose() {
        eprintln!("   → Skipping startup schedules (Partial mode)");
    }

    let pending_system_error = if mode == ReloadMode::Full {
        runtime.take_pending_system_error(world)
    } else {
        None
    };
    let startup_had_error = pending_system_error.is_some()
        || world
            .get_resource::<pybevy_core::LastSystemError>()
            .is_some_and(|e| {
                e.error.is_some()
                    && (e.timestamp_secs > pre_startup_error_ts || !pre_startup_had_error)
            });

    // If a Startup system raised a Python exception (not a panic), apply
    // the same generation rollback so Update systems from the broken
    // generation don't keep running.  Without this, the new-generation
    // Update systems execute every frame even though their Startup failed
    // to set up the entities/resources they depend on.
    if startup_had_error && mode == ReloadMode::Full {
        let error_msg = pending_system_error
            .or_else(|| {
                world
                    .get_resource::<pybevy_core::LastSystemError>()
                    .and_then(|e| e.error.clone())
            })
            .unwrap_or_else(|| "Startup system error".to_string());

        eprintln!(
            "⚠️ [Hot Reload] Startup system error - rolling back to generation {}",
            old_generation
        );

        // Clean up entities created during the failed Startup (same as
        // the panic path) so we don't leave orphaned render targets,
        // cameras, or other partially-created scene objects.
        {
            let post_entities: Vec<Entity> = world
                .query_filtered::<Entity, Without<IsResource>>()
                .iter(world)
                .collect();
            let mut cleaned = 0;
            for entity in post_entities {
                if !pre_startup_entities.contains(&entity) && world.get_entity(entity).is_ok() {
                    world.despawn(entity);
                    cleaned += 1;
                }
            }
            if is_verbose() && cleaned > 0 {
                eprintln!(
                    "   → Cleaned up {} entities created during failed Startup",
                    cleaned
                );
            }
        }

        hot_reload_state.set_generation(old_generation);
        {
            let mut gen_res = world.resource_mut::<HotReloadGeneration>();
            gen_res.current = old_generation;
        }
        {
            // Remove new_generation (not old_generation) - see panic path comment.
            let gen_res = world.resource::<HotReloadGeneration>();
            gen_res.forget_startup_run(new_generation);
        }

        let failure_traceback = world
            .get_resource::<pybevy_core::LastSystemError>()
            .and_then(|error| error.traceback.clone());
        let mut result = world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
        result.failed = true;
        result.failure_reason = Some(error_msg.clone());
        result.failure_traceback = failure_traceback.clone();
        result.running_previous_generation = true;

        runtime.clear_param_cache();
        runtime.retire_handles(&system_handles);
        require_full_recovery(world);

        return Err(ReloadError {
            message: error_msg,
            traceback: failure_traceback,
            is_load_failure: false,
        });
    }

    if !startup_had_error
        && let Some(mut last_error) = world.get_resource_mut::<pybevy_core::LastSystemError>()
    {
        last_error.error = None;
        last_error.traceback = None;
    }

    runtime.commit_schedule_configs(world);

    {
        let gen_res = world.resource::<HotReloadGeneration>();
        gen_res.retain_startup_runs_since(new_generation.saturating_sub(2));
    }

    // Register handles and gut old-generation systems
    let current_generation_systems = system_handles.len();
    if !system_handles.is_empty() {
        runtime.register_handles(world, new_generation, system_handles);
    }

    runtime.prune_messages(world, new_generation);
    runtime.prune_requests(world, (mode == ReloadMode::Full).then_some(old_generation));

    // Post-reload cleanup
    runtime.clear_param_cache();
    runtime.trigger_gc();

    {
        let rss_mb = get_current_rss_mb(world);
        let gc_objects = runtime.gc_object_count();
        let schedule_systems = count_schedule_systems(world);

        if let Some(mut stats) = world.get_resource_mut::<HotReloadStats>() {
            stats.memory_mb = rss_mb;
        }

        if let Some(mut profile) = world.get_resource_mut::<MemoryProfile>() {
            profile.capture_snapshot(
                new_generation,
                rss_mb,
                gc_objects,
                schedule_systems,
                current_generation_systems,
            );

            if is_verbose() {
                let growth = profile.growth_mb(rss_mb);
                let warning = if profile.is_warning(rss_mb) {
                    " ⚠️ WARNING"
                } else {
                    ""
                };
                eprintln!(
                    "   → Memory: {:.1}MB (growth: {:.1}MB, peak: {:.1}MB, gc: {}, systems: {} total, {} this generation){}",
                    rss_mb,
                    growth,
                    profile.peak_rss_mb,
                    gc_objects,
                    schedule_systems,
                    current_generation_systems,
                    warning
                );
            }
        }
    }

    // Update Time<Real>'s last-seen instant so the next frame's delta doesn't
    // include time spent performing the reload. Must be at the very end so the
    // delta between here and the next time_system call is minimal.
    if mode == ReloadMode::Full
        && let Some(mut time_real) = world.get_resource_mut::<Time<Real>>()
    {
        time_real.update_with_instant(Instant::now());
    }

    if is_verbose() {
        eprintln!("✅ [Hot Reload] {:?} reload complete\n", mode);
    }

    emit_reload_progress(
        world,
        ReloadProgress::new(ReloadProgressPhase::Complete, new_generation, mode),
    );

    // Record the fingerprint only now that this generation is live. Doing it
    // earlier let a failed reload (register error, or a Startup that panicked
    // or errored and rolled back to the previous generation) leave the tracker
    // pointing at definitions that never took effect, so the next Partial
    // reload with matching Startup/resource/observer fingerprints skipped
    // escalation and silently kept the old generation's state.
    let mut tracker = world.get_resource_or_insert_with(EscalationTracker::default);
    tracker.last = Some(fingerprint);
    tracker.full_reload_required = false;

    Ok(())
}

/// Roll the generation back and mark `ReloadResult` as failed before
/// propagating a registration error, so the previous generation's gated
/// systems keep running as the failure response claims.
fn fail_registration<S: HotReloadStateAccess>(
    world: &mut World,
    hot_reload_state: &S,
    old_generation: u32,
    mode: ReloadMode,
    error: &ReloadError,
) {
    hot_reload_state.set_generation(old_generation);
    world.resource_mut::<HotReloadGeneration>().update();

    let mut result = world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
    result.failed = true;
    result.failure_reason = Some(error.message.clone());
    result.failure_traceback = error.traceback.clone();
    result.running_previous_generation = true;

    if mode == ReloadMode::Full {
        require_full_recovery(world);
    }
}

fn require_full_recovery(world: &mut World) {
    world
        .get_resource_or_insert_with(EscalationTracker::default)
        .full_reload_required = true;
}

/// Trait abstracting the generation counter manipulation on the shared hot reload state.
/// This decouples the orchestrator from the PyO3-specific `HotReloadState`.
pub trait HotReloadStateAccess {
    fn current_generation(&self) -> u32;
    fn increment_generation(&self);
    fn set_generation(&self, generation: u32);
}

#[cfg(test)]
mod tests {
    use std::{
        any::TypeId,
        collections::{HashSet, VecDeque},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
    };

    use bevy::{app::Startup, ecs::schedule::Schedules, prelude::*};

    use super::*;
    use crate::{
        profiling::{MemoryProfile, SystemProfiler},
        runtime::{DefsFingerprint, ReloadError, ReloadRuntime},
    };

    /// Minimal mock runtime that succeeds immediately with no systems.
    struct MockRuntime;

    #[derive(Default, Resource)]
    struct RequestPruneCalls(Vec<Option<u32>>);

    #[derive(Default, Resource)]
    struct ResourceRebindCalls(usize);

    #[derive(Default, Resource)]
    struct StartupScheduleRuns(Vec<&'static str>);

    fn record_pre_startup(mut runs: ResMut<StartupScheduleRuns>) {
        runs.0.push("pre");
    }

    fn record_startup(mut runs: ResMut<StartupScheduleRuns>) {
        runs.0.push("startup");
    }

    fn record_post_startup(mut runs: ResMut<StartupScheduleRuns>) {
        runs.0.push("post");
    }

    impl ReloadRuntime for MockRuntime {
        type Defs = ();
        type SystemHandle = ();
        fn load_definitions(&mut self, _gen: u32) -> Result<(), ReloadError> {
            Ok(())
        }
        fn defs_fingerprint(&self, _defs: &()) -> DefsFingerprint {
            DefsFingerprint::default()
        }
        fn plugin_names(&self, _defs: &()) -> Vec<PluginIdentity> {
            vec![]
        }
        fn system_names(&self, _defs: &()) -> HashSet<String> {
            HashSet::new()
        }
        fn register_systems(
            &mut self,
            _world: &mut World,
            _defs: (),
            _gen: u32,
        ) -> Result<Vec<()>, ReloadError> {
            Ok(vec![])
        }
        fn register_resources(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn rebind_resources(&mut self, world: &mut World, _defs: &()) -> Result<(), ReloadError> {
            world
                .get_resource_or_insert_with(ResourceRebindCalls::default)
                .0 += 1;
            Ok(())
        }
        fn register_messages(
            &mut self,
            _world: &mut World,
            _defs: &(),
            _gen: u32,
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_observers(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_handles(&mut self, _world: &mut World, _gen: u32, _handles: Vec<()>) {}
        fn prune_messages(&mut self, _world: &mut World, _gen: u32) {}
        fn prune_requests(&mut self, world: &mut World, outgoing_generation: Option<u32>) {
            world
                .get_resource_or_insert_with(RequestPruneCalls::default)
                .0
                .push(outgoing_generation);
        }
        fn clear_custom_resources(&mut self, _world: &mut World, _verbose: bool) {}
        fn snapshot_native_resources(&self, _world: &World) -> HashSet<TypeId> {
            HashSet::new()
        }
        fn clear_native_resources(
            &self,
            _world: &mut World,
            _initial: &HashSet<TypeId>,
            _verbose: bool,
        ) {
        }
        fn detect_system_delta(
            &mut self,
            _world: &mut World,
            _new: HashSet<String>,
        ) -> Vec<String> {
            vec![]
        }
        fn clear_param_cache(&mut self) {}
        fn trigger_gc(&mut self) {}
        fn print_error(&self, _error: &ReloadError) {}
    }

    /// Mock runtime that injects a Startup system which writes to
    /// `LastSystemError` - simulating a Python exception (not a panic)
    /// during Startup.
    struct StartupErrorRuntime {
        retired: Arc<AtomicBool>,
        inject_startup_error: bool,
        pending_errors: VecDeque<Option<String>>,
        take_pending_calls: usize,
        fingerprint: DefsFingerprint,
    }

    impl Default for StartupErrorRuntime {
        fn default() -> Self {
            Self {
                retired: Arc::default(),
                inject_startup_error: true,
                pending_errors: VecDeque::new(),
                take_pending_calls: 0,
                fingerprint: DefsFingerprint::default(),
            }
        }
    }

    /// Bevy system that simulates a Python exception by writing to
    /// `LastSystemError` (same as DynamicSystem::run_inner on exception).
    /// Uses `unwrap_or(0.0)` to match real WASM behavior where Time
    /// returns 0.0 on the first reload (before any time update).
    fn crashing_startup_system(world: &mut World) {
        let current_time = world
            .get_resource::<bevy::time::Time>()
            .map(|t| t.elapsed_secs_f64())
            .unwrap_or(0.0);
        let mut last_error =
            world.get_resource_or_insert_with(pybevy_core::LastSystemError::default);
        last_error.error = Some("NameError: name 'foo' is not defined".to_string());
        last_error.traceback = Some("File \"main.py\", line 5\n  NameError".to_string());
        last_error.timestamp_secs = current_time;
    }

    impl ReloadRuntime for StartupErrorRuntime {
        type Defs = ();
        type SystemHandle = Arc<AtomicBool>;

        fn load_definitions(&mut self, _gen: u32) -> Result<(), ReloadError> {
            Ok(())
        }
        fn defs_fingerprint(&self, _defs: &()) -> DefsFingerprint {
            self.fingerprint.clone()
        }
        fn plugin_names(&self, _defs: &()) -> Vec<PluginIdentity> {
            vec![]
        }
        fn system_names(&self, _defs: &()) -> HashSet<String> {
            HashSet::new()
        }
        fn register_systems(
            &mut self,
            world: &mut World,
            _defs: (),
            _gen: u32,
        ) -> Result<Vec<Arc<AtomicBool>>, ReloadError> {
            if self.inject_startup_error {
                let mut schedules = world.resource_mut::<Schedules>();
                if let Some(startup) = schedules.get_mut(Startup) {
                    startup.add_systems(crashing_startup_system);
                }
            }
            Ok(vec![self.retired.clone()])
        }
        fn register_resources(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_messages(
            &mut self,
            _world: &mut World,
            _defs: &(),
            _gen: u32,
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_observers(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_handles(
            &mut self,
            _world: &mut World,
            _gen: u32,
            _handles: Vec<Arc<AtomicBool>>,
        ) {
        }
        fn retire_handles(&mut self, handles: &[Arc<AtomicBool>]) {
            for handle in handles {
                handle.store(true, Ordering::SeqCst);
            }
        }
        fn prune_messages(&mut self, _world: &mut World, _gen: u32) {}
        fn prune_requests(&mut self, world: &mut World, outgoing_generation: Option<u32>) {
            world
                .get_resource_or_insert_with(RequestPruneCalls::default)
                .0
                .push(outgoing_generation);
        }
        fn clear_custom_resources(&mut self, _world: &mut World, _verbose: bool) {}
        fn snapshot_native_resources(&self, _world: &World) -> HashSet<TypeId> {
            HashSet::new()
        }
        fn clear_native_resources(
            &self,
            _world: &mut World,
            _initial: &HashSet<TypeId>,
            _verbose: bool,
        ) {
        }
        fn detect_system_delta(
            &mut self,
            _world: &mut World,
            _new: HashSet<String>,
        ) -> Vec<String> {
            vec![]
        }
        fn clear_param_cache(&mut self) {}
        fn trigger_gc(&mut self) {}
        fn take_pending_system_error(&mut self, _world: &mut World) -> Option<String> {
            self.take_pending_calls += 1;
            self.pending_errors.pop_front().flatten()
        }
        fn print_error(&self, _error: &ReloadError) {}
    }

    struct MockState {
        generation: Arc<std::sync::atomic::AtomicU32>,
    }

    impl MockState {
        fn new(counter: Arc<std::sync::atomic::AtomicU32>) -> Self {
            Self {
                generation: counter,
            }
        }
    }

    impl HotReloadStateAccess for MockState {
        fn current_generation(&self) -> u32 {
            self.generation.load(Ordering::SeqCst)
        }
        fn increment_generation(&self) {
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
        fn set_generation(&self, g: u32) {
            self.generation.store(g, Ordering::SeqCst);
        }
    }

    fn setup_world() -> (World, Arc<std::sync::atomic::AtomicU32>) {
        let mut world = World::new();
        let gen_counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        world.insert_resource(HotReloadGeneration::new(gen_counter.clone()));
        world.insert_resource(MemoryProfile::default());
        world.insert_resource(SystemProfiler::new(60));
        world.insert_resource(PluginTracker::default());

        let mut schedules = Schedules::default();
        schedules.insert(Schedule::new(Startup));
        world.insert_resource(schedules);

        (world, gen_counter)
    }

    fn record_progress(world: &mut World) -> Arc<Mutex<Vec<ReloadProgress>>> {
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        world.insert_resource(crate::ReloadProgressReporter::new(move |progress| {
            captured.lock().unwrap().push(progress);
        }));
        events
    }

    #[test]
    fn full_reload_runs_the_ordered_startup_schedule_sequence() {
        let (mut world, gen_counter) = setup_world();
        world.insert_resource(MainScheduleOrder::default());
        world.insert_resource(StartupScheduleRuns::default());
        {
            let mut schedules = world.resource_mut::<Schedules>();
            schedules.insert(Schedule::new(PreStartup));
            schedules.insert(Schedule::new(PostStartup));
        }
        world.schedule_scope(PreStartup, |_world, schedule| {
            schedule.add_systems(record_pre_startup);
        });
        world.schedule_scope(Startup, |_world, schedule| {
            schedule.add_systems(record_startup);
        });
        world.schedule_scope(PostStartup, |_world, schedule| {
            schedule.add_systems(record_post_startup);
        });
        let state = MockState::new(gen_counter);

        assert!(perform_reload(&mut world, &mut MockRuntime, ReloadMode::Full, &state).is_ok());

        assert_eq!(
            world.resource::<StartupScheduleRuns>().0,
            ["pre", "startup", "post"]
        );
    }

    #[test]
    fn full_reload_reports_backend_neutral_progress() {
        let (mut world, gen_counter) = setup_world();
        let events = record_progress(&mut world);
        let state = MockState::new(gen_counter);

        assert!(perform_reload(&mut world, &mut MockRuntime, ReloadMode::Full, &state).is_ok());

        let phases: Vec<_> = events
            .lock()
            .unwrap()
            .iter()
            .map(|progress| progress.phase)
            .collect();
        assert_eq!(
            phases,
            vec![
                ReloadProgressPhase::DefinitionsLoading,
                ReloadProgressPhase::DefinitionsReady,
                ReloadProgressPhase::CleanupStarted,
                ReloadProgressPhase::CleanupFinished,
                ReloadProgressPhase::Registering,
                ReloadProgressPhase::StartupStarted,
                ReloadProgressPhase::StartupFinished,
                ReloadProgressPhase::Complete,
            ]
        );
        assert!(!world.contains_resource::<ResourceRebindCalls>());
    }

    #[test]
    fn partial_reload_skips_cleanup_and_startup_progress() {
        let (mut world, gen_counter) = setup_world();
        let events = record_progress(&mut world);
        let state = MockState::new(gen_counter);

        assert!(perform_reload(&mut world, &mut MockRuntime, ReloadMode::Partial, &state).is_ok());

        let phases: Vec<_> = events
            .lock()
            .unwrap()
            .iter()
            .map(|progress| progress.phase)
            .collect();
        assert_eq!(
            phases,
            vec![
                ReloadProgressPhase::DefinitionsLoading,
                ReloadProgressPhase::DefinitionsReady,
                ReloadProgressPhase::Registering,
                ReloadProgressPhase::Complete,
            ]
        );
        assert_eq!(world.resource::<ResourceRebindCalls>().0, 1);
    }

    #[test]
    fn successful_reloads_prune_requests_with_the_committed_mode() {
        let (mut full_world, full_generation) = setup_world();
        let full_state = MockState::new(full_generation);
        assert!(
            perform_reload(
                &mut full_world,
                &mut MockRuntime,
                ReloadMode::Full,
                &full_state,
            )
            .is_ok()
        );
        assert_eq!(full_world.resource::<RequestPruneCalls>().0, vec![Some(0)]);

        let (mut partial_world, partial_generation) = setup_world();
        let partial_state = MockState::new(partial_generation);
        assert!(
            perform_reload(
                &mut partial_world,
                &mut MockRuntime,
                ReloadMode::Partial,
                &partial_state,
            )
            .is_ok()
        );
        assert_eq!(partial_world.resource::<RequestPruneCalls>().0, vec![None]);
    }

    /// Runtime with a configurable fingerprint, for escalation tests.
    struct FingerprintRuntime(DefsFingerprint);

    impl ReloadRuntime for FingerprintRuntime {
        type Defs = ();
        type SystemHandle = ();
        fn load_definitions(&mut self, _gen: u32) -> Result<(), ReloadError> {
            Ok(())
        }
        fn defs_fingerprint(&self, _defs: &()) -> DefsFingerprint {
            self.0.clone()
        }
        fn plugin_names(&self, _defs: &()) -> Vec<PluginIdentity> {
            vec![]
        }
        fn system_names(&self, _defs: &()) -> HashSet<String> {
            HashSet::new()
        }
        fn register_systems(
            &mut self,
            _world: &mut World,
            _defs: (),
            _gen: u32,
        ) -> Result<Vec<()>, ReloadError> {
            Ok(vec![])
        }
        fn register_resources(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_messages(
            &mut self,
            _world: &mut World,
            _defs: &(),
            _gen: u32,
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_observers(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_handles(&mut self, _world: &mut World, _gen: u32, _handles: Vec<()>) {}
        fn prune_messages(&mut self, _world: &mut World, _keep: u32) {}
        fn clear_custom_resources(&mut self, _world: &mut World, _verbose: bool) {}
        fn snapshot_native_resources(&self, _world: &World) -> HashSet<TypeId> {
            HashSet::new()
        }
        fn clear_native_resources(
            &self,
            _world: &mut World,
            _initial: &HashSet<TypeId>,
            _verbose: bool,
        ) {
        }
        fn detect_system_delta(
            &mut self,
            _world: &mut World,
            _new: HashSet<String>,
        ) -> Vec<String> {
            vec![]
        }
        fn clear_param_cache(&mut self) {}
        fn trigger_gc(&mut self) {}
        fn print_error(&self, _error: &ReloadError) {}
    }

    /// An unchanged fingerprint must stay on the fast Partial path.
    /// Previously any scene with a Startup system escalated on every save.
    #[test]
    fn partial_reload_unchanged_fingerprint_stays_partial() {
        let (mut world, gen_counter) = setup_world();
        let state = MockState::new(gen_counter);
        let mut runtime = FingerprintRuntime(DefsFingerprint {
            startup_code: 42,
            has_startup: true,
            ..Default::default()
        });

        // First Partial has no fingerprint baseline: escalates conservatively.
        assert!(perform_reload(&mut world, &mut runtime, ReloadMode::Partial, &state).is_ok());
        assert!(
            world.resource::<pybevy_core::ReloadResult>().escalated,
            "first reload without a baseline should escalate"
        );

        // Second Partial with an identical fingerprint stays Partial.
        assert!(perform_reload(&mut world, &mut runtime, ReloadMode::Partial, &state).is_ok());
        let result = world.resource::<pybevy_core::ReloadResult>();
        assert!(
            !result.escalated,
            "unchanged definitions must stay on the Partial path"
        );
        assert_eq!(
            result.actual_mode,
            Some(pybevy_core::ReloadRequestMode::Partial)
        );
    }

    /// A failed reload must not poison the escalation tracker. Its fingerprint
    /// never became live, so a later Partial reload with matching definitions
    /// must still escalate to Full rather than believing the failed generation
    /// is already running.
    #[test]
    fn failed_reload_does_not_record_its_fingerprint() {
        let (mut world, gen_counter) = setup_world();
        let state = MockState::new(gen_counter);
        let fingerprint = DefsFingerprint {
            startup_code: 99,
            has_startup: true,
            ..Default::default()
        };

        // A reload whose Startup errors and rolls back to the old generation.
        let mut failing = StartupErrorRuntime {
            fingerprint: fingerprint.clone(),
            ..Default::default()
        };
        let failed = perform_reload(&mut world, &mut failing, ReloadMode::Full, &state);
        assert!(failed.is_err(), "the Startup error should fail the reload");

        // The tracker must not carry the failed generation's fingerprint.
        assert!(
            world
                .get_resource::<EscalationTracker>()
                .and_then(|t| t.last.clone())
                .is_none(),
            "a failed reload must not record its fingerprint"
        );

        // A subsequent Partial reload with the SAME fingerprint must escalate,
        // because that generation has never actually run.
        let mut runtime = FingerprintRuntime(fingerprint);
        assert!(perform_reload(&mut world, &mut runtime, ReloadMode::Partial, &state).is_ok());
        assert!(
            world.resource::<pybevy_core::ReloadResult>().escalated,
            "a Partial reload matching a previously-failed generation must escalate"
        );
    }

    /// A Full reload can clear the live scene before its Startup system fails.
    /// Restoring the exact previous source still has to run Startup again even
    /// though its fingerprint matches the last successful generation.
    #[test]
    fn failed_full_reload_forces_exact_baseline_recovery_to_full() {
        let (mut world, gen_counter) = setup_world();
        let state = MockState::new(gen_counter);
        let good_fingerprint = DefsFingerprint {
            startup_code: 41,
            has_startup: true,
            ..Default::default()
        };

        let mut good = FingerprintRuntime(good_fingerprint.clone());
        assert!(perform_reload(&mut world, &mut good, ReloadMode::Full, &state).is_ok());

        let mut failing = StartupErrorRuntime {
            fingerprint: DefsFingerprint {
                startup_code: 42,
                has_startup: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(perform_reload(&mut world, &mut failing, ReloadMode::Full, &state).is_err());

        let tracker = world.resource::<EscalationTracker>();
        assert_eq!(tracker.last, Some(good_fingerprint.clone()));
        assert!(tracker.full_reload_required);

        let mut restored = FingerprintRuntime(good_fingerprint);
        assert!(perform_reload(&mut world, &mut restored, ReloadMode::Partial, &state).is_ok());

        let result = world.resource::<pybevy_core::ReloadResult>();
        assert!(result.escalated);
        assert_eq!(
            result.escalation_reason.as_deref(),
            Some("recovering from failed Full reload")
        );
        assert!(!world.resource::<EscalationTracker>().full_reload_required);
    }

    #[test]
    fn partial_reload_changed_startup_escalates() {
        let (mut world, gen_counter) = setup_world();
        let state = MockState::new(gen_counter);
        let mut runtime = FingerprintRuntime(DefsFingerprint {
            startup_code: 42,
            has_startup: true,
            ..Default::default()
        });

        // Establish the baseline (Full reloads record it too).
        assert!(perform_reload(&mut world, &mut runtime, ReloadMode::Full, &state).is_ok());

        runtime.0.startup_code = 43;
        assert!(perform_reload(&mut world, &mut runtime, ReloadMode::Partial, &state).is_ok());
        let result = world.resource::<pybevy_core::ReloadResult>();
        assert!(result.escalated, "changed Startup code must escalate");
        assert_eq!(
            result.escalation_reason.as_deref(),
            Some("Startup systems changed")
        );
    }

    #[test]
    fn partial_reload_changed_component_layout_escalates() {
        let (mut world, gen_counter) = setup_world();
        let state = MockState::new(gen_counter);
        let mut runtime = FingerprintRuntime(DefsFingerprint::default());

        assert!(perform_reload(&mut world, &mut runtime, ReloadMode::Full, &state).is_ok());

        runtime.0.component_layout_changed = true;
        assert!(perform_reload(&mut world, &mut runtime, ReloadMode::Partial, &state).is_ok());
        let result = world.resource::<pybevy_core::ReloadResult>();
        assert!(result.escalated);
        assert_eq!(
            result.escalation_reason.as_deref(),
            Some("custom component layout changed")
        );
        assert_eq!(
            result.actual_mode,
            Some(pybevy_core::ReloadRequestMode::Full)
        );
    }

    /// A Python exception (not a panic) in a Startup system must trigger
    /// generation rollback so that Update systems from the broken
    /// generation don't keep running.
    #[test]
    fn startup_exception_rolls_back_generation() {
        let (mut world, gen_counter) = setup_world();
        let state = MockState::new(gen_counter);
        assert_eq!(state.current_generation(), 0);
        let retired = Arc::new(AtomicBool::new(false));
        let mut runtime = StartupErrorRuntime {
            retired: retired.clone(),
            ..Default::default()
        };

        let result = perform_reload(&mut world, &mut runtime, ReloadMode::Full, &state);

        assert!(result.is_err(), "reload should fail when Startup has error");
        let err = result.unwrap_err();
        assert!(
            err.message.contains("NameError"),
            "error should contain the Python exception, got: {}",
            err.message
        );

        assert_eq!(
            state.current_generation(),
            0,
            "generation should be rolled back to 0 after Startup error"
        );
        assert_eq!(
            world.resource::<HotReloadGeneration>().current,
            0,
            "HotReloadGeneration.current should be rolled back"
        );

        let reload_result = world.resource::<pybevy_core::ReloadResult>();
        assert!(reload_result.failed, "ReloadResult.failed should be true");
        assert!(
            reload_result.running_previous_generation,
            "should be running previous generation"
        );
        assert!(
            retired.load(Ordering::SeqCst),
            "systems from the rejected generation must be retired"
        );
        assert!(
            !world.contains_resource::<RequestPruneCalls>(),
            "a rejected generation must not prune requests owned by the running generation"
        );
    }

    #[test]
    fn outgoing_pending_error_does_not_reject_incoming_scene() {
        let (mut world, gen_counter) = setup_world();
        let state = MockState::new(gen_counter);
        let retired = Arc::new(AtomicBool::new(false));
        let mut runtime = StartupErrorRuntime {
            retired: retired.clone(),
            inject_startup_error: false,
            pending_errors: VecDeque::from([
                Some("ValueError: outgoing scene failed".to_string()),
                None,
            ]),
            take_pending_calls: 0,
            ..Default::default()
        };

        let result = perform_reload(&mut world, &mut runtime, ReloadMode::Full, &state);

        assert!(
            result.is_ok(),
            "outgoing scene errors must not fail the replacement"
        );
        assert_eq!(runtime.take_pending_calls, 2);
        assert_eq!(state.current_generation(), 1);
        assert!(
            !retired.load(Ordering::SeqCst),
            "systems from a successful generation must stay active"
        );
    }

    /// Entities spawned during a failed Startup must be cleaned up so we
    /// don't leave orphaned cameras/render targets that cause GPU errors.
    #[test]
    fn startup_exception_cleans_up_spawned_entities() {
        /// System that spawns entities AND then errors -- simulates a Startup
        /// that partially creates a scene before hitting a Python exception.
        fn spawning_crash_system(world: &mut World) {
            world.spawn(Name::new("orphan_camera"));
            world.spawn(Name::new("orphan_light"));

            let current_time = world
                .get_resource::<bevy::time::Time>()
                .map(|t| t.elapsed_secs_f64())
                .unwrap_or(0.0);
            let mut last_error =
                world.get_resource_or_insert_with(pybevy_core::LastSystemError::default);
            last_error.error = Some("RuntimeError: setup failed".to_string());
            last_error.traceback = Some("File \"scene.py\", line 10".to_string());
            last_error.timestamp_secs = current_time;
        }

        struct SpawningErrorRuntime;

        impl ReloadRuntime for SpawningErrorRuntime {
            type Defs = ();
            type SystemHandle = ();
            fn load_definitions(&mut self, _gen: u32) -> Result<(), ReloadError> {
                Ok(())
            }
            fn defs_fingerprint(&self, _defs: &()) -> DefsFingerprint {
                DefsFingerprint::default()
            }
            fn plugin_names(&self, _defs: &()) -> Vec<PluginIdentity> {
                vec![]
            }
            fn system_names(&self, _defs: &()) -> HashSet<String> {
                HashSet::new()
            }
            fn register_systems(
                &mut self,
                world: &mut World,
                _defs: (),
                _gen: u32,
            ) -> Result<Vec<()>, ReloadError> {
                let mut schedules = world.resource_mut::<Schedules>();
                if let Some(startup) = schedules.get_mut(Startup) {
                    startup.add_systems(spawning_crash_system);
                }
                Ok(vec![])
            }
            fn register_resources(
                &mut self,
                _world: &mut World,
                _defs: &(),
            ) -> Result<(), ReloadError> {
                Ok(())
            }
            fn register_messages(
                &mut self,
                _world: &mut World,
                _defs: &(),
                _gen: u32,
            ) -> Result<(), ReloadError> {
                Ok(())
            }
            fn register_observers(
                &mut self,
                _world: &mut World,
                _defs: &(),
            ) -> Result<(), ReloadError> {
                Ok(())
            }
            fn register_handles(&mut self, _world: &mut World, _gen: u32, _handles: Vec<()>) {}
            fn prune_messages(&mut self, _world: &mut World, _gen: u32) {}
            fn clear_custom_resources(&mut self, _world: &mut World, _verbose: bool) {}
            fn snapshot_native_resources(&self, _world: &World) -> HashSet<TypeId> {
                HashSet::new()
            }
            fn clear_native_resources(
                &self,
                _world: &mut World,
                _initial: &HashSet<TypeId>,
                _verbose: bool,
            ) {
            }
            fn detect_system_delta(
                &mut self,
                _world: &mut World,
                _new: HashSet<String>,
            ) -> Vec<String> {
                vec![]
            }
            fn clear_param_cache(&mut self) {}
            fn trigger_gc(&mut self) {}
            fn print_error(&self, _error: &ReloadError) {}
        }

        let (mut world, gen_counter) = setup_world();
        let state = MockState::new(gen_counter);

        // Resources are stored as entities; exclude them so the count
        // reflects only scene entities, matching the cleanup logic under test.
        let pre_entity_count = world
            .query_filtered::<Entity, Without<IsResource>>()
            .iter(&world)
            .count();

        let result = perform_reload(
            &mut world,
            &mut SpawningErrorRuntime,
            ReloadMode::Full,
            &state,
        );

        assert!(result.is_err(), "reload should fail");

        let post_entity_count = world
            .query_filtered::<Entity, Without<IsResource>>()
            .iter(&world)
            .count();
        assert_eq!(
            post_entity_count, pre_entity_count,
            "entities spawned during failed Startup should be cleaned up \
             (had {} before, {} after)",
            pre_entity_count, post_entity_count
        );

        // Verify the orphan entities are actually gone
        let names: Vec<String> = world
            .query::<&Name>()
            .iter(&world)
            .map(|n| n.as_str().to_string())
            .collect();
        assert!(
            !names.contains(&"orphan_camera".to_string()),
            "orphan_camera should have been despawned"
        );
    }

    /// After a failed reload, the next successful reload must run Startup.
    #[test]
    fn reload_after_failure_runs_startup() {
        let (mut world, gen_counter) = setup_world();
        let state = MockState::new(gen_counter);

        // successful reload (gen 0 -> 1)
        let result = perform_reload(&mut world, &mut MockRuntime, ReloadMode::Full, &state);
        assert!(result.is_ok(), "first reload should succeed");
        assert_eq!(state.current_generation(), 1);

        // failing reload (gen 1 -> 2, rolled back to 1)
        let result = perform_reload(
            &mut world,
            &mut StartupErrorRuntime::default(),
            ReloadMode::Full,
            &state,
        );
        assert!(result.is_err(), "second reload should fail");
        assert_eq!(state.current_generation(), 1);

        // successful reload (gen 1 -> 2 again) - must run Startup
        let result = perform_reload(&mut world, &mut MockRuntime, ReloadMode::Full, &state);
        assert!(result.is_ok(), "third reload should succeed");
        assert_eq!(state.current_generation(), 2);

        let gen_res = world.resource::<HotReloadGeneration>();
        assert!(
            gen_res.has_startup_run(2),
            "Startup should have run for generation 2"
        );
    }

    /// Runtime that returns Err from register_systems.
    struct RegisterSystemsErrorRuntime;

    impl ReloadRuntime for RegisterSystemsErrorRuntime {
        type Defs = ();
        type SystemHandle = ();
        fn load_definitions(&mut self, _gen: u32) -> Result<(), ReloadError> {
            Ok(())
        }
        fn defs_fingerprint(&self, _defs: &()) -> DefsFingerprint {
            DefsFingerprint::default()
        }
        fn plugin_names(&self, _defs: &()) -> Vec<PluginIdentity> {
            vec![]
        }
        fn system_names(&self, _defs: &()) -> HashSet<String> {
            HashSet::new()
        }
        fn register_systems(
            &mut self,
            _world: &mut World,
            _defs: (),
            _gen: u32,
        ) -> Result<Vec<()>, ReloadError> {
            Err(ReloadError {
                message: "synthetic register_systems failure".into(),
                traceback: Some("File \"scene.py\", line 12, in broken".into()),
                is_load_failure: false,
            })
        }
        fn register_resources(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_messages(
            &mut self,
            _world: &mut World,
            _defs: &(),
            _gen: u32,
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_observers(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_handles(&mut self, _world: &mut World, _gen: u32, _handles: Vec<()>) {}
        fn prune_messages(&mut self, _world: &mut World, _gen: u32) {}
        fn clear_custom_resources(&mut self, _world: &mut World, _verbose: bool) {}
        fn snapshot_native_resources(&self, _world: &World) -> HashSet<TypeId> {
            HashSet::new()
        }
        fn clear_native_resources(
            &self,
            _world: &mut World,
            _initial: &HashSet<TypeId>,
            _verbose: bool,
        ) {
        }
        fn detect_system_delta(
            &mut self,
            _world: &mut World,
            _new: HashSet<String>,
        ) -> Vec<String> {
            vec![]
        }
        fn clear_param_cache(&mut self) {}
        fn trigger_gc(&mut self) {}
        fn print_error(&self, _error: &ReloadError) {}
    }

    /// Runtime that returns Err from register_resources.
    struct RegisterResourcesErrorRuntime;

    impl ReloadRuntime for RegisterResourcesErrorRuntime {
        type Defs = ();
        type SystemHandle = ();
        fn load_definitions(&mut self, _gen: u32) -> Result<(), ReloadError> {
            Ok(())
        }
        fn defs_fingerprint(&self, _defs: &()) -> DefsFingerprint {
            DefsFingerprint::default()
        }
        fn plugin_names(&self, _defs: &()) -> Vec<PluginIdentity> {
            vec![]
        }
        fn system_names(&self, _defs: &()) -> HashSet<String> {
            HashSet::new()
        }
        fn register_systems(
            &mut self,
            _world: &mut World,
            _defs: (),
            _gen: u32,
        ) -> Result<Vec<()>, ReloadError> {
            Ok(vec![])
        }
        fn register_resources(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Err(ReloadError {
                message: "synthetic register_resources failure".into(),
                traceback: None,
                is_load_failure: false,
            })
        }
        fn register_messages(
            &mut self,
            _world: &mut World,
            _defs: &(),
            _gen: u32,
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_observers(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_handles(&mut self, _world: &mut World, _gen: u32, _handles: Vec<()>) {}
        fn prune_messages(&mut self, _world: &mut World, _gen: u32) {}
        fn clear_custom_resources(&mut self, _world: &mut World, _verbose: bool) {}
        fn snapshot_native_resources(&self, _world: &World) -> HashSet<TypeId> {
            HashSet::new()
        }
        fn clear_native_resources(
            &self,
            _world: &mut World,
            _initial: &HashSet<TypeId>,
            _verbose: bool,
        ) {
        }
        fn detect_system_delta(
            &mut self,
            _world: &mut World,
            _new: HashSet<String>,
        ) -> Vec<String> {
            vec![]
        }
        fn clear_param_cache(&mut self) {}
        fn trigger_gc(&mut self) {}
        fn print_error(&self, _error: &ReloadError) {}
    }

    /// register_systems Err must flag ReloadResult.failed and failure_reason.
    #[test]
    fn register_systems_error_flags_reload_result() {
        let (mut world, gen_counter) = setup_world();
        let state = MockState::new(gen_counter);

        let result = perform_reload(
            &mut world,
            &mut RegisterSystemsErrorRuntime,
            ReloadMode::Full,
            &state,
        );

        assert!(
            result.is_err(),
            "reload should fail when register_systems errors"
        );
        let err = result.unwrap_err();
        assert_eq!(err.message, "synthetic register_systems failure");

        let reload_result = world.resource::<pybevy_core::ReloadResult>();
        assert!(reload_result.failed, "ReloadResult.failed should be true");
        assert_eq!(
            reload_result.failure_reason.as_deref(),
            Some("synthetic register_systems failure"),
        );
        assert_eq!(
            reload_result.failure_traceback.as_deref(),
            Some("File \"scene.py\", line 12, in broken"),
        );
        assert!(
            reload_result.running_previous_generation,
            "running_previous_generation should be true",
        );
        assert_eq!(
            state.current_generation(),
            0,
            "generation must roll back so the previous generation's gated systems keep running",
        );
        assert_eq!(world.resource::<HotReloadGeneration>().current, 0);
    }

    /// register_resources Err must flag ReloadResult.failed and failure_reason.
    #[test]
    fn register_resources_error_flags_reload_result() {
        let (mut world, gen_counter) = setup_world();
        let state = MockState::new(gen_counter);

        let result = perform_reload(
            &mut world,
            &mut RegisterResourcesErrorRuntime,
            ReloadMode::Full,
            &state,
        );

        assert!(
            result.is_err(),
            "reload should fail when register_resources errors"
        );
        let err = result.unwrap_err();
        assert_eq!(err.message, "synthetic register_resources failure");

        let reload_result = world.resource::<pybevy_core::ReloadResult>();
        assert!(reload_result.failed, "ReloadResult.failed should be true");
        assert_eq!(
            reload_result.failure_reason.as_deref(),
            Some("synthetic register_resources failure"),
        );
        assert_eq!(
            state.current_generation(),
            0,
            "generation must roll back on register_resources failure",
        );
        assert_eq!(world.resource::<HotReloadGeneration>().current, 0);
    }

    /// Candidate repro for empty-world-after-many-reloads: each Full reload
    /// should leave the world with the entities its Startup spawned. After many
    /// cycles, the count must stay constant — never drop to zero.
    ///
    /// Probes for state leaks in `startup_run_for_generations` (set.retain) and
    /// `BaseEntitySet` interactions across long edit sessions.
    struct SpawningStartupRuntime {
        spawn_count: usize,
    }

    impl ReloadRuntime for SpawningStartupRuntime {
        type Defs = ();
        type SystemHandle = ();
        fn load_definitions(&mut self, _gen: u32) -> Result<(), ReloadError> {
            Ok(())
        }
        fn defs_fingerprint(&self, _defs: &()) -> DefsFingerprint {
            DefsFingerprint::default()
        }
        fn plugin_names(&self, _defs: &()) -> Vec<PluginIdentity> {
            vec![]
        }
        fn system_names(&self, _defs: &()) -> HashSet<String> {
            HashSet::new()
        }
        fn register_systems(
            &mut self,
            world: &mut World,
            _defs: (),
            generation: u32,
        ) -> Result<Vec<()>, ReloadError> {
            use bevy::ecs::schedule::IntoScheduleConfigs;

            use crate::state::startup_or_reload;
            let count = self.spawn_count;
            let mut schedules = world.resource_mut::<Schedules>();
            if let Some(startup) = schedules.get_mut(Startup) {
                // Mirror production: gate by startup_or_reload so stale systems
                // from older generations don't double-spawn.
                let spawn_system = move |w: &mut World| {
                    for _ in 0..count {
                        w.spawn(bevy::prelude::Name::new("setup_entity"));
                    }
                };
                startup.add_systems(spawn_system.run_if(startup_or_reload(generation)));
            }
            Ok(vec![])
        }
        fn register_resources(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_messages(
            &mut self,
            _world: &mut World,
            _defs: &(),
            _gen: u32,
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_observers(
            &mut self,
            _world: &mut World,
            _defs: &(),
        ) -> Result<(), ReloadError> {
            Ok(())
        }
        fn register_handles(&mut self, _world: &mut World, _gen: u32, _handles: Vec<()>) {}
        fn prune_messages(&mut self, _world: &mut World, _gen: u32) {}
        fn clear_custom_resources(&mut self, _world: &mut World, _verbose: bool) {}
        fn snapshot_native_resources(&self, _world: &World) -> HashSet<TypeId> {
            HashSet::new()
        }
        fn clear_native_resources(
            &self,
            _world: &mut World,
            _initial: &HashSet<TypeId>,
            _verbose: bool,
        ) {
        }
        fn detect_system_delta(
            &mut self,
            _world: &mut World,
            _new: HashSet<String>,
        ) -> Vec<String> {
            vec![]
        }
        fn clear_param_cache(&mut self) {}
        fn trigger_gc(&mut self) {}
        fn print_error(&self, _error: &ReloadError) {}
    }

    /// Many sequential Full reloads must each leave Startup-spawned entities
    /// in place — empty world after a successful reload would mean Startup
    /// silently failed to run.
    #[test]
    fn many_full_reloads_never_leave_empty_world() {
        let (mut world, gen_counter) = setup_world();
        // Pretend a single base entity exists (camera/window/etc).
        let base = world.spawn(bevy::prelude::Name::new("base")).id();
        world.insert_resource(crate::BaseEntitySet {
            entities: [base].into_iter().collect(),
        });
        let state = MockState::new(gen_counter);

        const SPAWN_PER_RELOAD: usize = 3;
        const RELOAD_CYCLES: usize = 40;

        for cycle in 0..RELOAD_CYCLES {
            let mut runtime = SpawningStartupRuntime {
                spawn_count: SPAWN_PER_RELOAD,
            };
            let result = perform_reload(&mut world, &mut runtime, ReloadMode::Full, &state);
            assert!(result.is_ok(), "reload {} should succeed", cycle);

            // Count named entities only: resources are entity-backed, so a raw
            // Entity query also sees reload-machinery internals.
            let live = world
                .query_filtered::<Entity, With<Name>>()
                .iter(&world)
                .count();
            // base + spawned-this-reload (previous reload entities are despawned by clear_world_state)
            assert_eq!(
                live,
                1 + SPAWN_PER_RELOAD,
                "cycle {}: world unexpectedly has {} entities (expected base + {} spawned)",
                cycle,
                live,
                SPAWN_PER_RELOAD,
            );
            assert_ne!(live, 1, "cycle {}: empty world (only base)!", cycle);
        }
    }

    /// Alternating success/failure cycles must not corrupt the
    /// startup_run_for_generations set so badly that a later success leaves
    /// the world empty (Startup gated off by stale has_startup_run entry).
    #[test]
    fn alternating_failure_success_never_skips_startup() {
        let (mut world, gen_counter) = setup_world();
        let base = world.spawn(bevy::prelude::Name::new("base")).id();
        world.insert_resource(crate::BaseEntitySet {
            entities: [base].into_iter().collect(),
        });
        let state = MockState::new(gen_counter);

        for cycle in 0..10 {
            // Fail
            let _ = perform_reload(
                &mut world,
                &mut StartupErrorRuntime::default(),
                ReloadMode::Full,
                &state,
            );
            // Succeed — must spawn entities
            let mut runtime = SpawningStartupRuntime { spawn_count: 2 };
            let result = perform_reload(&mut world, &mut runtime, ReloadMode::Full, &state);
            assert!(result.is_ok(), "cycle {} success reload should ok", cycle);
            // Named entities only; see many_full_reloads_never_leave_empty_world
            let live = world
                .query_filtered::<Entity, With<Name>>()
                .iter(&world)
                .count();
            assert_eq!(
                live, 3,
                "cycle {}: expected base + 2 spawn (got {})",
                cycle, live
            );
        }
    }
}
