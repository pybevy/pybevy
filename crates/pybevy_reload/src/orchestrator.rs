use std::{collections::HashSet, time::Instant};

use bevy::{
    app::Startup,
    ecs::{entity::Entity, schedule::Schedules, world::World},
    time::{Real, Time},
};

use crate::{
    BaseEntitySet,
    cleanup::{NativeResourceSnapshot, clear_world_state},
    profiling::{HotReloadStats, MemoryProfile, SystemProfiler},
    runtime::{ReloadError, ReloadRuntime},
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

    // PHASE 1: VALIDATE — load definitions before committing.
    let next_generation = old_generation + 1;
    let defs = match runtime.load_definitions(next_generation) {
        Ok(defs) => defs,
        Err(e) => {
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
            result.running_previous_generation = true;

            return Err(e);
        }
    };

    // PHASE 2: COMMIT — increment generation (point of no return).
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
        clear_world_state(world, runtime, is_verbose());
    }

    // Reset reload result tracking
    {
        let mut result = world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
        result.escalated = false;
        result.escalation_reason = None;
        result.failed = false;
        result.failure_reason = None;
        result.running_previous_generation = false;
        result.plugins_added = None;
        result.plugins_removed = None;
        result.systems_removed = None;
        result.actual_mode = Some(match mode {
            ReloadMode::Full => pybevy_core::ReloadRequestMode::Full,
            ReloadMode::Partial => pybevy_core::ReloadRequestMode::Partial,
        });
    }

    // Auto-escalation: partial -> full if startup systems or resources changed
    let mut mode = mode;
    if mode == ReloadMode::Partial
        && let Some(reason) = runtime.requires_escalation(&defs)
    {
        eprintln!("⬆️ [Hot Reload] Escalating Partial → Full: {reason}");
        mode = ReloadMode::Full;
        clear_world_state(world, runtime, is_verbose());

        if let Some(mut stats) = world.get_resource_mut::<HotReloadStats>() {
            stats.last_mode = Some(ReloadMode::Full);
        }
        if let Some(mut result) = world.get_resource_mut::<pybevy_core::ReloadResult>() {
            result.escalated = true;
            result.escalation_reason = Some(reason.to_string());
            result.actual_mode = Some(pybevy_core::ReloadRequestMode::Full);
        }
    }

    // Plugin delta detection
    {
        let plugin_names = runtime.plugin_names(&defs);
        if !plugin_names.is_empty() {
            let new_plugin_set: std::collections::HashSet<String> =
                plugin_names.into_iter().collect();

            let (added, removed) = {
                if let Some(mut tracker) = world.get_resource_mut::<PluginTracker>() {
                    if tracker.known_plugins.is_empty() {
                        tracker.known_plugins = new_plugin_set;
                        (Vec::new(), Vec::new())
                    } else {
                        let added: Vec<_> = new_plugin_set
                            .difference(&tracker.known_plugins)
                            .cloned()
                            .collect();
                        let removed: Vec<_> = tracker
                            .known_plugins
                            .difference(&new_plugin_set)
                            .cloned()
                            .collect();
                        tracker.known_plugins = new_plugin_set;
                        (added, removed)
                    }
                } else {
                    (Vec::new(), Vec::new())
                }
            };

            if !added.is_empty() || !removed.is_empty() {
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
    }

    if mode == ReloadMode::Full {
        if let Err(e) = runtime.register_resources(world, &defs) {
            flag_reload_failure(world, e.message.clone());
            return Err(e);
        }
    }

    if let Err(e) = runtime.register_messages(world, &defs, new_generation) {
        flag_reload_failure(world, e.message.clone());
        return Err(e);
    }

    if mode == ReloadMode::Full {
        if let Err(e) = runtime.register_observers(world, &defs) {
            flag_reload_failure(world, e.message.clone());
            return Err(e);
        }
    }

    // System delta detection
    {
        let new_system_names = runtime.system_names(&defs);
        let removed = runtime.detect_system_delta(world, new_system_names);
        if !removed.is_empty() {
            eprintln!(
                "⚠️ [Hot Reload] Systems removed/renamed (stale schedule entries remain, use run_scene to clear): {:?}",
                removed
            );
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
            flag_reload_failure(world, e.message.clone());
            return Err(e);
        }
    };

    // Run Startup with rollback on panic
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
        world.query::<Entity>().iter(world).collect()
    } else {
        std::collections::HashSet::new()
    };

    if mode == ReloadMode::Full {
        if world.resource::<Schedules>().contains(Startup) {
            if is_verbose() {
                eprintln!("   → Running Startup schedule");
            }

            let startup_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                world.run_schedule(Startup);
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
                    let post_entities: Vec<Entity> = world.query::<Entity>().iter(world).collect();
                    let mut cleaned = 0;
                    for entity in post_entities {
                        if !pre_startup_entities.contains(&entity)
                            && world.get_entity(entity).is_ok()
                        {
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
                    if let Ok(mut set) = gen_res.startup_run_for_generations.lock() {
                        set.remove(&new_generation);
                    }
                }

                let error_msg = format!("Startup panicked: {}", panic_msg);
                let mut result =
                    world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
                result.failed = true;
                result.failure_reason = Some(error_msg.clone());
                result.running_previous_generation = true;

                runtime.clear_param_cache();

                return Err(ReloadError {
                    message: error_msg,
                    is_load_failure: false,
                });
            }
        } else if is_verbose() {
            eprintln!("   → Skipping Startup schedule (not present in app)");
        }
    } else if is_verbose() {
        eprintln!("   → Skipping Startup schedule (Partial mode)");
    }

    let startup_had_error = world
        .get_resource::<pybevy_core::LastSystemError>()
        .is_some_and(|e| {
            e.error.is_some() && (e.timestamp_secs > pre_startup_error_ts || !pre_startup_had_error)
        });

    // If a Startup system raised a Python exception (not a panic), apply
    // the same generation rollback so Update systems from the broken
    // generation don't keep running.  Without this, the new-generation
    // Update systems execute every frame even though their Startup failed
    // to set up the entities/resources they depend on.
    if startup_had_error && mode == ReloadMode::Full {
        let error_msg = world
            .get_resource::<pybevy_core::LastSystemError>()
            .and_then(|e| e.error.clone())
            .unwrap_or_else(|| "Startup system error".to_string());

        eprintln!(
            "⚠️ [Hot Reload] Startup system error - rolling back to generation {}",
            old_generation
        );

        // Clean up entities created during the failed Startup (same as
        // the panic path) so we don't leave orphaned render targets,
        // cameras, or other partially-created scene objects.
        {
            let post_entities: Vec<Entity> = world.query::<Entity>().iter(world).collect();
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
            if let Ok(mut set) = gen_res.startup_run_for_generations.lock() {
                set.remove(&new_generation);
            }
        }

        let mut result = world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
        result.failed = true;
        result.failure_reason = Some(error_msg.clone());
        result.running_previous_generation = true;

        runtime.clear_param_cache();

        return Err(ReloadError {
            message: error_msg,
            is_load_failure: false,
        });
    }

    if !startup_had_error
        && let Some(mut last_error) = world.get_resource_mut::<pybevy_core::LastSystemError>()
    {
        last_error.error = None;
        last_error.traceback = None;
    }

    {
        let gen_res = world.resource::<HotReloadGeneration>();
        if let Ok(mut set) = gen_res.startup_run_for_generations.lock() {
            set.retain(|&g| g >= new_generation.saturating_sub(2));
        }
    }

    // Register handles and gut old-generation systems
    if !system_handles.is_empty() {
        runtime.register_handles(world, new_generation, system_handles);
    }

    runtime.prune_messages(world, new_generation);

    // Post-reload cleanup
    runtime.clear_param_cache();
    runtime.trigger_gc();

    {
        let rss_mb = get_current_rss_mb(world);
        let gc_objects = runtime.gc_object_count();
        let schedule_systems = count_schedule_systems(world);

        if let Some(mut profile) = world.get_resource_mut::<MemoryProfile>() {
            profile.capture_snapshot(new_generation, rss_mb, gc_objects, schedule_systems);

            if is_verbose() {
                let growth = profile.growth_mb(rss_mb);
                let warning = if profile.is_warning(rss_mb) {
                    " ⚠️ WARNING"
                } else {
                    ""
                };
                eprintln!(
                    "   → Memory: {:.1}MB (growth: {:.1}MB, peak: {:.1}MB, gc: {}, systems: {}){}",
                    rss_mb, growth, profile.peak_rss_mb, gc_objects, schedule_systems, warning
                );
            }
        }
    }

    // Update Time<Real>'s last-seen instant so the next frame's delta doesn't
    // include time spent performing the reload. Must be at the very end so the
    // delta between here and the next time_system call is minimal.
    if mode == ReloadMode::Full {
        if let Some(mut time_real) = world.get_resource_mut::<Time<Real>>() {
            time_real.update_with_instant(Instant::now());
        }
    }

    if is_verbose() {
        eprintln!("✅ [Hot Reload] {:?} reload complete\n", mode);
    }

    Ok(())
}

