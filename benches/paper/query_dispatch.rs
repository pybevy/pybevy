//! Benchmark comparing static vs dynamic query dispatch
//!
//! This isolates the query mechanism overhead independent of Python/registry lookups.

use std::sync::Once;

use bevy::{
    ecs::{
        query::{QueryBuilder, QueryState},
        world::{FilteredEntityMut, World},
    },
    prelude::*,
    tasks::ComputeTaskPool,
};
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

const ENTITY_COUNTS: &[usize] = &[10_000, 100_000, 1_000_000];

static INIT: Once = Once::new();

fn init_task_pool() {
    INIT.call_once(|| {
        ComputeTaskPool::get_or_init(|| {
            let num_threads = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4);
            bevy::tasks::TaskPoolBuilder::default()
                .num_threads(num_threads)
                .build()
        });
    });
}

/// Benchmark: Static typed query with par_iter_mut
fn bench_static_query(c: &mut Criterion) {
    init_task_pool();
    let mut group = c.benchmark_group("static_query");

    for &count in ENTITY_COUNTS {
        let mut world = World::new();

        // Spawn entities
        for i in 0..count {
            world.spawn(Transform::from_translation(Vec3::new(i as f32, 0.0, 0.0)));
        }

        // Pre-build query state (like Bevy systems do)
        let mut query_state: QueryState<&mut Transform> = world.query::<&mut Transform>();

        group.bench_with_input(BenchmarkId::new("increment", count), &count, |b, _| {
            b.iter(|| {
                query_state.par_iter_mut(&mut world).for_each(|mut t| {
                    t.translation.x += 1.0;
                    t.translation.y += 1.0;
                    t.translation.z += 1.0;
                });
                black_box(());
            });
        });
    }

    group.finish();
}

/// Benchmark: Dynamic query built each frame with mut_id
fn bench_dynamic_query_rebuild(c: &mut Criterion) {
    init_task_pool();
    let mut group = c.benchmark_group("dynamic_query_rebuild");

    for &count in ENTITY_COUNTS {
        let mut world = World::new();

        // Register and get component ID
        let component_id = world.register_component::<Transform>();

        // Spawn entities
        for i in 0..count {
            world.spawn(Transform::from_translation(Vec3::new(i as f32, 0.0, 0.0)));
        }

        group.bench_with_input(BenchmarkId::new("increment", count), &count, |b, _| {
            b.iter(|| {
                // Build query each iteration (like current dynamic path)
                let mut query_builder = QueryBuilder::<FilteredEntityMut>::new(&mut world);
                query_builder.mut_id(component_id);
                let mut query_state = query_builder.build();

                query_state
                    .par_iter_mut(&mut world)
                    .for_each(|mut entity: FilteredEntityMut| {
                        if let Some(mut t) = entity.get_mut::<Transform>() {
                            t.translation.x += 1.0;
                            t.translation.y += 1.0;
                            t.translation.z += 1.0;
                        }
                    });
                black_box(());
            });
        });
    }

    group.finish();
}

/// Benchmark: Dynamic query with cached QueryState
fn bench_dynamic_query_cached(c: &mut Criterion) {
    init_task_pool();
    let mut group = c.benchmark_group("dynamic_query_cached");

    for &count in ENTITY_COUNTS {
        let mut world = World::new();

        // Register and get component ID
        let component_id = world.register_component::<Transform>();

        // Spawn entities
        for i in 0..count {
            world.spawn(Transform::from_translation(Vec3::new(i as f32, 0.0, 0.0)));
        }

        // Pre-build dynamic query (cached)
        let mut query_builder = QueryBuilder::<FilteredEntityMut>::new(&mut world);
        query_builder.mut_id(component_id);
        let mut query_state = query_builder.build();

        group.bench_with_input(BenchmarkId::new("increment", count), &count, |b, _| {
            b.iter(|| {
                query_state
                    .par_iter_mut(&mut world)
                    .for_each(|mut entity: FilteredEntityMut| {
                        if let Some(mut t) = entity.get_mut::<Transform>() {
                            t.translation.x += 1.0;
                            t.translation.y += 1.0;
                            t.translation.z += 1.0;
                        }
                    });
                black_box(());
            });
        });
    }

    group.finish();
}

/// Benchmark: Dynamic query using get_mut_by_id (raw ComponentId access)
fn bench_dynamic_query_by_id(c: &mut Criterion) {
    init_task_pool();
    let mut group = c.benchmark_group("dynamic_query_by_id");

    for &count in ENTITY_COUNTS {
        let mut world = World::new();

        // Register and get component ID
        let component_id = world.register_component::<Transform>();

        // Spawn entities
        for i in 0..count {
            world.spawn(Transform::from_translation(Vec3::new(i as f32, 0.0, 0.0)));
        }

        // Pre-build dynamic query (cached)
        let mut query_builder = QueryBuilder::<FilteredEntityMut>::new(&mut world);
        query_builder.mut_id(component_id);
        let mut query_state = query_builder.build();

        group.bench_with_input(BenchmarkId::new("increment", count), &count, |b, _| {
            b.iter(|| {
                query_state
                    .par_iter_mut(&mut world)
                    .for_each(|mut entity: FilteredEntityMut| {
                        // Use get_mut_by_id like the actual View code does
                        if let Some(mut untyped) = entity.get_mut_by_id(component_id) {
                            let ptr = untyped.as_mut().as_ptr() as *mut Transform;
                            unsafe {
                                (*ptr).translation.x += 1.0;
                                (*ptr).translation.y += 1.0;
                                (*ptr).translation.z += 1.0;
                            }
                        }
                    });
                black_box(());
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_static_query,
    bench_dynamic_query_rebuild,
    bench_dynamic_query_cached,
    bench_dynamic_query_by_id,
);
criterion_main!(benches);
