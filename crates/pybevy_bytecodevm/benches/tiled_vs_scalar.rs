//! Scalar per-entity interpreter vs. tiled (op-outer/entity-inner) executor.
//!
//! Run with: `cargo bench -p pybevy_bytecodevm --bench tiled_vs_scalar`
//! For wider SIMD: `RUSTFLAGS="-C target-cpu=native" cargo bench ...`

use std::mem::size_of;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use pybevy_bytecodevm::{
    bytecode::{CompiledBytecode, Compiler, FieldId, FieldType, Op, VM},
    tiled::{
        TiledScratch, execute_assignment_tiled, execute_assignment_tiled_f32,
        execute_assignment_tiled_parallel,
    },
};

// Particle { x: f32, v: f32 } interleaved: stride 8, x@0, v@4.
fn fx() -> FieldId {
    FieldId {
        component_id: bevy_ecs::component::ComponentId::new(0),
        offset: 0,
        field_type: FieldType::F32,
    }
}
fn fv() -> FieldId {
    FieldId {
        component_id: bevy_ecs::component::ComponentId::new(0),
        offset: 4,
        field_type: FieldType::F32,
    }
}

/// Light kernel: x = x + v * dt
fn integrate() -> CompiledBytecode {
    let mut c = Compiler::new();
    let xi = c.add_field(fx());
    let vi = c.add_field(fv());
    let dt = c.add_constant(0.016);
    c.emit(Op::PushField(xi));
    c.emit(Op::PushField(vi));
    c.emit(Op::PushConst(dt));
    c.emit(Op::Mul);
    c.emit(Op::Add);
    c.emit(Op::StoreField(xi));
    c.finalize()
}

/// Heavy kernel: x = sqrt(x*x + v*v) * 0.5 + v * dt
fn heavy() -> CompiledBytecode {
    let mut c = Compiler::new();
    let xi = c.add_field(fx());
    let vi = c.add_field(fv());
    let half = c.add_constant(0.5);
    let dt = c.add_constant(0.016);
    c.emit(Op::PushField(xi));
    c.emit(Op::PushField(xi));
    c.emit(Op::Mul); // x*x
    c.emit(Op::PushField(vi));
    c.emit(Op::PushField(vi));
    c.emit(Op::Mul); // v*v
    c.emit(Op::Add); // x*x + v*v
    c.emit(Op::Sqrt);
    c.emit(Op::PushConst(half));
    c.emit(Op::Mul); // * 0.5
    c.emit(Op::PushField(vi));
    c.emit(Op::PushConst(dt));
    c.emit(Op::Mul); // v*dt
    c.emit(Op::Add);
    c.emit(Op::StoreField(xi));
    c.finalize()
}

fn make_data(n: usize) -> Vec<f32> {
    (0..n)
        .flat_map(|i| [i as f32 * 0.5, (i as f32).sin()])
        .collect()
}

/// Scalar baseline: production per-entity VM, but with a REUSED VM (best case for
/// the scalar path — isolates the loop-flip/SIMD win, not allocation overhead).
unsafe fn run_scalar(bc: &CompiledBytecode, buf: &mut [f32], n: usize) {
    let mut vm = VM::new();
    let base = buf.as_mut_ptr() as *mut u8;
    for i in 0..n {
        let xp = unsafe { base.add(i * 8) };
        let vp = unsafe { base.add(i * 8 + 4) };
        unsafe { vm.execute(bc, &[xp, vp], i) };
    }
}

unsafe fn run_tiled(bc: &CompiledBytecode, buf: &mut [f32], n: usize, scratch: &mut TiledScratch) {
    let base = buf.as_mut_ptr() as *mut u8;
    let bases = [base, unsafe { base.add(4) }];
    let strides = [8usize, 8usize];
    unsafe { execute_assignment_tiled(bc, &bases, &strides, n, scratch).unwrap() };
}

