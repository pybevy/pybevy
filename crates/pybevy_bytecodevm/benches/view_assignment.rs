//! Compares tiled and scalar assignment over a strided ECS component field.
//!
//! Run with:
//! `RUSTFLAGS="-C target-cpu=native" cargo bench -p pybevy_bytecodevm --bench view_assignment`

use bevy_ecs::{component::ComponentId, prelude::Component, world::World};
use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use pybevy_bytecodevm::{
    bytecode::{Compiler, FieldId, FieldType, Op, VM},
    tiled::{TiledScratch, execute_assignment_tiled},
};

const N: usize = 1_000_000;

#[derive(Component)]
#[allow(dead_code)]
struct Particle {
    a: f32,
    b: f32,
    c: f32,
}

fn add_bytecode(component_id: ComponentId) -> pybevy_bytecodevm::bytecode::CompiledBytecode {
    let mut compiler = Compiler::new();
    let field = compiler.add_field(FieldId {
        component_id,
        offset: 0,
        field_type: FieldType::F32,
    });
    let constant = compiler.add_constant(1.0000001);
    compiler.emit(Op::PushField(field));
    compiler.emit(Op::PushConst(constant));
    compiler.emit(Op::Add);
    compiler.emit(Op::StoreField(field));
    compiler.finalize()
}

fn benches(c: &mut Criterion) {
    let mut world = World::new();
    let component_id = world.register_component::<Particle>();
    for i in 0..N {
        world.spawn(Particle {
            a: i as f32 * 0.5,
            b: 1.0,
            c: 2.0,
        });
    }

    let stride = std::mem::size_of::<Particle>();
    let base = {
        let mut query = world.query::<&mut Particle>();
        let first = query.iter_mut(&mut world).next().expect("entities exist");
        first.into_inner() as *mut Particle as *mut u8
    };
    let bases = [base];
    let strides = [stride];
    let bytecode = add_bytecode(component_id);

    let mut group = c.benchmark_group("view_assignment_add");
    group.throughput(Throughput::Elements(N as u64));

    group.bench_function("tiled", |b| {
        let mut scratch = TiledScratch::new();
        b.iter(|| {
            // SAFETY: the base and stride describe N live Particle rows, and the
            // benchmark provides exclusive single-threaded access to the world.
            unsafe {
                execute_assignment_tiled(&bytecode, &bases, &strides, N, &mut scratch).unwrap();
            }
            black_box(base);
        })
    });

    group.bench_function("scalar", |b| {
        let mut vm = VM::new();
        b.iter(|| {
            // SAFETY: the same live, exclusively accessed rows are used above.
            unsafe {
                vm.execute_batch_multi(&bytecode, &bases, &strides, N);
            }
            black_box(base);
        })
    });

    group.finish();
}

criterion_group!(benches_group, benches);
criterion_main!(benches_group);
