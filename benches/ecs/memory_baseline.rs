#![allow(dead_code)]
//! Memory baseline: pure Rust Bevy entity spawning with RSS measurement.
//!
//! Uses a full Bevy App with MinimalPlugins and a Startup system to spawn
//! entities — matching how PyBevy spawns entities (via App + systems).
//!
//! Usage:
//!     cargo run --example memory_baseline --release --features linux-display -- <scenario> <N>
//!
//! Scenarios:
//!     transform           — Transform only (48 bytes)
//!     velocity_small      — Custom Velocity { vel: Vec3 } (12 bytes)
//!     velocity_string     — Custom VelocityStr { vel: Vec3, label: String }
//!     transform_velocity  — Transform + Velocity (two components)
//!     transform_marker    — Transform + zero-size Marker

use bevy::{app::ScheduleRunnerPlugin, prelude::*};

#[derive(Component, Clone)]
struct Velocity {
    #[allow(dead_code)]
    vel: Vec3,
}

#[derive(Component, Clone)]
struct VelocityStr {
    #[allow(dead_code)]
    vel: Vec3,
    #[allow(dead_code)]
    label: String,
}

#[derive(Component)]
struct Marker;

#[derive(Resource)]
struct SpawnConfig {
    scenario: String,
    count: usize,
}

fn get_rss_bytes() -> usize {
    let contents = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let pages: usize = contents
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    pages * page_size()
}

fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

fn spawn_entities(mut commands: Commands, config: Res<SpawnConfig>) {
    let n = config.count;
    match config.scenario.as_str() {
        "transform" => {
            for i in 0..n {
                commands.spawn(Transform::from_xyz(i as f32, 0.0, 0.0));
            }
        }
        "velocity_small" => {
            for i in 0..n {
                commands.spawn(Velocity {
                    vel: Vec3::new(i as f32, 0.0, 0.0),
                });
            }
        }
        "velocity_string" => {
            for i in 0..n {
                commands.spawn(VelocityStr {
                    vel: Vec3::new(i as f32, 0.0, 0.0),
                    label: String::from("entity"),
                });
            }
        }
        "transform_velocity" => {
            for i in 0..n {
                commands.spawn((
                    Transform::from_xyz(i as f32, 0.0, 0.0),
                    Velocity {
                        vel: Vec3::new(1.0, 0.0, 0.0),
                    },
                ));
            }
        }
        "transform_marker" => {
            for i in 0..n {
                commands.spawn((Transform::from_xyz(i as f32, 0.0, 0.0), Marker));
            }
        }
        _ => {
            eprintln!("Unknown scenario: {}", config.scenario);
            std::process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: memory_baseline <scenario> <entity_count>\n\
             Scenarios: transform, velocity_small, velocity_string, transform_velocity, transform_marker"
        );
        std::process::exit(1);
    }

    let scenario = args[1].clone();
    let n: usize = args[2].parse().expect("entity_count must be a number");

    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
    app.insert_resource(SpawnConfig {
        scenario: scenario.clone(),
        count: n,
    });
    app.add_systems(Startup, spawn_entities);

    // Initialize (registers schedules, applies plugins)
    app.finish();
    app.cleanup();

    // Measure RSS around the update that runs Startup systems
    let _ = get_rss_bytes();
    let rss_before = get_rss_bytes();

    app.update();

    let rss_after = get_rss_bytes();
    let delta = rss_after.saturating_sub(rss_before);
    let bpe = delta as f64 / n as f64;

    println!(
        r#"{{"scenario":"{}","entities":{},"rss_before":{},"rss_after":{},"delta":{},"bpe":{:.1}}}"#,
        scenario, n, rss_before, rss_after, delta, bpe
    );
}