/// Mark `ReloadResult` as failed before propagating a registration error.
fn flag_reload_failure(world: &mut World, message: String) {
    let mut result = world.get_resource_or_insert_with(pybevy_core::ReloadResult::default);
    result.failed = true;
    result.failure_reason = Some(message);
    result.running_previous_generation = true;
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
        collections::HashSet,
        sync::{Arc, atomic::Ordering},
    };

    use bevy::{app::Startup, ecs::schedule::Schedules, prelude::*};

    use super::*;
    use crate::{
        profiling::{MemoryProfile, SystemProfiler},
        runtime::{ReloadError, ReloadRuntime},
    };

    /// Minimal mock runtime that succeeds immediately with no systems.
    struct MockRuntime;

    impl ReloadRuntime for MockRuntime {
        type Defs = ();
        type SystemHandle = ();
        fn load_definitions(&mut self, _gen: u32) -> Result<(), ReloadError> {
            Ok(())
        }
        fn requires_escalation(&self, _defs: &()) -> Option<&'static str> {
            None
        }
        fn plugin_names(&self, _defs: &()) -> Vec<String> {
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

    /// Mock runtime that injects a Startup system which writes to
    /// `LastSystemError` - simulating a Python exception (not a panic)
    /// during Startup.
    struct StartupErrorRuntime;

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
        type SystemHandle = ();

        fn load_definitions(&mut self, _gen: u32) -> Result<(), ReloadError> {
            Ok(())
        }
        fn requires_escalation(&self, _defs: &()) -> Option<&'static str> {
            None
        }
        fn plugin_names(&self, _defs: &()) -> Vec<String> {
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
                startup.add_systems(crashing_startup_system);
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

    /// A Python exception (not a panic) in a Startup system must trigger
    /// generation rollback so that Update systems from the broken
    /// generation don't keep running.
    #[test]
    fn startup_exception_rolls_back_generation() {
        let (mut world, gen_counter) = setup_world();
        let state = MockState::new(gen_counter);
        assert_eq!(state.current_generation(), 0);

        let result = perform_reload(
            &mut world,
            &mut StartupErrorRuntime,
            ReloadMode::Full,
            &state,
        );

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
            fn requires_escalation(&self, _defs: &()) -> Option<&'static str> {
                None
            }
            fn plugin_names(&self, _defs: &()) -> Vec<String> {
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

        let pre_entity_count = world.query::<Entity>().iter(&world).count();

        let result = perform_reload(
            &mut world,
            &mut SpawningErrorRuntime,
            ReloadMode::Full,
            &state,
        );

        assert!(result.is_err(), "reload should fail");

        let post_entity_count = world.query::<Entity>().iter(&world).count();
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

        // Step 1: successful reload (gen 0 -> 1)
        let result = perform_reload(&mut world, &mut MockRuntime, ReloadMode::Full, &state);
        assert!(result.is_ok(), "first reload should succeed");
        assert_eq!(state.current_generation(), 1);

        // Step 2: failing reload (gen 1 -> 2, rolled back to 1)
        let result = perform_reload(
            &mut world,
            &mut StartupErrorRuntime,
            ReloadMode::Full,
            &state,
        );
        assert!(result.is_err(), "second reload should fail");
        assert_eq!(state.current_generation(), 1);

        // Step 3: successful reload (gen 1 -> 2 again) - must run Startup
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
        fn requires_escalation(&self, _defs: &()) -> Option<&'static str> {
            None
        }
        fn plugin_names(&self, _defs: &()) -> Vec<String> {
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
        fn requires_escalation(&self, _defs: &()) -> Option<&'static str> {
            None
        }
        fn plugin_names(&self, _defs: &()) -> Vec<String> {
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
        assert!(
            reload_result.running_previous_generation,
            "running_previous_generation should be true",
        );
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
    }
}