unsafe fn run_tiled_f32(
    bc: &CompiledBytecode,
    buf: &mut [f32],
    n: usize,
    scratch: &mut TiledScratch,
) {
    let base = buf.as_mut_ptr() as *mut u8;
    let bases = [base, unsafe { base.add(4) }];
    let strides = [8usize, 8usize];
    unsafe { execute_assignment_tiled_f32(bc, &bases, &strides, n, scratch).unwrap() };
}

fn bench_kernel(c: &mut Criterion, name: &str, bc: &CompiledBytecode) {
    let mut group = c.benchmark_group(name);
    let n = 1_000_000usize;
    group.throughput(Throughput::Elements(n as u64));

    group.bench_with_input(BenchmarkId::new("scalar_per_entity", n), &n, |b, &n| {
        b.iter_batched_ref(
            || make_data(n),
            |buf| unsafe { run_scalar(black_box(bc), buf, n) },
            criterion::BatchSize::LargeInput,
        );
    });

    let mut scratch = TiledScratch::new();
    group.bench_with_input(BenchmarkId::new("tiled_f64", n), &n, |b, &n| {
        b.iter_batched_ref(
            || make_data(n),
            |buf| unsafe { run_tiled(black_box(bc), buf, n, &mut scratch) },
            criterion::BatchSize::LargeInput,
        );
    });

    let mut scratch_f32 = TiledScratch::new();
    group.bench_with_input(BenchmarkId::new("tiled_f32", n), &n, |b, &n| {
        b.iter_batched_ref(
            || make_data(n),
            |buf| unsafe { run_tiled_f32(black_box(bc), buf, n, &mut scratch_f32) },
            criterion::BatchSize::LargeInput,
        );
    });

    group.finish();
}

// Dense layout: x and v are SEPARATE contiguous f32 columns (stride 4), matching
// the numpy dense-array memory the `dense` VM operates on (packed, not interleaved).
fn make_col(n: usize, f: impl Fn(usize) -> f32) -> Vec<f32> {
    (0..n).map(f).collect()
}

unsafe fn run_scalar_dense(bc: &CompiledBytecode, x: &mut [f32], v: &[f32], n: usize) {
    let mut vm = VM::new();
    let xb = x.as_mut_ptr() as *mut u8;
    let vb = v.as_ptr() as *mut u8;
    for i in 0..n {
        let xp = unsafe { xb.add(i * 4) };
        let vp = unsafe { vb.add(i * 4) };
        unsafe { vm.execute(bc, &[xp, vp], i) };
    }
}

unsafe fn run_tiled_dense(
    bc: &CompiledBytecode,
    x: &mut [f32],
    v: &[f32],
    n: usize,
    f32_mode: bool,
    scratch: &mut TiledScratch,
) {
    let bases = [x.as_mut_ptr() as *mut u8, v.as_ptr() as *mut u8];
    let strides = [4usize, 4usize];
    if f32_mode {
        unsafe { execute_assignment_tiled_f32(bc, &bases, &strides, n, scratch).unwrap() };
    } else {
        unsafe { execute_assignment_tiled(bc, &bases, &strides, n, scratch).unwrap() };
    }
}

