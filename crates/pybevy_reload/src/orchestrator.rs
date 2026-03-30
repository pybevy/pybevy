use bevy::{
    app::Startup,
    ecs::{entity::Entity, query::With, schedule::Schedules, world::World},
};

use crate::{
    HotReloadable,
    cleanup::clear_world_state,
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
        runtime.register_resources(world, &defs)?;
    }

    runtime.register_messages(world, &defs, new_generation)?;

    if mode == ReloadMode::Full {
        runtime.register_observers(world, &defs)?;
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
    let system_handles = runtime.register_systems(world, defs, new_generation)?;

    // Run Startup with rollback on panic
    let pre_startup_error_ts = world
        .get_resource::<pybevy_core::LastSystemError>()
        .map(|e| e.timestamp_secs)
        .unwrap_or(0.0);

    if mode == ReloadMode::Full {
        if world.resource::<Schedules>().contains(Startup) {
            if is_verbose() {
                eprintln!("   → Running Startup schedule");
            }

            let pre_startup_entities: std::collections::HashSet<Entity> = {
                let mut query = world.query_filtered::<Entity, With<HotReloadable>>();
                query.iter(world).collect()
            };

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
                    let mut query = world.query_filtered::<Entity, With<HotReloadable>>();
                    let post_entities: Vec<Entity> = query.iter(world).collect();
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
                    let gen_res = world.resource::<HotReloadGeneration>();
                    if let Ok(mut set) = gen_res.startup_run_for_generations.lock() {
                        set.remove(&old_generation);
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
        .is_some_and(|e| e.error.is_some() && e.timestamp_secs > pre_startup_error_ts);

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

    if is_verbose() {
        eprintln!("✅ [Hot Reload] {:?} reload complete\n", mode);
    }

    Ok(())
}

/// Trait abstracting the generation counter manipulation on the shared hot reload state.
/// This decouples the orchestrator from the PyO3-specific `HotReloadState`.
pub trait HotReloadStateAccess {
    fn current_generation(&self) -> u32;
    fn increment_generation(&self);
    fn set_generation(&self, generation: u32);
}
