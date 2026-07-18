//! Compares tiled and scalar execution over contiguous bounded-array columns.
//!
//! Run with:
//! `RUSTFLAGS="-C target-cpu=native" cargo bench -p pybevy_array --bench elementwise`

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use pybevy_array::{
    ArrayStorage, DenseArrayCore,
    kernels::{
        Operand, OperandRef, evaluate_float_expression, float_elementwise,
        float_elementwise_borrowed,
    },
};
use pybevy_bytecodevm::{
    bytecode::Op,
    columns::{ColumnMut, ColumnRef},
    dense::{DenseInput, DenseOutput, DenseProgram, execute_dense},
    tiled::{TiledScratch, f32_native_eligible, run_map, run_map_f32},
};

const N: usize = 1_000_000;

fn add_ops() -> Vec<Op> {
    vec![Op::PushInput(0), Op::PushInput(1), Op::Add]
}

fn heavy_ops() -> Vec<Op> {
    vec![
        Op::PushInput(0),
        Op::PushInput(0),
        Op::Mul,
        Op::PushInput(1),
        Op::PushInput(1),
        Op::Mul,
        Op::Add,
        Op::Sqrt,
    ]
}

fn fused_ops() -> Vec<Op> {
    vec![
        Op::PushInput(0),
        Op::PushInput(1),
        Op::PushInput(2),
        Op::Mul,
        Op::Add,
    ]
}

fn bench(c: &mut Criterion, name: &str, ops: &[Op]) {
    let mut group = c.benchmark_group(name);
    group.throughput(Throughput::Elements(N as u64));

    let a64: Vec<f64> = (0..N).map(|i| i as f64 * 0.5).collect();
    let b64: Vec<f64> = (0..N).map(|i| (i as f64).sin()).collect();
    let a32: Vec<f32> = (0..N).map(|i| i as f32 * 0.5).collect();
    let b32: Vec<f32> = (0..N).map(|i| (i as f32).sin()).collect();
    let mut scratch = TiledScratch::new();

    group.bench_function("tiled_f64", |b| {
        b.iter(|| {
            let sources = [
                ColumnRef::from_f64_slice(&a64),
                ColumnRef::from_f64_slice(&b64),
            ];
            let mut output = vec![0.0_f64; N];
            let destination = ColumnMut::from_f64_slice(&mut output);
            // SAFETY: every column is contiguous and length N, and the output
            // does not alias either input.
            unsafe {
                run_map(ops, &[], &sources, &destination, N, &mut scratch).unwrap();
            }
            black_box(output);
        })
    });

    group.bench_function("scalar_f64", |b| {
        let program = DenseProgram::new(ops.to_vec(), Vec::new(), 2).unwrap();
        b.iter(|| {
            let inputs = [DenseInput::F64(&a64), DenseInput::F64(&b64)];
            let mut output = vec![0.0_f64; N];
            execute_dense(&program, &inputs, DenseOutput::F64(&mut output)).unwrap();
            black_box(output);
        })
    });

    let native_f32 = f32_native_eligible(ops);
    group.bench_function("tiled_f32", |b| {
        b.iter(|| {
            let sources = [
                ColumnRef::from_f32_slice(&a32),
                ColumnRef::from_f32_slice(&b32),
            ];
            let mut output = vec![0.0_f32; N];
            let destination = ColumnMut::from_f32_slice(&mut output);
            // SAFETY: every column is contiguous and length N, and the output
            // does not alias either input.
            unsafe {
                if native_f32 {
                    run_map_f32(ops, &[], &sources, &destination, N, &mut scratch).unwrap();
                } else {
                    run_map(ops, &[], &sources, &destination, N, &mut scratch).unwrap();
                }
            }
            black_box(output);
        })
    });

    group.finish();
}

fn bench_array_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("array_chain_a_plus_b_mul_c");
    group.throughput(Throughput::Elements(N as u64));

    let core = |phase: f32| {
        let data = (0..N)
            .map(|index| (index as f32 * 0.001 + phase).sin())
            .collect();
        DenseArrayCore::from_storage(ArrayStorage::Float32(data), &[N]).unwrap()
    };
    let a = core(0.0);
    let b = core(1.0);
    let c_values = core(2.0);

    group.bench_function("eager_cloned", |bencher| {
        bencher.iter(|| {
            let product = float_elementwise(
                "multiply",
                vec![Op::PushInput(0), Op::PushInput(1), Op::Mul],
                vec![],
                vec![Operand::Array(b.clone()), Operand::Array(c_values.clone())],
            )
            .unwrap();
            let output = float_elementwise(
                "add",
                add_ops(),
                vec![],
                vec![Operand::Array(a.clone()), Operand::Array(product)],
            )
            .unwrap();
            black_box(output);
        })
    });

    group.bench_function("eager_borrowed", |bencher| {
        let multiply = [Op::PushInput(0), Op::PushInput(1), Op::Mul];
        let add = add_ops();
        bencher.iter(|| {
            let product = float_elementwise_borrowed(
                "multiply",
                &multiply,
                &[],
                &[OperandRef::Array(&b), OperandRef::Array(&c_values)],
            )
            .unwrap();
            let output = float_elementwise_borrowed(
                "add",
                &add,
                &[],
                &[OperandRef::Array(&a), OperandRef::Array(&product)],
            )
            .unwrap();
            black_box(output);
        })
    });

    group.bench_function("fused", |bencher| {
        let ops = fused_ops();
        bencher.iter(|| {
            let output = evaluate_float_expression(&ops, &[], &[&a, &b, &c_values]).unwrap();
            black_box(output);
        })
    });

    group.finish();
}

fn benches(c: &mut Criterion) {
    bench(c, "elementwise_add", &add_ops());
    bench(c, "elementwise_heavy_sqrt", &heavy_ops());
    bench_array_chain(c);
}

criterion_group!(benches_group, benches);
criterion_main!(benches_group);
