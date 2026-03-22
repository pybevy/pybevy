//! Benchmark native Rust physics kernel
//!
//! Same physics kernel as Python paper benchmarks:
//!   pos.x = x + y * dt + 0.5 * y * y * dt
//!   pos.y = y + sin(x * 0.1) * dt
//!   pos.z = z + cos(x * 0.1) * dt
//!
//! Two groups:
//! - `physics_par_iter`: parallel iteration (matches Numba parallel)
//! - `physics_iter`: single-threaded baseline

use std::sync::Once;

use bevy::{
    ecs::{query::QueryState, world::World},
    prelude::*,
};
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

const ENTITY_COUNTS: &[usize] = &[5_000, 10_000, 100_000, 1_000_000];
const DT: f32 = 0.016;

static INIT: Once = Once::new();

fn init_task_pool() {
    INIT.call_once(|| {
        bevy::tasks::ComputeTaskPool::get_or_init(|| {
            let num_threads = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4);
            bevy::tasks::TaskPoolBuilder::default()
                .num_threads(num_threads)
                .build()
        });
    });
}

fn spawn_entities(world: &mut World, count: usize) {
    for i in 0..count {
        world.spawn(Transform::from_translation(Vec3::new(
            i as f32,
            i as f32 * 0.5,
            0.0,
        )));
    }
}

/// Physics kernel: parallel iteration (par_iter_mut)
fn bench_physics_par_iter(c: &mut Criterion) {
    init_task_pool();
    let mut group = c.benchmark_group("physics_par_iter");

    for &count in ENTITY_COUNTS {
        let mut world = World::new();
        spawn_entities(&mut world, count);
        let mut query_state: QueryState<&mut Transform> = world.query::<&mut Transform>();

        group.bench_with_input(BenchmarkId::new("par_iter", count), &count, |b, _| {
            b.iter(|| {
                query_state.par_iter_mut(&mut world).for_each(|mut t| {
                    let x = t.translation.x;
                    let y = t.translation.y;
                    let z = t.translation.z;
                    t.translation.x = x + y * DT + 0.5 * y * y * DT;
                    t.translation.y = y + (x * 0.1).sin() * DT;
                    t.translation.z = z + (x * 0.1).cos() * DT;
                });
                black_box(());
            });
        });
    }
    group.finish();
}

/// Physics kernel: single-threaded iteration (iter_mut)
fn bench_physics_iter(c: &mut Criterion) {
    init_task_pool();
    let mut group = c.benchmark_group("physics_iter");

    for &count in ENTITY_COUNTS {
        let mut world = World::new();
        spawn_entities(&mut world, count);
        let mut query_state: QueryState<&mut Transform> = world.query::<&mut Transform>();

        group.bench_with_input(BenchmarkId::new("iter", count), &count, |b, _| {
            b.iter(|| {
                for mut t in query_state.iter_mut(&mut world) {
                    let x = t.translation.x;
                    let y = t.translation.y;
                    let z = t.translation.z;
                    t.translation.x = x + y * DT + 0.5 * y * y * DT;
                    t.translation.y = y + (x * 0.1).sin() * DT;
                    t.translation.z = z + (x * 0.1).cos() * DT;
                }
                black_box(());
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_physics_par_iter, bench_physics_iter,);
criterion_main!(benches);
