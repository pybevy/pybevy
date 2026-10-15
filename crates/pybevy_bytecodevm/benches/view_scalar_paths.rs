//! Per-row cost of the View paths that do not run the tiled executor on one
//! contiguous batch: read-only evaluation and tick-filtered assignment.
//!
//! Run with: `cargo bench -p pybevy_bytecodevm --bench view_scalar_paths`

use std::{
    collections::{HashMap, HashSet},
    mem::size_of,
    sync::Arc,
};

use bevy_ecs::{change_detection::Tick, component::Component, world::World};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use pybevy_bytecodevm::{
    bytecode::{FieldId, FieldType},
    expr::RustExpr,
    view_engine::ViewFilter,
    view_runtime::{CachedViewCore, ResolvedViewSpec, ViewReduction, ViewRuntimeCore},
};
use pybevy_storage::ValidityFlag;

const N: usize = 200_000;

#[derive(Component)]
#[repr(C)]
struct Health {
    hp: f32,
    max: f32,
}

fn runtime(world: &mut World, changed: bool) -> Arc<ViewRuntimeCore> {
    let component_id = world.register_component::<Health>();
    let filter = ViewFilter {
        component_ids: HashSet::from([component_id]),
        with_ids: Vec::new(),
        without_ids: Vec::new(),
        changed_ids: if changed {
            vec![component_id]
        } else {
            Vec::new()
        },
        added_ids: Vec::new(),
    };
    // SAFETY: the metadata describes `Health` as registered in this World.
    let spec = unsafe {
        ResolvedViewSpec::new(
            world.id(),
            filter,
            HashSet::from([component_id]),
            HashMap::from([(
                component_id,
                HashSet::from([(0, FieldType::F32), (4, FieldType::F32)]),
            )]),
            HashMap::from([(component_id, size_of::<Health>())]),
        )
    }
    .unwrap();
    let cached = Arc::new(CachedViewCore::new(spec, world).unwrap());
    let this_run = world.change_tick();
    // SAFETY: the World outlives the runtime and is not mutated structurally
    // while the benchmark runs.
    let runtime = unsafe {
        ViewRuntimeCore::new(
            cached,
            world.as_unsafe_world_cell(),
            ValidityFlag::new_write(),
            Tick::new(0),
            this_run,
        )
    }
    .unwrap();
    Arc::new(runtime)
}

fn benches(c: &mut Criterion) {
    let mut world = World::new();
    for index in 0..N {
        world.spawn(Health {
            hp: index as f32,
            max: 100.0,
        });
    }
    let component_id = world.register_component::<Health>();
    let field = |offset| RustExpr::Field {
        component_id,
        offset,
        field_type: FieldType::F32,
    };
    let expression = RustExpr::Add(
        Box::new(RustExpr::Mul(
            Box::new(field(0)),
            Box::new(RustExpr::FloatConst(0.5)),
        )),
        Box::new(field(4)),
    );
    let destination = FieldId {
        component_id,
        offset: 0,
        field_type: FieldType::F32,
    };

    let mut group = c.benchmark_group("view_scalar_paths");
    group.throughput(Throughput::Elements(N as u64));

    let plain = runtime(&mut world, false);
    let read = plain.prepare_read_program(&expression).unwrap();
    group.bench_function("evaluate_sum", |b| {
        b.iter(|| {
            let lease = plain.gather_batches().unwrap();
            lease.reduce(&read, ViewReduction::Sum, false).unwrap()
        })
    });
    let assign = plain
        .prepare_assignment_program(destination, &expression)
        .unwrap();
    group.bench_function("unfiltered_assignment", |b| {
        b.iter(|| {
            let lease = plain.gather_batches().unwrap();
            lease.execute_assignment(&assign, false).unwrap()
        })
    });

    let changed = runtime(&mut world, true);
    let assign = changed
        .prepare_assignment_program(destination, &expression)
        .unwrap();
    group.bench_function("changed_filtered_assignment", |b| {
        b.iter(|| {
            let lease = changed.gather_batches().unwrap();
            lease.execute_assignment(&assign, false).unwrap()
        })
    });
    group.finish();
}

criterion_group!(benches_group, benches);
criterion_main!(benches_group);
