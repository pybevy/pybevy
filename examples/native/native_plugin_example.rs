//! Native Bevy app with Python scripting via PyBevyPlugin.
//!
//! For existing Rust Bevy projects that want to add Python systems.
//! Rust defines components, Python queries and mutates them, Rust reads back.
//!
//! **Security:** PyBevyPlugin embeds a full CPython interpreter with unrestricted
//! access to the host system. Never load untrusted Python code.
//!
//!   cargo run --example native_plugin_example --release

use bevy::prelude::*;
use pybevy::{PyBevyPlugin, PyComponent};

/// Custom component defined in Rust, exposed to Python.
///
/// `#[derive(PyComponent)]` generates:
/// - `PyHealth` wrapper with getters/setters for `value` and `max`
/// - `HealthBridge` implementing `ComponentBridge`
/// - `register_health()` for global registry
#[derive(Component, Default, Clone, Debug, PyComponent)]
struct Health {
    value: f32,
    max: f32,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(
            PyBevyPlugin::new("example_systems")
                .with_python_path("examples/native")
                .register_component(HealthBridge)
                .with_startup_system("setup")
                .with_update_system("rotate_cube")
                .with_update_system("apply_damage")
                .with_hot_reload(),
        )
        // Rust systems
        .add_systems(Startup, spawn_with_health)
        .add_systems(Update, print_health)
        .run();
}

/// Rust Startup: spawn an entity with Health
fn spawn_with_health(mut commands: Commands) {
    commands.spawn((
        Health {
            value: 100.0,
            max: 100.0,
        },
        Transform::from_xyz(0.0, 2.0, 0.0),
    ));
}

/// Rust Update: read Health values (modified by Python each frame)
fn print_health(query: Query<&Health>, mut frame_count: Local<u32>) {
    *frame_count += 1;
    if *frame_count % 60 == 0 {
        for health in &query {
            println!("[Rust] Health: {:.1}/{:.1}", health.value, health.max);
        }
    }
}
