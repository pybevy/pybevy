use std::time::Duration;

use bevy::{
    animation::{AnimationClip, graph::AnimationGraph},
    asset::Assets,
    color::Color,
    ecs::{component::Component, world::World},
    image::{Image, TextureAtlasLayout},
    mesh::Mesh,
    pbr::{StandardMaterial, wireframe::WireframeMaterial},
    prelude::{ImageNode, Resource, TextColor, TextFont, default},
    text::FontSize,
    ui::{Node, PositionType, Val, widget::Text},
    world_serialization::WorldAsset,
};
use sysinfo::ProcessRefreshKind;

use crate::{
    profiling::{HotReloadStats, MemoryProfile, SystemMonitor, SystemProfiler},
    state::ReloadMode,
    util::is_verbose,
};

/// Whether the app started in paused mode (--pause flag)
/// When true, user systems are disabled until Space is pressed
#[derive(Resource)]
pub struct StartPaused(pub bool);

/// Whether memory overlay (F7) is visible
#[derive(Resource)]
pub struct MemoryOverlayVisible(pub bool);

/// Marker component for the hot reload overlay text entity
#[derive(Component)]
pub struct HotReloadOverlayText;

/// Marker component for the hot reload overlay icon entity
#[derive(Component)]
struct HotReloadOverlayIcon;

/// Marker component for the hot reload error text entity
#[derive(Component)]
pub struct HotReloadErrorText;

/// System that spawns the hot reload overlay UI entity
/// Called immediately when hot reload is enabled
pub fn spawn_hot_reload_overlay_system(world: &mut World) {
    let verbose = is_verbose();
    if verbose {
        eprintln!("🎨 [Hot Reload Overlay] Spawning overlay...");
    }

    // Load embedded icon from binary (skip if AssetPlugin not loaded, e.g. in tests)
    static ICON_PNG: &[u8] = include_bytes!("../../../assets/icon.png");
    let icon_handle = match image::load_from_memory(ICON_PNG) {
        Ok(dyn_img) => {
            let image =
                Image::from_dynamic(dyn_img, true, bevy::asset::RenderAssetUsages::default());
            match world.get_resource_mut::<Assets<Image>>() {
                Some(mut assets) => Some(assets.add(image)),
                None => {
                    if verbose {
                        eprintln!("   → Skipping icon (Assets<Image> not available)");
                    }
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("   → WARNING: Failed to decode embedded icon: {e}");
            None
        }
    };

    if let Some(handle) = icon_handle {
        let icon_entity = world.spawn((
            ImageNode::new(handle),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(12.0),
                width: Val::Px(48.0),
                height: Val::Px(48.0),
                ..default()
            },
            HotReloadOverlayIcon,
            #[cfg(feature = "mcp")]
            pybevy_control::bridge::InternalOverlayUi,
        ));

        if verbose {
            eprintln!("   → Spawned overlay icon entity: {:?}", icon_entity.id());
        }
    }

    // Spawn a text entity offset to the right of the icon (64px icon + 8px gap = 84px)
    // UI text works with Camera3d - no Camera2d needed
    let entity = world.spawn((
        Text::new("Hot Reload: Gen 0 | Last: -- | Reloads: 0 | Default: Partial (F6) | F5=Full | Mem: 0.0MB | CPU: 0.0% | GPU: -- | VRAM: --"),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgba(0.0, 1.0, 0.0, 1.0)), // Fully opaque green
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(84.0), // 64px icon + 20px padding
            right: Val::Px(12.0),
            ..default()
        },
        HotReloadOverlayText,
        #[cfg(feature = "mcp")]
        pybevy_control::bridge::InternalOverlayUi,
    ));

    if verbose {
        eprintln!("   → Spawned overlay text entity: {:?}", entity.id());
    }

    // Spawn error text entity below the stats overlay
    let error_entity = world.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 0.3, 0.3, 1.0)), // Red for errors
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(70.0),
            left: Val::Px(84.0),
            right: Val::Px(12.0),
            ..default()
        },
        bevy::prelude::Visibility::Hidden,
        HotReloadErrorText,
        #[cfg(feature = "mcp")]
        pybevy_control::bridge::InternalOverlayUi,
    ));

    if verbose {
        eprintln!(
            "   → Spawned overlay error text entity: {:?}",
            error_entity.id()
        );
    }
}

