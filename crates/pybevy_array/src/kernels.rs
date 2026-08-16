//! Interpreter-neutral numeric kernels behind the bounded array. Pure
//! computation over `DenseArrayCore` returns `KernelError`, so every adapter
//! shares one implementation. Float element-wise math routes through the safe dense VM
//! (`pybevy_bytecodevm::dense`); comparisons, `where`, reductions, and the
//! `isclose` family are computed directly over storage so integer results stay
//! exact and never round-trip through `f64`.

use pybevy_bytecodevm::{
    bytecode::{FieldType, Op},
    columns::{ColumnMut, ColumnRef},
    dense::{DenseError, DenseInput, DenseOutput, DenseProgram, StackKind, execute_dense},
    tiled::{
        ReduceOp, TiledScratch, f32_native_eligible, lane_reduce_slice, map_supported, run_map,
        run_map_f32, run_reduce,
    },
};

use crate::{
    ArrayDType, ArrayError, ArrayStorage, AxisReduce, DenseArrayCore, Layout, Scalar,
    backing::ArrayReadGuard, broadcast_shapes, broadcast_strides, checked_num_elements,
};

/// Neutral kernel failure. Each adapter maps this to its interpreter's
/// exception categories (identical mapping on both backends):
/// `Array(NotWritable)` -> ValueError, index errors -> IndexError,
/// unsupported dtype -> TypeError, expired borrow -> RuntimeError,
/// `RequiresArrayOperand` -> TypeError, `Dense` -> RuntimeError.
#[derive(Debug)]
pub enum KernelError {
    Array(ArrayError),
    Dense(DenseError),
    RequiresArrayOperand { op: &'static str },
    MixedFloatDTypes,
}

fn unsupported_dtype(op: &'static str, dtype: ArrayDType) -> KernelError {
    KernelError::Array(ArrayError::UnsupportedDType { op, dtype })
}

/// Reserve an empty result/gather buffer, failing with `AllocationFailed` for
/// unallocatable sizes.
fn try_alloc_vec<T>(dtype: ArrayDType, elements: usize) -> Result<Vec<T>, KernelError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(elements)
        .map_err(|_| KernelError::Array(ArrayError::AllocationFailed { dtype, elements }))?;
    Ok(values)
}

/// A left/right operand: a bounded array or a broadcast scalar. The scalar
/// keeps its neutral kind (so an integer scalar stays exact for comparisons and
/// integer-array assignment, not routed through `f64`).
pub enum Operand {
    Array(DenseArrayCore),
    Scalar(Scalar),
}

impl Operand {
    fn as_ref(&self) -> OperandRef<'_> {
        match self {
            Operand::Array(core) => OperandRef::Array(core),
            Operand::Scalar(value) => OperandRef::Scalar(*value),
        }
    }
}

#[derive(Clone, Copy)]
pub enum OperandRef<'a> {
    Array(&'a DenseArrayCore),
    Scalar(Scalar),
}

impl OperandRef<'_> {
    fn shape(&self) -> Option<&[usize]> {
        match self {
            OperandRef::Array(core) => Some(core.shape()),
            OperandRef::Scalar(_) => None,
        }
    }
}