fn bench_kernel_dense(c: &mut Criterion, name: &str, bc: &CompiledBytecode) {
    let mut group = c.benchmark_group(name);
    let n = 1_000_000usize;
    group.throughput(Throughput::Elements(n as u64));
    let x0 = || make_col(n, |i| i as f32 * 0.5);
    let v0 = make_col(n, |i| (i as f32).sin());

    group.bench_with_input(BenchmarkId::new("scalar_per_entity", n), &n, |b, &n| {
        b.iter_batched_ref(
            x0,
            |x| unsafe { run_scalar_dense(black_box(bc), x, &v0, n) },
            criterion::BatchSize::LargeInput,
        );
    });

    let mut s64 = TiledScratch::new();
    group.bench_with_input(BenchmarkId::new("tiled_f64", n), &n, |b, &n| {
        b.iter_batched_ref(
            x0,
            |x| unsafe { run_tiled_dense(black_box(bc), x, &v0, n, false, &mut s64) },
            criterion::BatchSize::LargeInput,
        );
    });

    let mut s32 = TiledScratch::new();
    group.bench_with_input(BenchmarkId::new("tiled_f32", n), &n, |b, &n| {
        b.iter_batched_ref(
            x0,
            |x| unsafe { run_tiled_dense(black_box(bc), x, &v0, n, true, &mut s32) },
            criterion::BatchSize::LargeInput,
        );
    });

    group.bench_with_input(BenchmarkId::new("tiled_parallel_f64", n), &n, |b, &n| {
        b.iter_batched_ref(
            x0,
            |x| unsafe {
                let bases = [x.as_mut_ptr() as *mut u8, v0.as_ptr() as *mut u8];
                execute_assignment_tiled_parallel(black_box(bc), &bases, &[4, 4], n, false)
                    .unwrap();
            },
            criterion::BatchSize::LargeInput,
        );
    });

    group.bench_with_input(BenchmarkId::new("tiled_parallel_f32", n), &n, |b, &n| {
        b.iter_batched_ref(
            x0,
            |x| unsafe {
                let bases = [x.as_mut_ptr() as *mut u8, v0.as_ptr() as *mut u8];
                execute_assignment_tiled_parallel(black_box(bc), &bases, &[4, 4], n, true).unwrap();
            },
            criterion::BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn integer_add_mul() -> CompiledBytecode {
    let mut compiler = Compiler::new();
    let field = compiler.add_field(FieldId {
        component_id: bevy_ecs::component::ComponentId::new(0),
        offset: 0,
        field_type: FieldType::U64,
    });
    let three = compiler.add_constant(3.0);
    let five = compiler.add_constant(5.0);
    compiler.emit(Op::PushField(field));
    compiler.emit(Op::PushConst(three));
    compiler.emit(Op::Mul);
    compiler.emit(Op::PushConst(five));
    compiler.emit(Op::Add);
    compiler.emit(Op::StoreField(field));
    compiler.finalize()
}

fn make_u64_data(n: usize) -> Vec<u64> {
    (0..n).map(|index| index as u64 * 1_000_003).collect()
}

unsafe fn run_scalar_u64(bytecode: &CompiledBytecode, values: &mut [u64]) {
    let mut vm = VM::new();
    for (index, value) in values.iter_mut().enumerate() {
        let pointers = [(value as *mut u64).cast::<u8>()];
        unsafe { vm.execute(bytecode, &pointers, index) };
    }
}

unsafe fn run_tiled_u64(
    bytecode: &CompiledBytecode,
    values: &mut [u64],
    scratch: &mut TiledScratch,
) {
    let bases = [values.as_mut_ptr().cast::<u8>()];
    let strides = [size_of::<u64>()];
    unsafe { execute_assignment_tiled(bytecode, &bases, &strides, values.len(), scratch).unwrap() };
}

fn bench_integer_u64(c: &mut Criterion) {
    let mut group = c.benchmark_group("integer_u64_add_mul");
    let n = 1_000_000usize;
    let bytecode = integer_add_mul();
    group.throughput(Throughput::Elements(n as u64));

    group.bench_with_input(BenchmarkId::new("scalar_per_entity", n), &n, |b, &n| {
        b.iter_batched_ref(
            || make_u64_data(n),
            |values| unsafe { run_scalar_u64(black_box(&bytecode), values) },
            criterion::BatchSize::LargeInput,
        );
    });

    let mut scratch = TiledScratch::new();
    group.bench_with_input(BenchmarkId::new("tiled", n), &n, |b, &n| {
        b.iter_batched_ref(
            || make_u64_data(n),
            |values| unsafe { run_tiled_u64(black_box(&bytecode), values, &mut scratch) },
            criterion::BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_kernel(c, "interleaved_integrate_x_plus_v_dt", &integrate());
    bench_kernel(c, "interleaved_heavy_sqrt_polynomial", &heavy());
    bench_kernel_dense(c, "dense_integrate_x_plus_v_dt", &integrate());
    bench_kernel_dense(c, "dense_heavy_sqrt_polynomial", &heavy());
    bench_integer_u64(c);
}

criterion_group!(g, benches);
criterion_main!(g);
