//! Benchmark VM allocation overhead
//!
//! Isolates the cost of creating VM and field_ptrs per entity

use std::{cell::RefCell, sync::Once};

use bevy::{
    ecs::{query::QueryState, world::World},
    prelude::*,
};
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use pybevy_bytecodevm::bytecode::{CompiledBytecode, FieldId, FieldType, Op, VM};

const ENTITY_COUNTS: &[usize] = &[10_000, 100_000, 1_000_000];

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

/// Create simple increment bytecode: field[0] = field[0] + 1.0
fn create_increment_bytecode() -> CompiledBytecode {
    CompiledBytecode {
        bytecode: vec![
            Op::PushField(0),
            Op::PushConst(0), // Index into constants
            Op::Add,
            Op::StoreField(0),
        ],
        constants: vec![1.0], // The constant 1.0
        field_map: vec![FieldId {
            component_id: bevy::ecs::component::ComponentId::new(0),
            offset: 0, // translation.x offset in Transform
            field_type: FieldType::F32,
        }],
    }
}

/// Baseline: Direct increment (no VM)
fn bench_direct_increment(c: &mut Criterion) {
    init_task_pool();
    let mut group = c.benchmark_group("direct_increment");

    for &count in ENTITY_COUNTS {
        let mut world = World::new();
        for i in 0..count {
            world.spawn(Transform::from_translation(Vec3::new(i as f32, 0.0, 0.0)));
        }
        let mut query_state: QueryState<&mut Transform> = world.query::<&mut Transform>();

        group.bench_with_input(BenchmarkId::new("baseline", count), &count, |b, _| {
            b.iter(|| {
                query_state.par_iter_mut(&mut world).for_each(|mut t| {
                    t.translation.x += 1.0;
                });
                black_box(());
            });
        });
    }
    group.finish();
}

/// Current View approach: New VM + Vec per entity
fn bench_vm_per_entity(c: &mut Criterion) {
    init_task_pool();
    let mut group = c.benchmark_group("vm_per_entity");
    let bytecode = create_increment_bytecode();

    for &count in ENTITY_COUNTS {
        let mut world = World::new();
        for i in 0..count {
            world.spawn(Transform::from_translation(Vec3::new(i as f32, 0.0, 0.0)));
        }
        let mut query_state: QueryState<&mut Transform> = world.query::<&mut Transform>();

        group.bench_with_input(
            BenchmarkId::new("alloc_per_entity", count),
            &count,
            |b, _| {
                b.iter(|| {
                    query_state.par_iter_mut(&mut world).for_each(|mut t| {
                        // Current approach: allocate VM and Vec per entity
                        let mut vm = VM::new();
                        let mut field_ptrs: Vec<*mut f32> = Vec::with_capacity(1);

                        let ptr = &mut t.translation.x as *mut f32;
                        field_ptrs.push(ptr);

                        unsafe {
                            vm.execute(
                                &bytecode,
                                &*(field_ptrs.as_slice() as *const [*mut f32]
                                    as *const [*mut u8]),
                                0,
                            );
                        }
                    });
                    black_box(());
                });
            },
        );
    }
    group.finish();
}

/// Optimized: Reuse VM via thread_local
fn bench_vm_reused(c: &mut Criterion) {
    init_task_pool();
    let mut group = c.benchmark_group("vm_reused");
    let bytecode = create_increment_bytecode();

    for &count in ENTITY_COUNTS {
        let mut world = World::new();
        for i in 0..count {
            world.spawn(Transform::from_translation(Vec3::new(i as f32, 0.0, 0.0)));
        }
        let mut query_state: QueryState<&mut Transform> = world.query::<&mut Transform>();

        group.bench_with_input(BenchmarkId::new("thread_local_vm", count), &count, |b, _| {
            b.iter(|| {
                query_state.par_iter_mut(&mut world).for_each(|mut t| {
                    thread_local! {
                        static VM_CACHE: RefCell<VM> = RefCell::new(VM::new());
                        static PTR_CACHE: RefCell<Vec<*mut f32>> = RefCell::new(Vec::with_capacity(8));
                    }

                    VM_CACHE.with(|vm_cell: &RefCell<VM>| {
                        PTR_CACHE.with(|ptr_cell: &RefCell<Vec<*mut f32>>| {
                            let mut vm = vm_cell.borrow_mut();
                            let mut field_ptrs = ptr_cell.borrow_mut();
                            field_ptrs.clear();

                            let ptr = &mut t.translation.x as *mut f32;
                            field_ptrs.push(ptr);

                            unsafe {
                                vm.execute(
                                    &bytecode,
                                    &*(field_ptrs.as_slice() as *const [*mut f32] as *const [*mut u8]),
                                    0,
                                );
                            }
                        });
                    });
                });
                black_box(());
            });
        });
    }
    group.finish();
}

/// Optimized: Fixed-size array instead of Vec
fn bench_vm_fixed_array(c: &mut Criterion) {
    init_task_pool();
    let mut group = c.benchmark_group("vm_fixed_array");
    let bytecode = create_increment_bytecode();

    for &count in ENTITY_COUNTS {
        let mut world = World::new();
        for i in 0..count {
            world.spawn(Transform::from_translation(Vec3::new(i as f32, 0.0, 0.0)));
        }
        let mut query_state: QueryState<&mut Transform> = world.query::<&mut Transform>();

        group.bench_with_input(BenchmarkId::new("fixed_array", count), &count, |b, _| {
            b.iter(|| {
                query_state.par_iter_mut(&mut world).for_each(|mut t| {
                    thread_local! {
                        static VM_CACHE: RefCell<VM> = RefCell::new(VM::new());
                    }

                    VM_CACHE.with(|vm_cell: &RefCell<VM>| {
                        let mut vm = vm_cell.borrow_mut();

                        // Fixed-size array on stack - no allocation
                        let field_ptrs: [*mut u8; 1] =
                            [&mut t.translation.x as *mut f32 as *mut u8];

                        unsafe {
                            vm.execute(&bytecode, &field_ptrs, 0);
                        }
                    });
                });
                black_box(());
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_direct_increment,
    bench_vm_per_entity,
    bench_vm_reused,
    bench_vm_fixed_array,
);
criterion_main!(benches);