/// System that updates system stats (memory, CPU, GPU, entities, assets) periodically
pub fn update_system_stats(world: &mut bevy::ecs::world::World) {
    // Extract resources - we need mutable access to monitor and stats
    let Some(mut monitor) = world.remove_resource::<SystemMonitor>() else {
        return;
    };
    let Some(mut stats) = world.remove_resource::<HotReloadStats>() else {
        world.insert_resource(monitor);
        return;
    };
    let Some(time) = world.get_resource::<bevy::time::Time>() else {
        world.insert_resource(monitor);
        world.insert_resource(stats);
        return;
    };

    let current_time = time.elapsed_secs_f64();

    // Update FPS every frame (lightweight calculation)
    let delta = time.delta_secs();
    if delta > 0.0 {
        let fps = 1.0 / delta;
        stats.fps_current = fps;

        // Add to rolling average buffer
        monitor.fps_history.push_back(fps);
        if monitor.fps_history.len() > 60 {
            monitor.fps_history.pop_front();
        }

        // Calculate rolling average
        if !monitor.fps_history.is_empty() {
            stats.fps_average =
                monitor.fps_history.iter().sum::<f32>() / monitor.fps_history.len() as f32;
        }
    }

    // Update uptime
    stats.uptime_secs = current_time;

    // Update stats every 1 second (respects sysinfo's minimum interval while reducing overhead)
    const UPDATE_INTERVAL: f64 = 1.0;
    if current_time - monitor.last_update >= UPDATE_INTERVAL {
        // Only update process stats if we have a valid PID
        if let Some(pid) = monitor.process_pid {
            // On Linux, must refresh global CPU state before process CPU for accurate measurements
            monitor.system.refresh_cpu_all();

            // Refresh process info with explicit CPU tracking enabled
            monitor.system.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[pid]),
                false,
                ProcessRefreshKind::nothing().with_cpu().with_memory(),
            );

            if let Some(process) = monitor.system.process(pid) {
                let new_memory = process.memory() as f64 / 1_048_576.0;
                let per_core_cpu = process.cpu_usage();

                // Divide by number of cores to show fraction of total CPU
                // process.cpu_usage() returns percentage where 100% = 1 full core
                // On a 4-core system, 200% usage = 2 cores = 50% of total CPU
                let num_cores = monitor.system.cpus().len().max(1) as f32;
                let total_cpu = per_core_cpu / num_cores;

                stats.memory_mb = new_memory;
                stats.cpu_percent = total_cpu;
            } else {
                eprintln!("[WARNING] Could not find process {} during update!", pid);
            }
        }

        // Update entity count (O(1) operation)
        stats.entity_count = world.entities().len() as usize;

        // Update asset counts by type (O(1) per type)
        stats.asset_counts.clear();

        if let Some(assets) = world.get_resource::<Assets<Mesh>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("Mesh".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<Image>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("Image".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<StandardMaterial>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("Material".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<AnimationGraph>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("AnimGraph".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<AnimationClip>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("AnimClip".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<WorldAsset>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("WorldAsset".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<TextureAtlasLayout>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("Atlas".to_string(), count);
            }
        }
        if let Some(assets) = world.get_resource::<Assets<WireframeMaterial>>() {
            let count = assets.len();
            if count > 0 {
                stats.asset_counts.insert("Wireframe".to_string(), count);
            }
        }

        monitor.last_update = current_time;

        // Capture memory baseline on first stats update (after Startup has run)
        if stats.memory_mb > 0.0
            && let Some(mut profile) = world.get_resource_mut::<MemoryProfile>()
        {
            profile.capture_baseline(stats.memory_mb);
        }
    }

    // Write cross-crate DebugSnapshot for MCP
    // Extract profiler data first (before mutable world access)
    let (update_profiles, startup_profiles) = world
        .get_resource::<SystemProfiler>()
        .map(|p| {
            let to_profile =
                |(name, avg, max): (String, Duration, Duration)| pybevy_core::SystemProfile {
                    name,
                    avg_ms: avg.as_secs_f64() * 1000.0,
                    max_ms: max.as_secs_f64() * 1000.0,
                };
            let up: Vec<_> = p.get_top_n_update(10).into_iter().map(to_profile).collect();
            let sp: Vec<_> = p
                .get_top_n_startup(10)
                .into_iter()
                .map(to_profile)
                .collect();
            (up, sp)
        })
        .unwrap_or_default();

    let mut asset_counts: Vec<_> = stats
        .asset_counts
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    asset_counts.sort_by(|a, b| a.0.cmp(&b.0));

    // Extract memory profiling data
    let (
        total_schedule_systems,
        python_gc_objects,
        memory_growth_mb,
        memory_peak_mb,
        memory_warning,
        reload_memory_snapshots,
    ) = world
        .get_resource::<MemoryProfile>()
        .map(|profile| {
            let current_rss = stats.memory_mb;
            let snapshots = profile
                .snapshots
                .iter()
                .map(|s| pybevy_core::ReloadMemorySnapshotInfo {
                    generation: s.generation,
                    rss_mb: s.rss_mb,
                    delta_mb: s.delta_mb,
                    gc_objects: s.gc_objects,
                    schedule_systems: s.schedule_systems,
                })
                .collect();
            (
                profile
                    .snapshots
                    .last()
                    .map(|s| s.schedule_systems)
                    .unwrap_or(0),
                profile.snapshots.last().map(|s| s.gc_objects).unwrap_or(0),
                profile.growth_mb(current_rss),
                profile.peak_rss_mb,
                profile.is_warning(current_rss),
                snapshots,
            )
        })
        .unwrap_or_default();

    let snapshot = pybevy_core::DebugSnapshot {
        populated: true,
        reload_count: stats.reload_count,
        last_reload_mode: stats.last_mode.map(|m| match m {
            ReloadMode::Full => "full".to_string(),
            ReloadMode::Partial => "partial".to_string(),
        }),
        reload_failed: world
            .get_resource::<pybevy_core::ReloadResult>()
            .is_some_and(|r| r.failed),
        reload_failure_reason: world
            .get_resource::<pybevy_core::ReloadResult>()
            .and_then(|r| r.failure_reason.clone()),
        memory_mb: stats.memory_mb,
        total_memory_mb: stats.total_memory_mb,
        cpu_percent: stats.cpu_percent,
        cpu_core_count: stats.cpu_core_count,
        fps_average: stats.fps_average,
        fps_current: stats.fps_current,
        uptime_secs: stats.uptime_secs,
        entity_count: stats.entity_count,
        gil_enabled: stats.gil_enabled,
        asset_counts,
        update_profiles,
        startup_profiles,
        total_schedule_systems,
        python_gc_objects,
        memory_growth_mb,
        memory_peak_mb,
        memory_warning,
        reload_memory_snapshots,
    };

    world.insert_resource(snapshot);

    // Re-insert resources
    world.insert_resource(monitor);
    world.insert_resource(stats);
}

/// System that updates the hot reload overlay text
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn render_hot_reload_overlay(
    mut query: bevy::ecs::system::Query<&mut Text, bevy::ecs::query::With<HotReloadOverlayText>>,
    mut error_query: bevy::ecs::system::Query<
        (&mut Text, &mut bevy::prelude::Visibility),
        (
            bevy::ecs::query::With<HotReloadErrorText>,
            bevy::ecs::query::Without<HotReloadOverlayText>,
        ),
    >,
    monitor: Option<bevy::ecs::system::ResMut<SystemMonitor>>,
    stats: Option<bevy::ecs::system::ResMut<HotReloadStats>>,
    profiler: Option<bevy::ecs::system::Res<SystemProfiler>>,
    time: Option<bevy::ecs::system::Res<bevy::time::Time>>,
    last_error: Option<bevy::ecs::system::Res<pybevy_core::LastSystemError>>,
    memory_profile: Option<bevy::ecs::system::Res<MemoryProfile>>,
    reload_result: Option<bevy::ecs::system::Res<pybevy_core::ReloadResult>>,
    memory_visible: Option<bevy::ecs::system::Res<MemoryOverlayVisible>>,
    start_paused: Option<bevy::ecs::system::Res<StartPaused>>,
) {
    // Skip if resources aren't available
    let (Some(mut monitor), Some(mut stats), Some(time)) = (monitor, stats, time) else {
        return;
    };

    let current_time = time.elapsed_secs_f64();

    // Throttle updates to 4 times per second (every 250ms) for readability
    const RENDER_INTERVAL: f64 = 0.25;
    if current_time - monitor.last_render_update < RENDER_INTERVAL {
        return;
    }
    monitor.last_render_update = current_time;

    let is_paused = start_paused.as_ref().is_some_and(|p| p.0);
    let reload_failed = reload_result.as_ref().is_some_and(|r| r.failed);

    for mut text in query.iter_mut() {
        let last_mode_str = if reload_failed {
            "FAILED (prev gen)"
        } else {
            match stats.last_mode {
                Some(ReloadMode::Full) => "Full",
                Some(ReloadMode::Partial) => "Partial",
                None => "--",
            }
        };

        // Format uptime (e.g., "1m23s")
        let uptime = format_uptime(stats.uptime_secs);

        // GIL status display
        let gil_str = if stats.gil_enabled {
            "GIL"
        } else {
            "Free-threaded"
        };

        // Format asset counts
        let assets_str = if stats.asset_counts.is_empty() {
            "Assets: --".to_string()
        } else {
            let mut counts: Vec<_> = stats.asset_counts.iter().collect();
            counts.sort_by_key(|(name, _)| *name);
            let formatted = counts
                .into_iter()
                .map(|(name, count)| format!("{}:{}", name, count))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Assets: {}", formatted)
        };

        // Line 1: System info

        let line1 = format!(
            "Reloads: {} | Last: {} | RAM: {:.0}/{:.0}MB | CPU: {:.1}% ({}c) | {} | Up: {} | FPS: {:.0}/{:.0} | Entities: {} | {}",
            stats.reload_count,
            last_mode_str,
            stats.memory_mb,
            stats.total_memory_mb,
            stats.cpu_percent,
            stats.cpu_core_count,
            gil_str,
            uptime,
            stats.fps_average,
            stats.fps_current,
            stats.entity_count,
            assets_str,
        );

        // Line 2: Update/Last systems profile
        let line2 = if let Some(p) = profiler.as_ref() {
            let top5 = p.get_top_n_update(5);
            if !top5.is_empty() {
                let systems = top5
                    .into_iter()
                    .map(|(n, avg, _max)| format!("{}({:.2}ms)", n, avg.as_secs_f64() * 1000.0))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("Profile: {}", systems)
            } else {
                "Profile: --".to_string()
            }
        } else {
            "Profile: --".to_string()
        };

        // Line 3: Startup systems (first 5s only)
        let line3 = if let Some(p) = profiler.as_ref() {
            if p.should_show_startup(current_time) {
                let top5 = p.get_top_n_startup(5);
                if !top5.is_empty() {
                    let systems = top5
                        .into_iter()
                        .map(|(n, avg, _max)| format!("{}({:.2}ms)", n, avg.as_secs_f64() * 1000.0))
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("Startup: {}", systems)
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Line 4: Memory profiling (only when F7 toggle is active)
        let line4 = if memory_visible.as_ref().is_some_and(|v| v.0) {
            if let Some(profile) = memory_profile.as_ref() {
                let mut parts = Vec::new();
                if profile.baseline_captured {
                    let growth = profile.growth_mb(stats.memory_mb);
                    let warning = if profile.is_warning(stats.memory_mb) {
                        " WARN"
                    } else {
                        ""
                    };
                    parts.push(format!(
                        "Growth: {:.1}MB | Peak: {:.1}MB{}",
                        growth, profile.peak_rss_mb, warning
                    ));
                }
                // Show last few reload deltas as a mini trend
                let recent: Vec<_> = profile.snapshots.iter().rev().take(5).collect();
                if !recent.is_empty() {
                    let trend = recent
                        .iter()
                        .rev()
                        .map(|s| {
                            let sign = if s.delta_mb >= 0.0 { "+" } else { "" };
                            format!("g{}:{}{:.1}", s.generation, sign, s.delta_mb)
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    parts.push(format!("Trend: [{}]", trend));

                    // Show GC + systems from latest snapshot
                    if let Some(latest) = profile.snapshots.last() {
                        parts.push(format!(
                            "GC: {} | Systems: {}",
                            latest.gc_objects, latest.schedule_systems
                        ));
                    }
                }
                if parts.is_empty() {
                    "Memory: awaiting baseline...".to_string()
                } else {
                    format!("Memory: {}", parts.join(" | "))
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Combine lines (line3/line4 may be empty)
        let mut output = if is_paused {
            format!(
                "PAUSED — press Space to load | F5=Full reload\n{}\n{}",
                line1, line2
            )
        } else {
            format!("{}\n{}", line1, line2)
        };
        if !line3.is_empty() {
            output.push('\n');
            output.push_str(&line3);
        }
        if !line4.is_empty() {
            output.push('\n');
            output.push_str(&line4);
        }
        text.0 = output;
    }

    // Update error text overlay
    for (mut error_text, mut visibility) in error_query.iter_mut() {
        let has_error = last_error.as_ref().and_then(|e| e.error.as_ref()).is_some();

        if let Some(last_err) = last_error.as_ref()
            && last_err.error.is_some()
            && last_err.timestamp_secs > stats.last_error_timestamp
        {
            // Extract the meaningful error line from traceback or error message
            let error_msg = last_err
                .traceback
                .as_deref()
                .and_then(|tb| {
                    // Get the last non-empty line (the actual error)
                    tb.lines().rev().find(|l| !l.trim().is_empty())
                })
                .or(last_err.error.as_deref())
                .unwrap_or("Unknown error");

            let display_msg = if error_msg.len() > 120 {
                format!("Error: {}...", &error_msg[..117])
            } else {
                format!("Error: {}", error_msg)
            };

            error_text.0 = display_msg;
            stats.last_error_timestamp = last_err.timestamp_secs;
        }

        *visibility = if has_error {
            bevy::prelude::Visibility::Inherited
        } else {
            bevy::prelude::Visibility::Hidden
        };
    }
}

/// Format uptime in human-readable format (e.g., "1m23s")
pub fn format_uptime(secs: f64) -> String {
    let total_secs = secs as u32;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if mins > 0 {
        format!("{}m{}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}
