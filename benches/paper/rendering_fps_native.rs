//! Native Bevy rendering FPS benchmark.
//!
//! Same benchmark as `benches/paper/rendering_fps.py` but in pure Rust.
//! Spawns increasing numbers of cubes with sine wave animation and measures
//! frame time at each entity count.
//!
//! Usage:
//!     cargo run --example rendering_fps_native --release --features linux-display

use bevy::{ecs::message::Messages, prelude::*, window::PresentMode};

const ENTITY_COUNTS: &[usize] = &[
    1_000, 5_000, 10_000, 50_000, 80_000, 90_000, 100_000, 110_000, 120_000, 130_000, 140_000,
    150_000, 200_000, 250_000, 400_000,
];
const WARMUP_FRAMES: usize = 60;
const MEASURE_FRAMES: usize = 120;

#[derive(Component)]
struct Cube;

#[derive(Resource)]
struct BenchState {
    phase_idx: usize,
    current_entities: usize,
    frame_in_phase: usize,
    frame_times: Vec<f32>,
    cube_mesh: Handle<Mesh>,
    cube_material: Handle<StandardMaterial>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera must see entire grid. With default 45° FOV and spacing=1.2:
    //   half_extent = (sqrt(max_count) + 1) * 1.2 / 2
    //   distance = half_extent / tan(22.5°) ≈ half_extent * 2.5
    let max_count = *ENTITY_COUNTS.last().unwrap();
    let max_side = (max_count as f32).sqrt() as f32 + 1.0;
    let half_extent = max_side * 1.2 / 2.0;
    let cam_dist = half_extent * 2.5;
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, cam_dist * 0.7, cam_dist).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let cube_mesh = meshes.add(Cuboid::new(0.5, 0.5, 0.5));
    let cube_material = materials.add(StandardMaterial::default());

    commands.insert_resource(BenchState {
        phase_idx: 0,
        current_entities: 0,
        frame_in_phase: 0,
        frame_times: Vec::with_capacity(MEASURE_FRAMES),
        cube_mesh,
        cube_material,
    });

    println!(
        "\nRendering FPS Benchmark ({} warmup, {} measured frames)",
        WARMUP_FRAMES, MEASURE_FRAMES
    );
    let header_len = 56;
    println!("{}", "=".repeat(header_len));
    println!();
    println!(
        "{:>10}  {:>12}  {:>10}  {:>10}  {:>18}  {:>8}",
        "Entities", "Median FPS", "Min FPS", "Max FPS", "Frame ms", "Status"
    );
    println!("{}", "-".repeat(82));
}

fn spawn_and_measure(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<BenchState>,
    mut messages: ResMut<Messages<AppExit>>,
) {
    if state.phase_idx >= ENTITY_COUNTS.len() {
        return;
    }

    let target = ENTITY_COUNTS[state.phase_idx];

    // Spawn more entities if needed
    if state.current_entities < target {
        let start = state.current_entities;
        let side = ((target as f32).sqrt() as usize) + 1;
        let spacing = 1.2_f32;
        let mesh = state.cube_mesh.clone();
        let material = state.cube_material.clone();

        for i in start..target {
            let x = (i % side) as f32 * spacing - (side as f32 * spacing / 2.0);
            let z = (i / side) as f32 * spacing - (side as f32 * spacing / 2.0);
            commands.spawn((
                Cube,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(x, 0.0, z),
            ));
        }

        state.current_entities = target;
        state.frame_in_phase = 0;
        state.frame_times.clear();
        return;
    }

    state.frame_in_phase += 1;

    // Warmup
    if state.frame_in_phase <= WARMUP_FRAMES {
        return;
    }

    // Measure
    let dt = time.delta_secs();
    if dt > 0.0 {
        state.frame_times.push(dt);
    }

    if state.frame_in_phase >= WARMUP_FRAMES + MEASURE_FRAMES {
        let target = ENTITY_COUNTS[state.phase_idx];

        if !state.frame_times.is_empty() {
            let mut frame_ms: Vec<f32> = state.frame_times.iter().map(|t| t * 1000.0).collect();
            frame_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mut fps_values: Vec<f32> = state
                .frame_times
                .iter()
                .filter(|&&t| t > 0.0)
                .map(|&t| 1.0 / t)
                .collect();
            fps_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let median_fps = fps_values[fps_values.len() / 2];
            let min_fps = fps_values[0];
            let max_fps = fps_values[fps_values.len() - 1];
            let median_frame_ms = frame_ms[frame_ms.len() / 2];
            let q1 = frame_ms[frame_ms.len() / 4];
            let q3 = frame_ms[3 * frame_ms.len() / 4];
            let iqr = q3 - q1;

            let status = if median_fps >= 60.0 {
                "OK"
            } else if median_fps >= 30.0 {
                "WARN"
            } else {
                "SLOW"
            };

            println!(
                "{:>10}  {:>12.1}  {:>10.1}  {:>10.1}  {:>7.2} +/- {:.2} ms  {:>8}",
                target, median_fps, min_fps, max_fps, median_frame_ms, iqr, status
            );
        }

        // Next phase
        state.phase_idx += 1;
        state.frame_in_phase = 0;
        state.frame_times.clear();

        if state.phase_idx >= ENTITY_COUNTS.len() {
            println!();
            println!("Status: OK = >=60 FPS, WARN = 30-60 FPS, SLOW = <30 FPS");
            messages.write(AppExit::Success);
        }
    }
}

fn animate(time: Res<Time>, mut query: Query<&mut Transform, With<Cube>>) {
    let t = time.elapsed_secs();
    query.par_iter_mut().for_each(|mut transform| {
        let x = transform.translation.x;
        let z = transform.translation.z;
        let dist = (x * x + z * z).sqrt();
        transform.translation.y = (dist * 0.5 - t * 2.0).sin() * 3.0;
    });
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, (animate, spawn_and_measure).chain())
        .run();
}
