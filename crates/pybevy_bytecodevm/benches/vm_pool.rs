//! Benchmarks for VM pool vs direct allocation.
//!
//! Run with: `cargo bench -p pybevy_bytecodevm`

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use pybevy_bytecodevm::bytecode::{
    CompiledBytecode, Compiler, FieldId, FieldType, Op, PooledVM, VM,
};

/// Create a simple increment bytecode: field[0] = field[0] + 1.0
fn create_increment_bytecode() -> CompiledBytecode {
    let mut compiler = Compiler::new();

    let field_id = FieldId {
        component_id: bevy_ecs::component::ComponentId::new(0),
        offset: 0,
        field_type: FieldType::F32,
    };
    let field_idx = compiler.add_field(field_id);
    let const_idx = compiler.add_constant(1.0);

    compiler.emit(Op::PushField(field_idx));
    compiler.emit(Op::PushConst(const_idx));
    compiler.emit(Op::Add);
    compiler.emit(Op::StoreField(field_idx));

    compiler.finalize()
}

/// Benchmark: Direct VM allocation per entity
fn bench_vm_alloc_per_entity(c: &mut Criterion) {
    let bytecode = create_increment_bytecode();
    let mut group = c.benchmark_group("vm_allocation");

    for entity_count in [1_000, 10_000, 100_000, 1_000_000] {
        group.throughput(Throughput::Elements(entity_count as u64));
        group.bench_with_input(
            BenchmarkId::new("direct_alloc", entity_count),
            &entity_count,
            |b, &count| {
                let mut values: Vec<f32> = (0..count).map(|i| i as f32).collect();

                b.iter(|| {
                    for (i, value) in values.iter_mut().enumerate() {
                        // Direct allocation per entity (old pattern)
                        let mut vm = VM::new();
                        let ptr = value as *mut f32 as *mut u8;
                        unsafe {
                            vm.execute(&bytecode, &[ptr], i);
                        }
                    }
                    black_box(&values);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Pooled VM reuse across entities
fn bench_vm_pooled(c: &mut Criterion) {
    let bytecode = create_increment_bytecode();
    let mut group = c.benchmark_group("vm_pooled");

    for entity_count in [1_000, 10_000, 100_000, 1_000_000] {
        group.throughput(Throughput::Elements(entity_count as u64));
        group.bench_with_input(
            BenchmarkId::new("pooled", entity_count),
            &entity_count,
            |b, &count| {
                let mut values: Vec<f32> = (0..count).map(|i| i as f32).collect();

                b.iter(|| {
                    for (i, value) in values.iter_mut().enumerate() {
                        // Pooled VM (new pattern)
                        let mut pooled = PooledVM::acquire();
                        let vm = pooled.get_mut();
                        vm.reset();
                        let ptr = value as *mut f32 as *mut u8;
                        unsafe {
                            vm.execute(&bytecode, &[ptr], i);
                        }
                    }
                    black_box(&values);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Single VM reuse (ideal case - what we'd get with batch processing)
fn bench_vm_single_reuse(c: &mut Criterion) {
    let bytecode = create_increment_bytecode();
    let mut group = c.benchmark_group("vm_single_reuse");

    for entity_count in [1_000, 10_000, 100_000, 1_000_000] {
        group.throughput(Throughput::Elements(entity_count as u64));
        group.bench_with_input(
            BenchmarkId::new("single_vm", entity_count),
            &entity_count,
            |b, &count| {
                let mut values: Vec<f32> = (0..count).map(|i| i as f32).collect();

                b.iter(|| {
                    // Single VM reused for all entities (ideal case)
                    let mut vm = VM::new();
                    for (i, value) in values.iter_mut().enumerate() {
                        vm.reset();
                        let ptr = value as *mut f32 as *mut u8;
                        unsafe {
                            vm.execute(&bytecode, &[ptr], i);
                        }
                    }
                    black_box(&values);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Native Rust baseline (direct field access, no VM)
fn bench_native_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("native_baseline");

    for entity_count in [1_000, 10_000, 100_000, 1_000_000] {
        group.throughput(Throughput::Elements(entity_count as u64));
        group.bench_with_input(
            BenchmarkId::new("direct_rust", entity_count),
            &entity_count,
            |b, &count| {
                let mut values: Vec<f32> = (0..count).map(|i| i as f32).collect();

                b.iter(|| {
                    // Native Rust: direct field increment (what Bevy does)
                    for value in values.iter_mut() {
                        *value += 1.0;
                    }
                    black_box(&values);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Comparison of all approaches including native baseline
fn bench_comparison(c: &mut Criterion) {
    let bytecode = create_increment_bytecode();
    let entity_count = 100_000;
    let mut values: Vec<f32> = (0..entity_count).map(|i| i as f32).collect();

    let mut group = c.benchmark_group("comparison_100k");

    // Native Rust baseline
    group.bench_function("native_rust", |b| {
        b.iter(|| {
            for value in values.iter_mut() {
                *value += 1.0;
            }
            black_box(&values);
        });
    });

    group.bench_function("direct_alloc", |b| {
        b.iter(|| {
            for (i, value) in values.iter_mut().enumerate() {
                let mut vm = VM::new();
                let ptr = value as *mut f32 as *mut u8;
                unsafe {
                    vm.execute(&bytecode, &[ptr], i);
                }
            }
            black_box(&values);
        });
    });

    group.bench_function("pooled", |b| {
        b.iter(|| {
            for (i, value) in values.iter_mut().enumerate() {
                let mut pooled = PooledVM::acquire();
                let vm = pooled.get_mut();
                vm.reset();
                let ptr = value as *mut f32 as *mut u8;
                unsafe {
                    vm.execute(&bytecode, &[ptr], i);
                }
            }
            black_box(&values);
        });
    });

    group.bench_function("single_vm", |b| {
        b.iter(|| {
            let mut vm = VM::new();
            for (i, value) in values.iter_mut().enumerate() {
                vm.reset();
                let ptr = value as *mut f32 as *mut u8;
                unsafe {
                    vm.execute(&bytecode, &[ptr], i);
                }
            }
            black_box(&values);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_native_baseline,
    bench_vm_alloc_per_entity,
    bench_vm_pooled,
    bench_vm_single_reuse,
    bench_comparison,
);
criterion_main!(benches);