fn broadcast_operands(operands: &[OperandRef<'_>]) -> Result<Vec<usize>, KernelError> {
    let mut shape: Vec<usize> = Vec::new();
    let mut seen = false;
    for op in operands {
        if let Some(s) = op.shape() {
            if !seen {
                shape = s.to_vec();
                seen = true;
            } else {
                shape = broadcast_shapes(&shape, s).map_err(KernelError::Array)?;
            }
        }
    }
    Ok(shape)
}

/// Materialize an array operand, broadcast to `target_shape`, as an `f64`
/// column for the dense VM.
fn gather_f64(core: &DenseArrayCore, target_shape: &[usize]) -> Result<Vec<f64>, KernelError> {
    let storage = core.read_storage().map_err(KernelError::Array)?;
    let n = checked_num_elements(target_shape).map_err(KernelError::Array)?;
    let strides = broadcast_strides(core.layout(), target_shape).map_err(KernelError::Array)?;
    let view = Layout {
        shape: target_shape.to_vec(),
        strides,
        offset: core.layout().offset,
    };
    let mut values = try_alloc_vec::<f64>(ArrayDType::Float64, n)?;
    values.extend(view.iter_offsets().map(|off| storage.get(off).to_f64()));
    Ok(values)
}

/// Materialize an array operand, broadcast to `target_shape`, as an `f32`
/// column while retaining native f32 rounding in the dense VM.
fn gather_f32(core: &DenseArrayCore, target_shape: &[usize]) -> Result<Vec<f32>, KernelError> {
    let storage = core.read_storage().map_err(KernelError::Array)?;
    let n = checked_num_elements(target_shape).map_err(KernelError::Array)?;
    let strides = broadcast_strides(core.layout(), target_shape).map_err(KernelError::Array)?;
    let view = Layout {
        shape: target_shape.to_vec(),
        strides,
        offset: core.layout().offset,
    };
    let mut values = try_alloc_vec::<f32>(ArrayDType::Float32, n)?;
    values.extend(
        view.iter_offsets()
            .map(|off| storage.get(off).to_f64() as f32),
    );
    Ok(values)
}

/// Broadcast an operand to `target_shape` as neutral scalars (exact for ints).
pub fn gather_scalars(op: &Operand, target_shape: &[usize]) -> Result<Vec<Scalar>, KernelError> {
    gather_scalars_borrowed(op.as_ref(), target_shape)
}

/// Broadcast a borrowed operand without cloning its array storage.
pub fn gather_scalars_borrowed(
    op: OperandRef<'_>,
    target_shape: &[usize],
) -> Result<Vec<Scalar>, KernelError> {
    match op {
        OperandRef::Scalar(v) => {
            let n = checked_num_elements(target_shape).map_err(KernelError::Array)?;
            let mut values = Vec::new();
            values.try_reserve_exact(n).map_err(|_| {
                KernelError::Array(ArrayError::AllocationFailed {
                    dtype: scalar_dtype(v),
                    elements: n,
                })
            })?;
            values.resize(n, v);
            Ok(values)
        }
        OperandRef::Array(core) => {
            let storage = core.read_storage().map_err(KernelError::Array)?;
            let n = checked_num_elements(target_shape).map_err(KernelError::Array)?;
            let strides =
                broadcast_strides(core.layout(), target_shape).map_err(KernelError::Array)?;
            let view = Layout {
                shape: target_shape.to_vec(),
                strides,
                offset: core.layout().offset,
            };
            let mut values = try_alloc_vec::<Scalar>(core.dtype(), n)?;
            values.extend(view.iter_offsets().map(|off| storage.get(off)));
            Ok(values)
        }
    }
}

/// Result dtype for a float element-wise op: `float64` if any operand is
/// float64, else `float32`. Non-float array operands raise unsupported-dtype.
fn float_result_dtype(
    op_name: &'static str,
    operands: &[OperandRef<'_>],
) -> Result<ArrayDType, KernelError> {
    let mut has_f64 = false;
    let mut has_f32 = false;
    for op in operands {
        if let OperandRef::Array(c) = op {
            match c.dtype() {
                ArrayDType::Float64 => has_f64 = true,
                ArrayDType::Float32 => has_f32 = true,
                other => return Err(unsupported_dtype(op_name, other)),
            }
        }
    }
    Ok(if has_f64 || !has_f32 {
        ArrayDType::Float64
    } else {
        ArrayDType::Float32
    })
}

/// If every operand can back a zero-copy contiguous column at `result_shape` (arrays:
/// C-contiguous, offset 0, shape already `result_shape`, F32/F64 storage; scalars:
/// broadcast), return the columns. Otherwise `None` and the caller gathers + falls back
/// to the scalar dense VM (handling broadcast and non-contiguous views).
enum LockedInput {
    Scalar(Scalar),
    Array(ArrayReadGuard),
}

impl LockedInput {
    fn column(&self, n: usize) -> Option<ColumnRef<'_>> {
        match self {
            LockedInput::Scalar(value) => Some(ColumnRef::broadcast(value.to_f64())),
            LockedInput::Array(storage) => match &**storage {
                ArrayStorage::Float64(values) => values.get(..n).map(ColumnRef::from_f64_slice),
                _ => {
                    // SAFETY: `ArrayReadGuard` checked borrowed validity and
                    // remains alive for the returned column's whole use.
                    unsafe { storage.as_f32_contiguous_unchecked() }
                        .and_then(|values| values.get(..n))
                        .map(ColumnRef::from_f32_slice)
                }
            },
        }
    }
}

fn lock_contiguous_inputs(
    operands: &[OperandRef<'_>],
    result_shape: &[usize],
    n: usize,
) -> Option<Vec<LockedInput>> {
    operands
        .iter()
        .map(|op| match op {
            OperandRef::Scalar(value) => Some(LockedInput::Scalar(*value)),
            OperandRef::Array(core) => {
                if core.layout().offset != 0
                    || !core.is_c_contiguous()
                    || core.shape() != result_shape
                {
                    return None;
                }
                let storage = core.read_storage().ok()?;
                let has_column = match &*storage {
                    ArrayStorage::Float64(values) => values.len() >= n,
                    _ => {
                        // SAFETY: the guard checked borrowed validity and no
                        // reference escapes this length probe.
                        unsafe { storage.as_f32_contiguous_unchecked() }
                            .is_some_and(|values| values.len() >= n)
                    }
                };
                has_column.then_some(LockedInput::Array(storage))
            }
        })
        .collect()
}

/// Lock one zero-copy contiguous floating-point input for a whole-array
/// reduction. Non-contiguous, offset, expired, and non-floating arrays fall
/// back to logical row-major materialization.
fn lock_contiguous_input(core: &DenseArrayCore, len: usize) -> Option<LockedInput> {
    if core.layout().offset != 0 || !core.is_c_contiguous() {
        return None;
    }
    let storage = core.read_storage().ok()?;
    let has_column = match &*storage {
        ArrayStorage::Float64(values) => values.len() >= len,
        _ => {
            // SAFETY: the read guard checked validity and remains live.
            unsafe { storage.as_f32_contiguous_unchecked() }
                .is_some_and(|values| values.len() >= len)
        }
    };
    has_column.then_some(LockedInput::Array(storage))
}

/// Whole-array float reduction with identical ordering for contiguous and
/// materialized inputs.
fn reduce_float(core: &DenseArrayCore, op: ReduceOp) -> Result<f64, KernelError> {
    let len = core.size();
    if let Some(input) = lock_contiguous_input(core, len) {
        let source = input
            .column(len)
            .expect("locked contiguous input exposes its validated column");
        let mut scratch = TiledScratch::new();
        // SAFETY: the locked input exposes a source valid over `0..len`.
        Ok(unsafe { run_reduce(op, &source, len, &mut scratch) })
    } else {
        let values: Vec<f64> = core
            .to_scalars()
            .map_err(KernelError::Array)?
            .iter()
            .map(|value| value.to_f64())
            .collect();
        Ok(lane_reduce_slice(op, &values))
    }
}

/// Allocate an uninitialized f64 output and let `fill` initialize every element.
///
/// # Safety
/// `fill` must write every element in `0..n` without reading the destination and
/// must return only after all elements are initialized.
unsafe fn build_output_f64(
    n: usize,
    fill: impl FnOnce(&ColumnMut<'_>),
) -> Result<Vec<f64>, KernelError> {
    let mut output = try_alloc_vec::<f64>(ArrayDType::Float64, n)?;
    // SAFETY: the allocation has capacity for `n` writable f64 locations. The
    // caller guarantees that `fill` writes all of them before they become visible.
    let destination = unsafe {
        ColumnMut::strided_1d(
            output.as_mut_ptr().cast(),
            size_of::<f64>() as isize,
            FieldType::F64,
            n,
        )
    };
    fill(&destination);
    // SAFETY: the caller guarantees that a returning `fill` initialized all `n`
    // elements. If `fill` panics, this line is skipped and the length remains zero.
    unsafe { output.set_len(n) };
    Ok(output)
}

/// f32 counterpart of [`build_output_f64`].
///
/// # Safety
/// `fill` must write every element in `0..n` without reading the destination and
/// must return only after all elements are initialized.
unsafe fn build_output_f32(
    n: usize,
    fill: impl FnOnce(&ColumnMut<'_>),
) -> Result<Vec<f32>, KernelError> {
    let mut output = try_alloc_vec::<f32>(ArrayDType::Float32, n)?;
    // SAFETY: the allocation has capacity for `n` writable f32 locations. The
    // caller guarantees that `fill` writes all of them before they become visible.
    let destination = unsafe {
        ColumnMut::strided_1d(
            output.as_mut_ptr().cast(),
            size_of::<f32>() as isize,
            FieldType::F32,
            n,
        )
    };
    fill(&destination);
    // SAFETY: the caller guarantees that a returning `fill` initialized all `n`
    // elements. If `fill` panics, this line is skipped and the length remains zero.
    unsafe { output.set_len(n) };
    Ok(output)
}

/// Run a float-producing dense program over `operands`, returning a new array.
pub fn float_elementwise(
    op_name: &'static str,
    ops: Vec<Op>,
    constants: Vec<f64>,
    operands: Vec<Operand>,
) -> Result<DenseArrayCore, KernelError> {
    let borrowed: Vec<OperandRef<'_>> = operands.iter().map(Operand::as_ref).collect();
    float_elementwise_borrowed(op_name, &ops, &constants, &borrowed)
}

/// Run an element-wise program while borrowing every array operand.
pub fn float_elementwise_borrowed(
    op_name: &'static str,
    ops: &[Op],
    constants: &[f64],
    operands: &[OperandRef<'_>],
) -> Result<DenseArrayCore, KernelError> {
    float_elementwise_impl(op_name, ops, constants, operands)
}

fn float_elementwise_impl(
    op_name: &'static str,
    ops: &[Op],
    constants: &[f64],
    operands: &[OperandRef<'_>],
) -> Result<DenseArrayCore, KernelError> {
    if !operands
        .iter()
        .any(|operand| matches!(operand, OperandRef::Array(_)))
    {
        return Err(KernelError::RequiresArrayOperand { op: op_name });
    }
    let result_shape = broadcast_operands(operands)?;
    let out_dtype = float_result_dtype(op_name, operands)?;
    let n = checked_num_elements(&result_shape).map_err(KernelError::Array)?;
    // Programs with out-of-range indices skip the tiled fast paths so the
    // dense validator reports them instead of the executors panicking.
    let indices_in_range = ops.iter().all(|op| match op {
        Op::PushInput(index) => usize::from(*index) < operands.len(),
        Op::PushConst(index) => usize::from(*index) < constants.len(),
        _ => true,
    });

    // Native f32 programs round every intermediate in the f32 domain.
    if out_dtype == ArrayDType::Float32
        && indices_in_range
        && f32_native_eligible(ops)
        && let Some(inputs) = lock_contiguous_inputs(operands, &result_shape, n)
    {
        let srcs: Vec<ColumnRef<'_>> = inputs
            .iter()
            .map(|input| {
                input
                    .column(n)
                    .expect("locked contiguous input exposes its validated column")
            })
            .collect();
        let mut scratch = TiledScratch::new();
        let fill = |destination: &ColumnMut<'_>| {
            // SAFETY: sources are valid length-n columns and the destination is
            // fresh, disjoint, writable storage of the matching field type.
            unsafe { run_map_f32(ops, constants, &srcs, destination, n, &mut scratch) }
                .expect("native f32 preconditions checked");
        };
        // SAFETY: `run_map_f32` writes every destination row before returning and
        // never reads the fresh destination allocation.
        let out = unsafe { build_output_f32(n, fill) }?;
        return DenseArrayCore::from_storage(ArrayStorage::Float32(out), &result_shape)
            .map_err(KernelError::Array);
    }

    // Vectorizable arithmetic over directly usable contiguous columns. Broadcast,
    // non-contiguous, and unsupported programs use the scalar dense path.
    if indices_in_range
        && map_supported(ops)
        && let Some(inputs) = lock_contiguous_inputs(operands, &result_shape, n)
    {
        let srcs: Vec<ColumnRef<'_>> = inputs
            .iter()
            .map(|input| {
                input
                    .column(n)
                    .expect("locked contiguous input exposes its validated column")
            })
            .collect();
        let mut scratch = TiledScratch::new();
        let storage = match out_dtype {
            ArrayDType::Float64 => {
                let fill = |destination: &ColumnMut<'_>| {
                    // SAFETY: sources are valid length-n columns and destination
                    // is fresh, disjoint writable f64 storage.
                    unsafe { run_map(ops, constants, &srcs, destination, n, &mut scratch) }
                        .expect("map_supported ensured a tiled-runnable program");
                };
                // SAFETY: `run_map` writes every destination row before returning
                // and never reads the fresh destination allocation.
                let out = unsafe { build_output_f64(n, fill) }?;
                ArrayStorage::Float64(out)
            }
            ArrayDType::Float32 => {
                let fill = |destination: &ColumnMut<'_>| {
                    // SAFETY: sources are valid length-n columns and destination
                    // is fresh, disjoint writable f32 storage.
                    unsafe { run_map(ops, constants, &srcs, destination, n, &mut scratch) }
                        .expect("map_supported ensured a tiled-runnable program");
                };
                // SAFETY: `run_map` writes every destination row before returning
                // and never reads the fresh destination allocation.
                let out = unsafe { build_output_f32(n, fill) }?;
                ArrayStorage::Float32(out)
            }
            _ => unreachable!("float_result_dtype only yields float dtypes"),
        };
        return DenseArrayCore::from_storage(storage, &result_shape).map_err(KernelError::Array);
    }

    if out_dtype == ArrayDType::Float32 {
        let columns: Vec<Option<Vec<f32>>> = operands
            .iter()
            .map(|op| match op {
                OperandRef::Array(c) => gather_f32(c, &result_shape).map(Some),
                OperandRef::Scalar(_) => Ok(None),
            })
            .collect::<Result<_, KernelError>>()?;
        let inputs: Vec<DenseInput<'_>> = operands
            .iter()
            .enumerate()
            .map(|(i, op)| match op {
                OperandRef::Array(_) => DenseInput::F32(columns[i].as_ref().unwrap()),
                OperandRef::Scalar(v) => DenseInput::Scalar(v.to_f64()),
            })
            .collect();
        let program = DenseProgram::new(ops.to_vec(), constants.to_vec(), operands.len())
            .map_err(KernelError::Dense)?;
        let mut out = try_alloc_vec::<f32>(ArrayDType::Float32, n)?;
        out.resize(n, 0.0);
        execute_dense(&program, &inputs, DenseOutput::F32(&mut out)).map_err(KernelError::Dense)?;
        return DenseArrayCore::from_storage(ArrayStorage::Float32(out), &result_shape)
            .map_err(KernelError::Array);
    }

    let columns: Vec<Option<Vec<f64>>> = operands
        .iter()
        .map(|op| match op {
            OperandRef::Array(c) => gather_f64(c, &result_shape).map(Some),
            OperandRef::Scalar(_) => Ok(None),
        })
        .collect::<Result<_, KernelError>>()?;
    let inputs: Vec<DenseInput<'_>> = operands
        .iter()
        .enumerate()
        .map(|(i, op)| match op {
            OperandRef::Array(_) => DenseInput::F64(columns[i].as_ref().unwrap()),
            OperandRef::Scalar(v) => DenseInput::Scalar(v.to_f64()),
        })
        .collect();

    let program = DenseProgram::new(ops.to_vec(), constants.to_vec(), operands.len())
        .map_err(KernelError::Dense)?;
    let mut out = try_alloc_vec::<f64>(ArrayDType::Float64, n)?;
    out.resize(n, 0.0);
    execute_dense(&program, &inputs, DenseOutput::F64(&mut out)).map_err(KernelError::Dense)?;
    let storage = ArrayStorage::Float64(out);
    DenseArrayCore::from_storage(storage, &result_shape).map_err(KernelError::Array)
}

/// Evaluate one fused expression over borrowed float arrays without copying its
/// input storage. All array leaves must use the same floating-point dtype.
pub fn evaluate_float_expression(
    ops: &[Op],
    constants: &[f64],
    arrays: &[&DenseArrayCore],
) -> Result<DenseArrayCore, KernelError> {
    let Some(first) = arrays.first() else {
        return Err(KernelError::RequiresArrayOperand { op: "evaluate" });
    };
    let dtype = first.dtype();
    if !matches!(dtype, ArrayDType::Float32 | ArrayDType::Float64) {
        return Err(unsupported_dtype("evaluate", dtype));
    }
    for array in &arrays[1..] {
        if !matches!(array.dtype(), ArrayDType::Float32 | ArrayDType::Float64) {
            return Err(unsupported_dtype("evaluate", array.dtype()));
        }
        if array.dtype() != dtype {
            return Err(KernelError::MixedFloatDTypes);
        }
    }

    let operands: Vec<OperandRef<'_>> = arrays
        .iter()
        .map(|array| OperandRef::Array(array))
        .collect();
    let program = DenseProgram::new(ops.to_vec(), constants.to_vec(), arrays.len())
        .map_err(KernelError::Dense)?;
    if program.result_kind() == StackKind::Float {
        return float_elementwise_impl("evaluate", ops, constants, &operands);
    }

    let result_shape = broadcast_operands(&operands)?;
    let n = checked_num_elements(&result_shape).map_err(KernelError::Array)?;
    let mut output = try_alloc_vec::<bool>(ArrayDType::Bool, n)?;
    output.resize(n, false);
    match dtype {
        ArrayDType::Float32 => {
            let columns: Vec<Vec<f32>> = arrays
                .iter()
                .map(|array| gather_f32(array, &result_shape))
                .collect::<Result<_, _>>()?;
            let inputs: Vec<DenseInput<'_>> = columns
                .iter()
                .map(|column| DenseInput::F32(column))
                .collect();
            execute_dense(&program, &inputs, DenseOutput::Bool(&mut output))
                .map_err(KernelError::Dense)?;
        }
        ArrayDType::Float64 => {
            let columns: Vec<Vec<f64>> = arrays
                .iter()
                .map(|array| gather_f64(array, &result_shape))
                .collect::<Result<_, _>>()?;
            let inputs: Vec<DenseInput<'_>> = columns
                .iter()
                .map(|column| DenseInput::F64(column))
                .collect();
            execute_dense(&program, &inputs, DenseOutput::Bool(&mut output))
                .map_err(KernelError::Dense)?;
        }
        _ => unreachable!("dtype validated as float"),
    }
    DenseArrayCore::from_storage(ArrayStorage::Bool(output), &result_shape)
        .map_err(KernelError::Array)
}

/// A comparison kind, mapping to exact per-dtype semantics.
#[derive(Clone, Copy)]
pub enum CmpKind {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

fn cmp_scalars(a: Scalar, b: Scalar, kind: CmpKind) -> bool {
    let both_integral = matches!(a, Scalar::I64(_) | Scalar::Bool(_))
        && matches!(b, Scalar::I64(_) | Scalar::Bool(_));
    if both_integral {
        let (x, y) = (a.to_i64_trunc(), b.to_i64_trunc());
        match kind {
            CmpKind::Eq => x == y,
            CmpKind::Ne => x != y,
            CmpKind::Lt => x < y,
            CmpKind::Le => x <= y,
            CmpKind::Gt => x > y,
            CmpKind::Ge => x >= y,
        }
    } else {
        let (x, y) = (a.to_f64(), b.to_f64());
        match kind {
            CmpKind::Eq => x == y,
            CmpKind::Ne => x != y,
            CmpKind::Lt => x < y,
            CmpKind::Le => x <= y,
            CmpKind::Gt => x > y,
            CmpKind::Ge => x >= y,
        }
    }
}

/// Element-wise comparison producing a bool array. Exact for every supported
/// dtype (integers never route through `f64`).
pub fn compare(a: Operand, b: Operand, kind: CmpKind) -> Result<DenseArrayCore, KernelError> {
    compare_borrowed(a.as_ref(), b.as_ref(), kind)
}

pub fn compare_borrowed(
    a: OperandRef<'_>,
    b: OperandRef<'_>,
    kind: CmpKind,
) -> Result<DenseArrayCore, KernelError> {
    let result_shape = broadcast_pair(a, b)?;
    let av = gather_scalars_borrowed(a, &result_shape)?;
    let bv = gather_scalars_borrowed(b, &result_shape)?;
    let out: Vec<bool> = av
        .iter()
        .zip(bv.iter())
        .map(|(&x, &y)| cmp_scalars(x, y, kind))
        .collect();
    DenseArrayCore::from_storage(ArrayStorage::Bool(out), &result_shape).map_err(KernelError::Array)
}

fn broadcast_pair(a: OperandRef<'_>, b: OperandRef<'_>) -> Result<Vec<usize>, KernelError> {
    match (a.shape(), b.shape()) {
        (Some(x), Some(y)) => broadcast_shapes(x, y).map_err(KernelError::Array),
        (Some(x), None) => Ok(x.to_vec()),
        (None, Some(y)) => Ok(y.to_vec()),
        (None, None) => Ok(Vec::new()),
    }
}

/// `where(condition, a, b)`: computed directly so any dtype selects exactly.
pub fn where_select(cond: Operand, a: Operand, b: Operand) -> Result<DenseArrayCore, KernelError> {
    where_select_borrowed(cond.as_ref(), a.as_ref(), b.as_ref())
}

pub fn where_select_borrowed(
    cond: OperandRef<'_>,
    a: OperandRef<'_>,
    b: OperandRef<'_>,
) -> Result<DenseArrayCore, KernelError> {
    let mut shape = broadcast_pair(cond, a)?;
    if let Some(bs) = b.shape() {
        shape = broadcast_shapes(&shape, bs).map_err(KernelError::Array)?;
    }
    let cv = gather_scalars_borrowed(cond, &shape)?;
    let av = gather_scalars_borrowed(a, &shape)?;
    let bv = gather_scalars_borrowed(b, &shape)?;
    let out_dtype = promote_select(&a, &b);
    let n = checked_num_elements(&shape).map_err(KernelError::Array)?;
    let mut storage = ArrayStorage::zeros(out_dtype, n).map_err(KernelError::Array)?;
    for i in 0..cv.len() {
        let picked = if cv[i].to_bool() { av[i] } else { bv[i] };
        storage.set(i, picked);
    }
    DenseArrayCore::from_storage(storage, &shape).map_err(KernelError::Array)
}

fn operand_dtype(op: &OperandRef<'_>) -> ArrayDType {
    match op {
        OperandRef::Array(c) => c.dtype(),
        OperandRef::Scalar(value) => scalar_dtype(*value),
    }
}

fn scalar_dtype(value: Scalar) -> ArrayDType {
    match value {
        Scalar::F64(_) => ArrayDType::Float64,
        Scalar::I64(_) => ArrayDType::Int64,
        Scalar::Bool(_) => ArrayDType::Bool,
    }
}

fn promote_select(a: &OperandRef<'_>, b: &OperandRef<'_>) -> ArrayDType {
    match (operand_dtype(a), operand_dtype(b)) {
        (x, y) if x == y => x,
        (x, y) => {
            let dt = [x, y];
            if dt.contains(&ArrayDType::Float64) {
                ArrayDType::Float64
            } else if dt.contains(&ArrayDType::Float32) {
                ArrayDType::Float32
            } else {
                ArrayDType::Int64
            }
        }
    }
}

fn isclose_pair(a: f64, b: f64) -> bool {
    // NumPy defaults: rtol=1e-5, atol=1e-8; NaN != NaN unless equal_nan.
    if a.is_nan() || b.is_nan() {
        return false;
    }
    if a.is_infinite() || b.is_infinite() {
        return a == b;
    }
    (a - b).abs() <= 1e-8 + 1e-5 * b.abs()
}

pub fn isfinite(a: &DenseArrayCore) -> Result<DenseArrayCore, KernelError> {
    let out: Vec<bool> = a
        .to_scalars()
        .map_err(KernelError::Array)?
        .iter()
        .map(|s| s.to_f64().is_finite())
        .collect();
    DenseArrayCore::from_storage(ArrayStorage::Bool(out), a.shape()).map_err(KernelError::Array)
}

pub fn isclose(a: Operand, b: Operand) -> Result<DenseArrayCore, KernelError> {
    isclose_borrowed(a.as_ref(), b.as_ref())
}

pub fn isclose_borrowed(
    a: OperandRef<'_>,
    b: OperandRef<'_>,
) -> Result<DenseArrayCore, KernelError> {
    let shape = broadcast_pair(a, b)?;
    let av = gather_scalars_borrowed(a, &shape)?;
    let bv = gather_scalars_borrowed(b, &shape)?;
    let out: Vec<bool> = av
        .iter()
        .zip(bv.iter())
        .map(|(&x, &y)| isclose_pair(x.to_f64(), y.to_f64()))
        .collect();
    DenseArrayCore::from_storage(ArrayStorage::Bool(out), &shape).map_err(KernelError::Array)
}

pub fn allclose(a: Operand, b: Operand) -> Result<bool, KernelError> {
    allclose_borrowed(a.as_ref(), b.as_ref())
}

pub fn allclose_borrowed(a: OperandRef<'_>, b: OperandRef<'_>) -> Result<bool, KernelError> {
    let shape = broadcast_pair(a, b)?;
    let av = gather_scalars_borrowed(a, &shape)?;
    let bv = gather_scalars_borrowed(b, &shape)?;
    Ok(av
        .iter()
        .zip(bv.iter())
        .all(|(&x, &y)| isclose_pair(x.to_f64(), y.to_f64())))
}

pub fn array_equal(a: &DenseArrayCore, b: &DenseArrayCore) -> Result<bool, KernelError> {
    if a.shape() != b.shape() {
        return Ok(false);
    }
    let av = a.to_scalars().map_err(KernelError::Array)?;
    let bv = b.to_scalars().map_err(KernelError::Array)?;
    Ok(av
        .iter()
        .zip(bv.iter())
        .all(|(&x, &y)| cmp_scalars(x, y, CmpKind::Eq)))
}

/// Complete-array reductions.
pub enum Reduce {
    Sum,
    Mean,
    Min,
    Max,
    All,
    Any,
}

/// Neutral result of a reduction: a scalar (whole-array) or an array (axis).
pub enum Reduced {
    Scalar(Scalar),
    Array(DenseArrayCore),
}

fn to_axis_reduce(kind: &Reduce) -> AxisReduce {
    match kind {
        Reduce::Sum => AxisReduce::Sum,
        Reduce::Mean => AxisReduce::Mean,
        Reduce::Min => AxisReduce::Min,
        Reduce::Max => AxisReduce::Max,
        Reduce::All => AxisReduce::All,
        Reduce::Any => AxisReduce::Any,
    }
}

/// Reduce over the whole array (returning a scalar) or along `axis` (returning
/// a new array).
pub fn reduce(
    core: &DenseArrayCore,
    kind: Reduce,
    axis: Option<usize>,
) -> Result<Reduced, KernelError> {
    if let Some(ax) = axis {
        if matches!(kind, Reduce::Sum | Reduce::Mean) && !core.dtype().is_float() {
            let name = if matches!(kind, Reduce::Sum) {
                "sum"
            } else {
                "mean"
            };
            return Err(unsupported_dtype(name, core.dtype()));
        }
        let reduced = core
            .reduce_axis(ax, to_axis_reduce(&kind))
            .map_err(KernelError::Array)?;
        return Ok(Reduced::Array(reduced));
    }
    match kind {
        Reduce::All => {
            let scalars = core.to_scalars().map_err(KernelError::Array)?;
            return Ok(Reduced::Scalar(Scalar::Bool(
                scalars.iter().all(|s| s.to_bool()),
            )));
        }
        Reduce::Any => {
            let scalars = core.to_scalars().map_err(KernelError::Array)?;
            return Ok(Reduced::Scalar(Scalar::Bool(
                scalars.iter().any(|s| s.to_bool()),
            )));
        }
        _ => {}
    }
    match kind {
        Reduce::Sum | Reduce::Mean => {
            if !core.dtype().is_float() {
                return Err(unsupported_dtype(
                    if matches!(kind, Reduce::Sum) {
                        "sum"
                    } else {
                        "mean"
                    },
                    core.dtype(),
                ));
            }
            // Empty sum is 0.0 and empty mean is NaN (0/0).
            let total = reduce_float(core, ReduceOp::Sum)?;
            let value = if matches!(kind, Reduce::Mean) {
                total / core.size() as f64
            } else {
                total
            };
            Ok(Reduced::Scalar(Scalar::F64(value)))
        }
        Reduce::Min | Reduce::Max => {
            if core.size() == 0 {
                return Err(KernelError::Array(ArrayError::ZeroSizeReduction));
            }

            let op = if matches!(kind, Reduce::Max) {
                ReduceOp::Max
            } else {
                ReduceOp::Min
            };
            if core.dtype().is_float() {
                Ok(Reduced::Scalar(Scalar::F64(reduce_float(core, op)?)))
            } else {
                // Integer and boolean reductions remain exact instead of widening
                // through the floating-point executor.
                let scalars = core.to_scalars().map_err(KernelError::Array)?;
                let want_max = matches!(kind, Reduce::Max);
                let mut best = scalars[0];
                for &value in &scalars[1..] {
                    let take = if want_max {
                        cmp_scalars(value, best, CmpKind::Gt)
                    } else {
                        cmp_scalars(value, best, CmpKind::Lt)
                    };
                    if take {
                        best = value;
                    }
                }
                Ok(Reduced::Scalar(best))
            }
        }
        Reduce::All | Reduce::Any => unreachable!("handled above"),
    }
}

/// Wrap owned `float32` data as a read-only bounded core. Shared owned-copy
/// path used by both backends' read snapshots.
pub fn read_only_f32_core(data: Vec<f32>, shape: &[usize]) -> Result<DenseArrayCore, KernelError> {
    let mut core = DenseArrayCore::from_storage(ArrayStorage::Float32(data), shape)
        .map_err(KernelError::Array)?;
    core.set_read_only();
    Ok(core)
}
