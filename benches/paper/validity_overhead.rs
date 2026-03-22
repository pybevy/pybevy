//! Benchmark validity flag overhead
//!
//! Measures the cost of AtomicU8 validity checks that PyBevy performs
//! on every borrowed component access. Compares direct field access
//! (no check) against access with validity flag checking.

use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use bevy::{
    ecs::{query::QueryState, world::World},
    prelude::*,
};
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

const ENTITY_COUNT: usize = 1_000_000;

/// Simulate the validity check PyBevy performs on every component access.
/// This is the hot path: load an AtomicU8, compare against Invalid (0).
#[inline(always)]
fn check_validity(flag: &AtomicU8) -> bool {
    flag.load(Ordering::Acquire) != 0
}

/// Benchmark 1: Raw atomic load overhead in isolation.
/// Measures how much a single AtomicU8::load(Acquire) costs.
fn bench_raw_atomic_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("validity_flag");

    let flag = Arc::new(AtomicU8::new(2)); // Write mode

    group.bench_function("raw_atomic_load", |b| {
        b.iter(|| {
            black_box(check_validity(&flag));
        });
    });

    group.finish();
}

/// Benchmark 2: Validity check overhead in Query iteration context.
/// Compares iterating 1M transforms with and without a validity check per entity.
fn bench_query_with_validity(c: &mut Criterion) {
    let mut group = c.benchmark_group("validity_in_query");

    let mut world = World::new();
    for i in 0..ENTITY_COUNT {
        world.spawn(Transform::from_translation(Vec3::new(i as f32, 0.0, 0.0)));
    }

    let mut query_state: QueryState<&mut Transform> = world.query::<&mut Transform>();
    let flag = Arc::new(AtomicU8::new(2)); // Write mode (valid)

    // Baseline: direct increment, no validity check
    group.bench_with_input(
        BenchmarkId::new("no_check", ENTITY_COUNT),
        &ENTITY_COUNT,
        |b, _| {
            b.iter(|| {
                for mut t in query_state.iter_mut(&mut world) {
                    t.translation.x = black_box(t.translation.x + 1.0);
                }
            });
        },
    );

    // With validity check: one atomic load per entity
    group.bench_with_input(
        BenchmarkId::new("with_check", ENTITY_COUNT),
        &ENTITY_COUNT,
        |b, _| {
            b.iter(|| {
                for mut t in query_state.iter_mut(&mut world) {
                    if check_validity(&flag) {
                        t.translation.x = black_box(t.translation.x + 1.0);
                    }
                }
            });
        },
    );

    // With validity check + mode match (closer to real PyBevy path)
    group.bench_with_input(
        BenchmarkId::new("with_mode_check", ENTITY_COUNT),
        &ENTITY_COUNT,
        |b, _| {
            b.iter(|| {
                for mut t in query_state.iter_mut(&mut world) {
                    let mode = flag.load(Ordering::Acquire);
                    if mode == 2 {
                        // Write mode
                        t.translation.x = black_box(t.translation.x + 1.0);
                    }
                }
            });
        },
    );

    group.finish();
}

criterion_group!(benches, bench_raw_atomic_load, bench_query_with_validity);
criterion_main!(benches);
