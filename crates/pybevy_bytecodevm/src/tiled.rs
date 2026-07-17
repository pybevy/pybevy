//! Tiled (op-outer, entity-inner) executor for SIMD-friendly evaluation.
//!
//! Variants: serial f64 (bit-identical to the scalar VM, enforced by the
//! `differential_*` fuzz tests below), serial f32 (lower precision, ~2x lanes), and
//! rayon-parallel versions of each (SIMD x threads). Programs with non-arithmetic ops
//! return [`UnsupportedOp`] so callers can fall back.

use crate::{
    bytecode::{
        CompiledBytecode, FieldType, Op, python_clip, python_maximum, python_minimum,
        python_remainder, python_round, python_sign,
    },
    columns::{ColumnMut, ColumnRef, load_tile_f32, load_tile_f64, store_tile_f32, store_tile_f64},
};

/// Entities per tile. 256 keeps the stack-of-tiles in L1 while giving long runs.
pub const TILE: usize = 256;
const MAX_DEPTH: usize = 32;
/// Entities per rayon task in the parallel variants.
#[cfg(feature = "parallel")]
const PCHUNK: usize = 16384;

/// f32 element-wise minimum that propagates NaN (returning the NaN operand), matching
/// `python_minimum`. Rust's `f32::min` returns the non-NaN operand, which diverges from
/// the scalar VM and NumPy.
#[inline]
fn min_f32_nan(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else {
        a.min(b)
    }
}

#[inline]
fn max_f32_nan(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else {
        a.max(b)
    }
}

#[inline]
fn sign_f32(value: f32) -> f32 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        value
    }
}

#[inline]
fn remainder_f32(dividend: f32, divisor: f32) -> f32 {
    let remainder = dividend % divisor;
    if remainder == 0.0 {
        return remainder.copysign(divisor);
    }
    if (remainder < 0.0) != (divisor < 0.0) {
        remainder + divisor
    } else {
        remainder
    }
}

#[inline]
fn clip_f32(value: f32, min: f32, max: f32) -> f32 {
    min_f32_nan(max_f32_nan(value, min), max)
}

/// Returned when a program contains an operation the tiled executor does not support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsupportedOp;

/// Reusable stack-of-tiles scratch. Allocate once, reuse across calls/tables.
pub struct TiledScratch {
    stack: Vec<f64>,
    stack_f32: Vec<f32>,
    stack_i32: Vec<i32>,
    stack_i64: Vec<i64>,
    stack_u8: Vec<u8>,
    stack_u32: Vec<u32>,
    stack_u64: Vec<u64>,
}

impl Default for TiledScratch {
    fn default() -> Self {
        Self::new()
    }
}

impl TiledScratch {
    pub fn new() -> Self {
        Self {
            stack: vec![0.0; MAX_DEPTH * TILE],
            stack_f32: vec![0.0; MAX_DEPTH * TILE],
            stack_i32: Vec::new(),
            stack_i64: Vec::new(),
            stack_u8: Vec::new(),
            stack_u32: Vec::new(),
            stack_u64: Vec::new(),
        }
    }

    #[inline]
    fn ensure_depth(&mut self, depth: usize) {
        let required = depth * TILE;
        if self.stack.len() < required {
            self.stack.resize(required, 0.0);
        }
        if self.stack_f32.len() < required {
            self.stack_f32.resize(required, 0.0);
        }
    }
}

#[inline]
fn all_fields_float(bytecode: &CompiledBytecode) -> bool {
    bytecode.bytecode.iter().all(|op| match op {
        Op::PushField(i) | Op::StoreField(i) => bytecode
            .field_map
            .get(*i as usize)
            .is_some_and(|field| matches!(field.field_type, FieldType::F32 | FieldType::F64)),
        Op::PushInput(_) => false,
        _ => true,
    })
}

trait IntegerLane: Copy + Default + Ord {
    const FIELD_TYPE: FieldType;

    fn from_constant(value: f64) -> Option<Self>;
    fn wrapping_add(self, other: Self) -> Self;
    fn wrapping_sub(self, other: Self) -> Self;
    fn wrapping_mul(self, other: Self) -> Self;
    fn wrapping_neg(self) -> Self;
    fn wrapping_abs(self) -> Self;
    fn sign(self) -> Self;
}

macro_rules! impl_integer_lane {
    ($ty:ty, $field_type:expr, $from_constant:expr, $abs:expr, $sign:expr) => {
        impl IntegerLane for $ty {
            const FIELD_TYPE: FieldType = $field_type;

            #[inline]
            fn from_constant(value: f64) -> Option<Self> {
                ($from_constant)(value)
            }

            #[inline]
            fn wrapping_add(self, other: Self) -> Self {
                self.wrapping_add(other)
            }

            #[inline]
            fn wrapping_sub(self, other: Self) -> Self {
                self.wrapping_sub(other)
            }

            #[inline]
            fn wrapping_mul(self, other: Self) -> Self {
                self.wrapping_mul(other)
            }

            #[inline]
            fn wrapping_neg(self) -> Self {
                self.wrapping_neg()
            }

            #[inline]
            fn wrapping_abs(self) -> Self {
                ($abs)(self)
            }

            #[inline]
            fn sign(self) -> Self {
                ($sign)(self)
            }
        }
    };
}

impl_integer_lane!(
    i32,
    FieldType::I32,
    |v: f64| (v.is_finite()
        && v.fract() == 0.0
        && v >= f64::from(i32::MIN)
        && v <= f64::from(i32::MAX))
    .then_some(v as i32),
    i32::wrapping_abs,
    i32::signum
);
impl_integer_lane!(
    i64,
    FieldType::I64,
    |v: f64| (v.is_finite() && v.fract() == 0.0 && v >= i64::MIN as f64 && v < -(i64::MIN as f64))
        .then_some(v as i64),
    i64::wrapping_abs,
    i64::signum
);
impl_integer_lane!(
    u8,
    FieldType::U8,
    |v: f64| (v.is_finite() && v.fract() == 0.0 && v >= 0.0 && v <= f64::from(u8::MAX))
        .then_some(v as u8),
    |v| v,
    |v| u8::from(v != 0)
);
impl_integer_lane!(
    u32,
    FieldType::U32,
    |v: f64| (v.is_finite() && v.fract() == 0.0 && v >= 0.0 && v <= f64::from(u32::MAX))
        .then_some(v as u32),
    |v| v,
    |v| u32::from(v != 0)
);
impl_integer_lane!(
    u64,
    FieldType::U64,
    |v: f64| (v.is_finite() && v.fract() == 0.0 && v >= 0.0 && v < 2_f64.powi(64))
        .then_some(v as u64),
    |v| v,
    |v| u64::from(v != 0)
);

#[inline]
fn integer_program_type(bytecode: &CompiledBytecode) -> Option<FieldType> {
    let mut lane_type = None;
    for op in &bytecode.bytecode {
        match op {
            Op::PushField(index) | Op::StoreField(index) => {
                let field_type = bytecode.field_map.get(*index as usize)?.field_type;
                if !matches!(
                    field_type,
                    FieldType::I32
                        | FieldType::I64
                        | FieldType::U8
                        | FieldType::U32
                        | FieldType::U64
                ) || lane_type.is_some_and(|current| current != field_type)
                {
                    return None;
                }
                lane_type = Some(field_type);
            }
            Op::PushConst(index) => {
                let value = *bytecode.constants.get(*index as usize)?;
                let representable = match lane_type {
                    Some(FieldType::I32) => i32::from_constant(value).is_some(),
                    Some(FieldType::I64) => i64::from_constant(value).is_some(),
                    Some(FieldType::U8) => u8::from_constant(value).is_some(),
                    Some(FieldType::U32) => u32::from_constant(value).is_some(),
                    Some(FieldType::U64) => u64::from_constant(value).is_some(),
                    Some(_) | None => true,
                };
                if !representable {
                    return None;
                }
            }
            Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Neg
            | Op::Abs
            | Op::Min
            | Op::Max
            | Op::Clamp
            | Op::Floor
            | Op::Ceil
            | Op::Round
            | Op::Sign => {}
            _ => return None,
        }
    }
    let lane_type = lane_type?;
    let constants_valid = bytecode.bytecode.iter().all(|op| match op {
        Op::PushConst(index) => bytecode
            .constants
            .get(*index as usize)
            .is_some_and(|value| match lane_type {
                FieldType::I32 => i32::from_constant(*value).is_some(),
                FieldType::I64 => i64::from_constant(*value).is_some(),
                FieldType::U8 => u8::from_constant(*value).is_some(),
                FieldType::U32 => u32::from_constant(*value).is_some(),
                FieldType::U64 => u64::from_constant(*value).is_some(),
                _ => false,
            }),
        _ => true,
    });
    constants_valid.then_some(lane_type)
}

/// Return `(maximum_depth, final_depth)` for a structurally valid program.
/// Malformed bytecode is rejected before any tiled stack indexing occurs.
fn stack_layout(ops: &[Op]) -> Option<(usize, usize)> {
    let mut depth = 0usize;
    let mut maximum = 0usize;
    for op in ops {
        let (pops, pushes) = match op {
            Op::PushField(_) | Op::PushInput(_) | Op::PushConst(_) | Op::Random => (0, 1),
            Op::StoreField(_)
            | Op::Neg
            | Op::Sin
            | Op::Cos
            | Op::Tan
            | Op::Asin
            | Op::Acos
            | Op::Atan
            | Op::Sqrt
            | Op::Abs
            | Op::Floor
            | Op::Ceil
            | Op::Round
            | Op::Not
            | Op::Exp
            | Op::Ln
            | Op::Log10
            | Op::Log2
            | Op::Sign
            | Op::Fract => (1, usize::from(!matches!(op, Op::StoreField(_)))),
            Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Pow
            | Op::Min
            | Op::Max
            | Op::Eq
            | Op::Ne
            | Op::Lt
            | Op::Le
            | Op::Gt
            | Op::Ge
            | Op::And
            | Op::Or
            | Op::Mod
            | Op::RandomRange => (2, 1),
            Op::Clamp | Op::Where | Op::Lerp => (3, 1),
        };
        if depth < pops {
            return None;
        }
        depth = depth - pops + pushes;
        maximum = maximum.max(depth);
    }
    Some((maximum, depth))
}

#[inline]
fn is_supported(op: &Op) -> bool {
    matches!(
        op,
        Op::PushField(_)
            | Op::PushInput(_)
            | Op::PushConst(_)
            | Op::StoreField(_)
            | Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Neg
            | Op::Sqrt
            | Op::Abs
            | Op::Min
            | Op::Max
            | Op::Floor
            | Op::Ceil
            | Op::Round
            | Op::Sign
            | Op::Fract
            | Op::Clamp
            | Op::Lerp
            | Op::Sin
            | Op::Cos
            | Op::Tan
            | Op::Asin
            | Op::Acos
            | Op::Atan
            | Op::Exp
            | Op::Ln
            | Op::Log10
            | Op::Log2
            | Op::Pow
            | Op::Mod
    )
}

#[inline]
fn all_supported(bytecode: &CompiledBytecode) -> bool {
    bytecode.bytecode.iter().all(is_supported) && stack_layout(&bytecode.bytecode).is_some()
}

/// Whether every op in the program is vectorizable by the tiled executors.
/// Lets callers skip expensive setup before falling back to the scalar path.
#[inline]
pub fn supported_program(bytecode: &CompiledBytecode) -> bool {
    all_supported(bytecode)
        && (all_fields_float(bytecode) || integer_program_type(bytecode).is_some())
}

/// Whether a MAP program (a `StoreField`-free expression producing one value per row)
/// is fully handled by [`run_map`]. Lets array adapters cheaply choose the tiled path
/// before allocating output.
#[inline]
pub fn map_supported(ops: &[Op]) -> bool {
    ops.iter()
        .all(|o| is_supported(o) && !matches!(o, Op::PushField(_) | Op::StoreField(_)))
        && stack_layout(ops).is_some_and(|(_, final_depth)| final_depth == 1)
}

/// Whether a MAP program can run entirely in the native-f32 tiled executor.
#[inline]
pub fn f32_native_eligible(ops: &[Op]) -> bool {
    map_supported(ops)
}

#[inline]
fn all_fields_f32(bytecode: &CompiledBytecode) -> bool {
    bytecode.bytecode.iter().all(|op| match op {
        Op::PushField(i) | Op::StoreField(i) => {
            bytecode.field_map[*i as usize].field_type == FieldType::F32
        }
        Op::PushInput(_) => false,
        _ => true,
    })
}

/// Borrow two disjoint tiles `[a_depth]` and `[b_depth]` (a_depth < b_depth) mutably.
#[inline]
fn two_tiles<T>(
    stack: &mut [T],
    a_depth: usize,
    b_depth: usize,
    len: usize,
) -> (&mut [T], &mut [T]) {
    debug_assert!(a_depth < b_depth);
    let (left, right) = stack.split_at_mut(b_depth * TILE);
    (
        &mut left[a_depth * TILE..a_depth * TILE + len],
        &mut right[..len],
    )
}

/// Apply one storage-independent stack op (`PushConst` + arithmetic) to the tile stack,
/// returning the new depth. Loads and stores are handled by the caller.
#[inline]
fn apply_stack_op(
    op: &Op,
    constants: &[f64],
    stack: &mut [f64],
    depth: usize,
    len: usize,
) -> usize {
    match op {
        Op::PushConst(idx) => {
            stack[depth * TILE..depth * TILE + len].fill(constants[*idx as usize]);
            depth + 1
        }
        Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Min | Op::Max | Op::Pow | Op::Mod => {
            let (a, b) = two_tiles(stack, depth - 2, depth - 1, len);
            match op {
                Op::Add => a.iter_mut().zip(&*b).for_each(|(x, y)| *x += *y),
                Op::Sub => a.iter_mut().zip(&*b).for_each(|(x, y)| *x -= *y),
                Op::Mul => a.iter_mut().zip(&*b).for_each(|(x, y)| *x *= *y),
                Op::Div => a.iter_mut().zip(&*b).for_each(|(x, y)| *x /= *y),
                Op::Min => a
                    .iter_mut()
                    .zip(&*b)
                    .for_each(|(x, y)| *x = python_minimum(*x, *y)),
                Op::Max => a
                    .iter_mut()
                    .zip(&*b)
                    .for_each(|(x, y)| *x = python_maximum(*x, *y)),
                Op::Pow => a.iter_mut().zip(&*b).for_each(|(x, y)| *x = x.powf(*y)),
                Op::Mod => a
                    .iter_mut()
                    .zip(&*b)
                    .for_each(|(x, y)| *x = python_remainder(*x, *y)),
                _ => unreachable!(),
            }
            depth - 1
        }
        Op::Neg
        | Op::Sqrt
        | Op::Abs
        | Op::Floor
        | Op::Ceil
        | Op::Round
        | Op::Sign
        | Op::Fract
        | Op::Sin
        | Op::Cos
        | Op::Tan
        | Op::Asin
        | Op::Acos
        | Op::Atan
        | Op::Exp
        | Op::Ln
        | Op::Log10
        | Op::Log2 => {
            let a = &mut stack[(depth - 1) * TILE..(depth - 1) * TILE + len];
            match op {
                Op::Neg => a.iter_mut().for_each(|x| *x = -*x),
                Op::Sqrt => a.iter_mut().for_each(|x| *x = x.sqrt()),
                Op::Abs => a.iter_mut().for_each(|x| *x = x.abs()),
                Op::Floor => a.iter_mut().for_each(|x| *x = x.floor()),
                Op::Ceil => a.iter_mut().for_each(|x| *x = x.ceil()),
                Op::Round => a.iter_mut().for_each(|x| *x = python_round(*x)),
                Op::Sign => a.iter_mut().for_each(|x| *x = python_sign(*x)),
                Op::Fract => a.iter_mut().for_each(|x| *x = x.fract()),
                Op::Sin => a.iter_mut().for_each(|x| *x = x.sin()),
                Op::Cos => a.iter_mut().for_each(|x| *x = x.cos()),
                Op::Tan => a.iter_mut().for_each(|x| *x = x.tan()),
                Op::Asin => a.iter_mut().for_each(|x| *x = x.asin()),
                Op::Acos => a.iter_mut().for_each(|x| *x = x.acos()),
                Op::Atan => a.iter_mut().for_each(|x| *x = x.atan()),
                Op::Exp => a.iter_mut().for_each(|x| *x = x.exp()),
                Op::Ln => a.iter_mut().for_each(|x| *x = x.ln()),
                Op::Log10 => a.iter_mut().for_each(|x| *x = x.log10()),
                Op::Log2 => a.iter_mut().for_each(|x| *x = x.log2()),
                _ => unreachable!(),
            }
            depth
        }
        // Clamp pops (max, min, value); Lerp pops (t, b, a).
        Op::Clamp | Op::Lerp => {
            let (a0, b0, c0) = ((depth - 3) * TILE, (depth - 2) * TILE, (depth - 1) * TILE);
            for l in 0..len {
                let (a, b, c) = (stack[a0 + l], stack[b0 + l], stack[c0 + l]);
                stack[a0 + l] = match op {
                    // value=a, min=b, max=c
                    Op::Clamp => python_clip(a, b, c),
                    // a + t*(b - a) with t=c
                    Op::Lerp => a + c * (b - a),
                    _ => unreachable!(),
                };
            }
            depth - 2
        }
        _ => unreachable!("pre-checked by caller; loads and stores handled separately"),
    }
}

/// Core f64 tile loop over rows `0..count`. `srcs` is indexed by field index (used by
/// `PushField`); `dests` is indexed by store order (the k-th `StoreField`). Callers must
/// pre-check ops.
///
/// # Safety
/// Every `srcs[i]` read by a `PushField(i)` and every `dests[k]` must be valid over
/// `0..count` (their construction contracts). See [`crate::columns`].
unsafe fn run_tiles_f64(
    bytecode: &CompiledBytecode,
    srcs: &[ColumnRef<'_>],
    dests: &[ColumnMut<'_>],
    count: usize,
    stack: &mut [f64],
) {
    let mut tile_start = 0usize;
    while tile_start < count {
        let len = (count - tile_start).min(TILE);
        let mut depth = 0usize;
        let mut store_idx = 0usize;
        for op in &bytecode.bytecode {
            match op {
                Op::PushField(idx) => {
                    let dst = &mut stack[depth * TILE..depth * TILE + len];
                    // SAFETY: srcs[idx] is valid over 0..count (fn contract); len <= count.
                    unsafe { load_tile_f64(&srcs[*idx as usize], tile_start, dst) };
                    depth += 1;
                }
                Op::StoreField(_idx) => {
                    let src = &stack[(depth - 1) * TILE..(depth - 1) * TILE + len];
                    // SAFETY: dests[store_idx] is valid over 0..count (fn contract).
                    unsafe { store_tile_f64(&dests[store_idx], tile_start, src) };
                    store_idx += 1;
                    depth -= 1;
                }
                other => depth = apply_stack_op(other, &bytecode.constants, stack, depth, len),
            }
        }
        tile_start += len;
    }
}

/// Core f64 tile loop for a MAP program: `ops` has no `StoreField` and produces exactly
/// one value per row, which is written to `out` each tile. Backs [`run_map`].
///
/// # Safety
/// Every `srcs[i]` read by a `PushInput(i)` and `out` must be valid over `0..count`.
unsafe fn run_tiles_map_f64(
    ops: &[Op],
    constants: &[f64],
    srcs: &[ColumnRef<'_>],
    out: &ColumnMut<'_>,
    count: usize,
    stack: &mut [f64],
) {
    let mut tile_start = 0usize;
    while tile_start < count {
        let len = (count - tile_start).min(TILE);
        let mut depth = 0usize;
        for op in ops {
            match op {
                Op::PushInput(idx) => {
                    let dst = &mut stack[depth * TILE..depth * TILE + len];
                    // SAFETY: srcs[idx] valid over 0..count (fn contract); len <= count.
                    unsafe { load_tile_f64(&srcs[*idx as usize], tile_start, dst) };
                    depth += 1;
                }
                other => depth = apply_stack_op(other, constants, stack, depth, len),
            }
        }
        debug_assert_eq!(depth, 1, "map program must leave exactly one result");
        // SAFETY: out valid over 0..count; result tile is stack[0..len].
        unsafe { store_tile_f64(out, tile_start, &stack[..len]) };
        tile_start += len;
    }
}

#[inline]
fn apply_stack_op_f32(
    op: &Op,
    constants: &[f64],
    stack: &mut [f32],
    depth: usize,
    len: usize,
) -> usize {
    match op {
        Op::PushConst(idx) => {
            stack[depth * TILE..depth * TILE + len].fill(constants[*idx as usize] as f32);
            depth + 1
        }
        Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Min | Op::Max | Op::Pow | Op::Mod => {
            let (a, b) = two_tiles(stack, depth - 2, depth - 1, len);
            match op {
                Op::Add => a.iter_mut().zip(&*b).for_each(|(x, y)| *x += *y),
                Op::Sub => a.iter_mut().zip(&*b).for_each(|(x, y)| *x -= *y),
                Op::Mul => a.iter_mut().zip(&*b).for_each(|(x, y)| *x *= *y),
                Op::Div => a.iter_mut().zip(&*b).for_each(|(x, y)| *x /= *y),
                Op::Min => a
                    .iter_mut()
                    .zip(&*b)
                    .for_each(|(x, y)| *x = min_f32_nan(*x, *y)),
                Op::Max => a
                    .iter_mut()
                    .zip(&*b)
                    .for_each(|(x, y)| *x = max_f32_nan(*x, *y)),
                Op::Pow => a.iter_mut().zip(&*b).for_each(|(x, y)| *x = x.powf(*y)),
                Op::Mod => a
                    .iter_mut()
                    .zip(&*b)
                    .for_each(|(x, y)| *x = remainder_f32(*x, *y)),
                _ => unreachable!(),
            }
            depth - 1
        }
        Op::Neg
        | Op::Sqrt
        | Op::Abs
        | Op::Floor
        | Op::Ceil
        | Op::Round
        | Op::Sign
        | Op::Fract
        | Op::Sin
        | Op::Cos
        | Op::Tan
        | Op::Asin
        | Op::Acos
        | Op::Atan
        | Op::Exp
        | Op::Ln
        | Op::Log10
        | Op::Log2 => {
            let a = &mut stack[(depth - 1) * TILE..(depth - 1) * TILE + len];
            match op {
                Op::Neg => a.iter_mut().for_each(|x| *x = -*x),
                Op::Sqrt => a.iter_mut().for_each(|x| *x = x.sqrt()),
                Op::Abs => a.iter_mut().for_each(|x| *x = x.abs()),
                Op::Floor => a.iter_mut().for_each(|x| *x = x.floor()),
                Op::Ceil => a.iter_mut().for_each(|x| *x = x.ceil()),
                Op::Round => a.iter_mut().for_each(|x| *x = x.round_ties_even()),
                Op::Sign => a.iter_mut().for_each(|x| *x = sign_f32(*x)),
                Op::Fract => a.iter_mut().for_each(|x| *x = x.fract()),
                Op::Sin => a.iter_mut().for_each(|x| *x = x.sin()),
                Op::Cos => a.iter_mut().for_each(|x| *x = x.cos()),
                Op::Tan => a.iter_mut().for_each(|x| *x = x.tan()),
                Op::Asin => a.iter_mut().for_each(|x| *x = x.asin()),
                Op::Acos => a.iter_mut().for_each(|x| *x = x.acos()),
                Op::Atan => a.iter_mut().for_each(|x| *x = x.atan()),
                Op::Exp => a.iter_mut().for_each(|x| *x = x.exp()),
                Op::Ln => a.iter_mut().for_each(|x| *x = x.ln()),
                Op::Log10 => a.iter_mut().for_each(|x| *x = x.log10()),
                Op::Log2 => a.iter_mut().for_each(|x| *x = x.log2()),
                _ => unreachable!(),
            }
            depth
        }
        Op::Clamp | Op::Lerp => {
            let (a0, b0, c0) = ((depth - 3) * TILE, (depth - 2) * TILE, (depth - 1) * TILE);
            for lane in 0..len {
                let (a, b, c) = (stack[a0 + lane], stack[b0 + lane], stack[c0 + lane]);
                stack[a0 + lane] = match op {
                    Op::Clamp => clip_f32(a, b, c),
                    Op::Lerp => a + c * (b - a),
                    _ => unreachable!(),
                };
            }
            depth - 2
        }
        _ => unreachable!("pre-checked by caller; loads and stores handled separately"),
    }
}

/// Core f32 tile loop over entities `0..count`. Callers must pre-check ops + f32 fields.
///
/// # Safety
/// As [`run_tiles_f64`], and every referenced field must be `F32`.
unsafe fn run_tiles_f32(
    bytecode: &CompiledBytecode,
    field_bases: &[*mut u8],
    strides: &[usize],
    count: usize,
    stack: &mut [f32],
) {
    let mut tile_start = 0usize;
    while tile_start < count {
        let len = (count - tile_start).min(TILE);
        let mut depth = 0usize;
        for op in &bytecode.bytecode {
            match op {
                Op::PushField(idx) => {
                    let i = *idx as usize;
                    let (base, stride) = (field_bases[i], strides[i]);
                    let dst = &mut stack[depth * TILE..depth * TILE + len];
                    for (l, slot) in dst.iter_mut().enumerate() {
                        // SAFETY: fn contract guarantees base.add((tile_start+l)*stride) is a
                        // valid F32 field pointer for every l < len (all fields pre-checked F32).
                        unsafe {
                            let ptr = base.add((tile_start + l) * stride) as *const f32;
                            *slot = ptr.read_unaligned();
                        }
                    }
                    depth += 1;
                }
                Op::StoreField(idx) => {
                    let i = *idx as usize;
                    let (base, stride) = (field_bases[i], strides[i]);
                    let src = &stack[(depth - 1) * TILE..(depth - 1) * TILE + len];
                    for (l, val) in src.iter().enumerate() {
                        // SAFETY: fn contract guarantees base.add((tile_start+l)*stride) is a
                        // valid writable F32 field pointer for every l < len.
                        unsafe {
                            let ptr = base.add((tile_start + l) * stride) as *mut f32;
                            ptr.write_unaligned(*val);
                        }
                    }
                    depth -= 1;
                }
                other => {
                    depth = apply_stack_op_f32(other, &bytecode.constants, stack, depth, len);
                }
            }
        }
        tile_start += len;
    }
}

/// Column-based f64 assignment executor (F64Strict; bit-identical to the scalar VM up
/// to NaN sign). `srcs` is indexed by field index; `dests` by store order (the k-th
/// `StoreField`). This is the entry adapters build directly for zero-copy contiguous
/// columns; [`execute_assignment_tiled`] is the strided-pointer convenience wrapper.
///
/// # Safety
/// Every `srcs[i]` read by a `PushField(i)` and every `dests[k]` must be valid over
/// `0..count`; destinations must not overlap sources except as an exact same-run
/// in-place alias (see [`crate::columns::in_place_pair`]). Validity/aliasing is the
/// adapter's obligation, checked before the call (see module and design docs).
pub unsafe fn run_assignment(
    bytecode: &CompiledBytecode,
    srcs: &[ColumnRef<'_>],
    dests: &[ColumnMut<'_>],
    count: usize,
    scratch: &mut TiledScratch,
) -> Result<(), UnsupportedOp> {
    if !all_supported(bytecode) || !all_fields_float(bytecode) {
        return Err(UnsupportedOp);
    }
    let (depth, _) = stack_layout(&bytecode.bytecode).expect("checked by all_supported");
    scratch.ensure_depth(depth);
    // SAFETY: forwards this fn's column-validity contract to run_tiles_f64.
    unsafe { run_tiles_f64(bytecode, srcs, dests, count, &mut scratch.stack) };
    Ok(())
}

/// Column-based f64 MAP executor for array element-wise operations: `ops` is a `StoreField`-free
/// float program producing one value per row, written to `out` (F64Strict interior;
/// `out`'s dtype narrows the result). `srcs` is indexed by input index. Returns
/// `UnsupportedOp` if any op is outside the tiled map set.
///
/// # Safety
/// Every `srcs[i]` read by a `PushInput(i)` and `out` must be valid over `0..count`;
/// `out` must not alias any source. Validity is the adapter's obligation.
pub unsafe fn run_map(
    ops: &[Op],
    constants: &[f64],
    srcs: &[ColumnRef<'_>],
    out: &ColumnMut<'_>,
    count: usize,
    scratch: &mut TiledScratch,
) -> Result<(), UnsupportedOp> {
    if !map_supported(ops)
        || ops.iter().any(|op| match op {
            Op::PushInput(index) => *index as usize >= srcs.len(),
            Op::PushConst(index) => *index as usize >= constants.len(),
            _ => false,
        })
    {
        return Err(UnsupportedOp);
    }
    let (depth, _) = stack_layout(ops).expect("checked by map_supported");
    scratch.ensure_depth(depth);
    // SAFETY: forwards the column-validity contract to run_tiles_map_f64.
    unsafe { run_tiles_map_f64(ops, constants, srcs, out, count, &mut scratch.stack) };
    Ok(())
}

/// Whole-array floating-point reduction operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReduceOp {
    Sum,
    Min,
    Max,
}

/// The stable accumulation width used by whole-array `sum` and `mean`.
///
/// Lane `k` receives flat indices congruent to `k` modulo this value, in
/// increasing index order, and the lanes are folded left-to-right. Keeping the
/// order explicit makes results independent of the execution path and backend.
pub const REDUCE_LANES: usize = 8;
const _: () = assert!(
    TILE.is_multiple_of(REDUCE_LANES),
    "TILE must be a multiple of REDUCE_LANES"
);

#[inline]
fn reduce_combine(op: ReduceOp, left: f64, right: f64) -> f64 {
    match op {
        ReduceOp::Sum => left + right,
        ReduceOp::Min => python_minimum(left, right),
        ReduceOp::Max => python_maximum(left, right),
    }
}

#[inline]
fn reduction_identity(op: ReduceOp) -> [f64; REDUCE_LANES] {
    let value = match op {
        ReduceOp::Sum => 0.0,
        ReduceOp::Min => f64::INFINITY,
        ReduceOp::Max => f64::NEG_INFINITY,
    };
    [value; REDUCE_LANES]
}

#[inline]
fn feed_reduction_lanes(op: ReduceOp, lanes: &mut [f64; REDUCE_LANES], values: &[f64]) {
    let mut chunks = values.chunks_exact(REDUCE_LANES);
    for chunk in &mut chunks {
        for (lane, &value) in lanes.iter_mut().zip(chunk) {
            *lane = reduce_combine(op, *lane, value);
        }
    }
    for (lane, &value) in lanes.iter_mut().zip(chunks.remainder()) {
        *lane = reduce_combine(op, *lane, value);
    }
}

#[inline]
fn finish_reduction_lanes(op: ReduceOp, lanes: &[f64; REDUCE_LANES]) -> f64 {
    let mut result = lanes[0];
    for &lane in &lanes[1..] {
        result = reduce_combine(op, result, lane);
    }
    result
}

/// Reduce an f64 slice in the stable lane order used by [`run_reduce`].
pub fn lane_reduce_slice(op: ReduceOp, values: &[f64]) -> f64 {
    let mut lanes = reduction_identity(op);
    feed_reduction_lanes(op, &mut lanes, values);
    finish_reduction_lanes(op, &lanes)
}

/// Reduce one floating-point column. f32 inputs widen to f64 before accumulation.
/// Empty input returns the operation identity; callers reject empty min/max.
///
/// # Safety
/// `source` must be valid for reads over rows `0..count`.
pub unsafe fn run_reduce(
    op: ReduceOp,
    source: &ColumnRef<'_>,
    count: usize,
    scratch: &mut TiledScratch,
) -> f64 {
    let mut lanes = reduction_identity(op);
    let mut tile_start = 0usize;
    while tile_start < count {
        let len = (count - tile_start).min(TILE);
        let tile = &mut scratch.stack[..len];
        // SAFETY: `source` is valid over `0..count`, and this tile is in bounds.
        unsafe { load_tile_f64(source, tile_start, tile) };
        // TILE is a multiple of REDUCE_LANES, so tile-local and global lane
        // indices are identical, including for the final partial tile.
        feed_reduction_lanes(op, &mut lanes, tile);
        tile_start += len;
    }
    finish_reduction_lanes(op, &lanes)
}

/// Native-f32 MAP tile loop with no widening to f64.
///
/// # Safety
/// Every `srcs[i]` read by a `PushInput(i)` and `out` must be valid contiguous-f32 runs
/// over `0..count`.
unsafe fn run_tiles_map_f32(
    ops: &[Op],
    constants: &[f64],
    srcs: &[ColumnRef<'_>],
    out: &ColumnMut<'_>,
    count: usize,
    stack: &mut [f32],
) {
    let mut tile_start = 0usize;
    while tile_start < count {
        let len = (count - tile_start).min(TILE);
        let mut depth = 0usize;
        for op in ops {
            match op {
                Op::PushInput(idx) => {
                    let dst = &mut stack[depth * TILE..depth * TILE + len];
                    // SAFETY: srcs[idx] valid over 0..count; len <= count.
                    unsafe { load_tile_f32(&srcs[*idx as usize], tile_start, dst) };
                    depth += 1;
                }
                other => depth = apply_stack_op_f32(other, constants, stack, depth, len),
            }
        }
        debug_assert_eq!(depth, 1, "map program must leave exactly one result");
        // SAFETY: out valid over 0..count; result tile is stack[0..len].
        unsafe { store_tile_f32(out, tile_start, &stack[..len]) };
        tile_start += len;
    }
}

/// Native-f32 MAP executor. Returns `UnsupportedOp` when the program or columns do not
/// satisfy the native-f32 requirements.
///
/// # Safety
/// As [`run_map`], and every column must be contiguous f32 (or a broadcast source).
pub unsafe fn run_map_f32(
    ops: &[Op],
    constants: &[f64],
    srcs: &[ColumnRef<'_>],
    out: &ColumnMut<'_>,
    count: usize,
    scratch: &mut TiledScratch,
) -> Result<(), UnsupportedOp> {
    if !f32_native_eligible(ops)
        || !srcs.iter().all(|column| column.supports_native_f32(count))
        || !out.supports_native_f32(count)
        || ops.iter().any(|op| match op {
            Op::PushInput(index) => *index as usize >= srcs.len(),
            Op::PushConst(index) => *index as usize >= constants.len(),
            _ => false,
        })
    {
        return Err(UnsupportedOp);
    }
    let (depth, _) = stack_layout(ops).expect("checked by f32_native_eligible");
    scratch.ensure_depth(depth);
    // SAFETY: forwards the column-validity contract to run_tiles_map_f32.
    unsafe { run_tiles_map_f32(ops, constants, srcs, out, count, &mut scratch.stack_f32) };
    Ok(())
}

/// Execute a homogeneous fixed-width integer program without widening through f64.
/// Arithmetic deliberately uses wrapping operations, matching fixed-width array lanes.
///
/// # Safety
/// Every referenced base/stride pair must be valid for `count` values of `T`, and
/// each bytecode field must have `T::FIELD_TYPE` (checked by the dispatcher).
unsafe fn run_tiles_integer<T: IntegerLane>(
    bytecode: &CompiledBytecode,
    field_bases: &[*mut u8],
    strides: &[usize],
    count: usize,
    stack: &mut [T],
) {
    debug_assert!(bytecode.bytecode.iter().all(|op| match op {
        Op::PushField(index) | Op::StoreField(index) => {
            bytecode.field_map[*index as usize].field_type == T::FIELD_TYPE
        }
        _ => true,
    }));
    let mut tile_start = 0usize;
    while tile_start < count {
        let len = (count - tile_start).min(TILE);
        let mut depth = 0usize;
        for op in &bytecode.bytecode {
            match op {
                Op::PushField(index) => {
                    let index = *index as usize;
                    let destination = &mut stack[depth * TILE..depth * TILE + len];
                    for (lane, slot) in destination.iter_mut().enumerate() {
                        // SAFETY: guaranteed by this function's pointer contract and
                        // integer_program_type's homogeneous type validation.
                        let ptr = unsafe {
                            field_bases[index].add((tile_start + lane) * strides[index]) as *const T
                        };
                        // SAFETY: `ptr` addresses the validated homogeneous lane
                        // within the caller-provided column extent.
                        *slot = unsafe { ptr.read_unaligned() };
                    }
                    depth += 1;
                }
                Op::PushConst(index) => {
                    let value = T::from_constant(bytecode.constants[*index as usize])
                        .expect("integer_program_type validates constants");
                    stack[depth * TILE..depth * TILE + len].fill(value);
                    depth += 1;
                }
                Op::StoreField(index) => {
                    let index = *index as usize;
                    let source = &stack[(depth - 1) * TILE..(depth - 1) * TILE + len];
                    for (lane, value) in source.iter().enumerate() {
                        // SAFETY: guaranteed by this function's pointer contract and
                        // integer_program_type's homogeneous type validation.
                        let ptr = unsafe {
                            field_bases[index].add((tile_start + lane) * strides[index]) as *mut T
                        };
                        // SAFETY: `ptr` addresses the validated destination lane,
                        // and the executor holds exclusive access for this call.
                        unsafe { ptr.write_unaligned(*value) };
                    }
                    depth -= 1;
                }
                Op::Add | Op::Sub | Op::Mul | Op::Min | Op::Max => {
                    let (a, b) = two_tiles(stack, depth - 2, depth - 1, len);
                    match op {
                        Op::Add => a
                            .iter_mut()
                            .zip(&*b)
                            .for_each(|(a, b)| *a = a.wrapping_add(*b)),
                        Op::Sub => a
                            .iter_mut()
                            .zip(&*b)
                            .for_each(|(a, b)| *a = a.wrapping_sub(*b)),
                        Op::Mul => a
                            .iter_mut()
                            .zip(&*b)
                            .for_each(|(a, b)| *a = a.wrapping_mul(*b)),
                        Op::Min => a.iter_mut().zip(&*b).for_each(|(a, b)| *a = (*a).min(*b)),
                        Op::Max => a.iter_mut().zip(&*b).for_each(|(a, b)| *a = (*a).max(*b)),
                        _ => unreachable!(),
                    }
                    depth -= 1;
                }
                Op::Neg | Op::Abs | Op::Sign => {
                    let values = &mut stack[(depth - 1) * TILE..(depth - 1) * TILE + len];
                    match op {
                        Op::Neg => values.iter_mut().for_each(|v| *v = v.wrapping_neg()),
                        Op::Abs => values.iter_mut().for_each(|v| *v = v.wrapping_abs()),
                        Op::Sign => values.iter_mut().for_each(|v| *v = v.sign()),
                        _ => unreachable!(),
                    }
                }
                Op::Floor | Op::Ceil | Op::Round => {}
                Op::Clamp => {
                    let (value_start, min_start, max_start) =
                        ((depth - 3) * TILE, (depth - 2) * TILE, (depth - 1) * TILE);
                    for lane in 0..len {
                        let value = stack[value_start + lane];
                        let min = stack[min_start + lane];
                        let max = stack[max_start + lane];
                        stack[value_start + lane] = value.max(min).min(max);
                    }
                    depth -= 2;
                }
                _ => unreachable!("integer_program_type pre-checked every operation"),
            }
        }
        tile_start += len;
    }
}

unsafe fn run_integer_dispatch(
    lane_type: FieldType,
    bytecode: &CompiledBytecode,
    field_bases: &[*mut u8],
    strides: &[usize],
    count: usize,
    scratch: &mut TiledScratch,
    depth: usize,
) {
    let required = depth * TILE;
    macro_rules! run {
        ($field:ident, $ty:ty) => {{
            if scratch.$field.len() < required {
                scratch.$field.resize(required, <$ty>::default());
            }
            // SAFETY: caller forwards the raw-pointer contract; lane_type was validated.
            unsafe {
                run_tiles_integer::<$ty>(bytecode, field_bases, strides, count, &mut scratch.$field)
            };
        }};
    }
    match lane_type {
        FieldType::I32 => run!(stack_i32, i32),
        FieldType::I64 => run!(stack_i64, i64),
        FieldType::U8 => run!(stack_u8, u8),
        FieldType::U32 => run!(stack_u32, u32),
        FieldType::U64 => run!(stack_u64, u64),
        _ => unreachable!("integer_program_type only returns integer fields"),
    }
}

/// Build strided columns from raw `field_bases`/`strides` for rows `[start, start+count)`:
/// `srcs` indexed by field index, `dests` in `StoreField` order. Used by the raw-pointer
/// wrapper and the parallel f64 chunks.
///
/// # Safety
/// `field_bases[i].add((start+e) * strides[i])` valid for referenced `i`, `e in 0..count`.
unsafe fn build_strided_columns<'a>(
    bytecode: &CompiledBytecode,
    field_bases: &[*mut u8],
    strides: &[usize],
    start: usize,
    count: usize,
) -> (Vec<ColumnRef<'a>>, Vec<ColumnMut<'a>>) {
    let column = |i: usize| {
        let stride = strides[i] as isize;
        // SAFETY: caller contract; `start` offset keeps every 0..count row in bounds.
        let base = unsafe { field_bases[i].offset(stride * start as isize) };
        (base, stride, bytecode.field_map[i].field_type)
    };
    let srcs = (0..field_bases.len())
        .map(|i| {
            let (base, stride, ftype) = column(i);
            // SAFETY: strided run valid for 0..count by the caller contract.
            unsafe { ColumnRef::strided_1d(base as *const u8, stride, ftype, count) }
        })
        .collect();
    let dests = bytecode
        .bytecode
        .iter()
        .filter_map(|op| match op {
            Op::StoreField(idx) => {
                let (base, stride, ftype) = column(*idx as usize);
                // SAFETY: exclusive strided destination run valid for 0..count.
                Some(unsafe { ColumnMut::strided_1d(base, stride, ftype, count) })
            }
            _ => None,
        })
        .collect();
    (srcs, dests)
}

/// Serial f64 tiled executor over raw field pointers (bit-identical to the scalar VM up
/// to NaN sign). Convenience wrapper: builds strided columns and calls [`run_assignment`].
///
/// # Safety
/// `field_bases[i].add(e * strides[i])` valid for all referenced `i`, `e in 0..entity_count`.
pub unsafe fn execute_assignment_tiled(
    bytecode: &CompiledBytecode,
    field_bases: &[*mut u8],
    strides: &[usize],
    entity_count: usize,
    scratch: &mut TiledScratch,
) -> Result<(), UnsupportedOp> {
    if !supported_program(bytecode) {
        return Err(UnsupportedOp);
    }
    let (depth, _) = stack_layout(&bytecode.bytecode).expect("checked by all_supported");
    if let Some(lane_type) = integer_program_type(bytecode) {
        // SAFETY: forwards this function's pointer-validity contract; the dispatcher
        // validates every field and constant against the homogeneous integer lane.
        unsafe {
            run_integer_dispatch(
                lane_type,
                bytecode,
                field_bases,
                strides,
                entity_count,
                scratch,
                depth,
            )
        };
        return Ok(());
    }
    scratch.ensure_depth(depth);
    // SAFETY: forwards this fn's pointer-validity contract to build_strided_columns.
    let (srcs, dests) =
        unsafe { build_strided_columns(bytecode, field_bases, strides, 0, entity_count) };
    // SAFETY: columns are valid over 0..entity_count; any read/write alias of the same
    // field is an exact same-run alias (identical base/stride/len), which is permitted.
    unsafe { run_tiles_f64(bytecode, &srcs, &dests, entity_count, &mut scratch.stack) };
    Ok(())
}

/// Serial f32-native tiled executor (~2x lanes; intermediates in f32, NOT
/// bit-identical to the scalar VM). Requires all referenced fields to be `F32`.
///
/// # Safety
/// As [`execute_assignment_tiled`].
pub unsafe fn execute_assignment_tiled_f32(
    bytecode: &CompiledBytecode,
    field_bases: &[*mut u8],
    strides: &[usize],
    entity_count: usize,
    scratch: &mut TiledScratch,
) -> Result<(), UnsupportedOp> {
    if !all_supported(bytecode) || !all_fields_f32(bytecode) {
        return Err(UnsupportedOp);
    }
    let (depth, _) = stack_layout(&bytecode.bytecode).expect("checked by all_supported");
    scratch.ensure_depth(depth);
    // SAFETY: forwards this fn's identical pointer-validity contract to run_tiles_f32.
    unsafe {
        run_tiles_f32(
            bytecode,
            field_bases,
            strides,
            entity_count,
            &mut scratch.stack_f32,
        )
    };
    Ok(())
}

/// Parallel tiled executor: entity range split into `PCHUNK` tasks, each running
/// the tiled (SIMD) loop with its own stack — i.e. SIMD × threads. Falls back to
/// serial when the `parallel` feature is off.
///
/// # Safety
/// As [`execute_assignment_tiled`]. No other code may touch the same memory.
pub unsafe fn execute_assignment_tiled_parallel(
    bytecode: &CompiledBytecode,
    field_bases: &[*mut u8],
    strides: &[usize],
    entity_count: usize,
    use_f32: bool,
) -> Result<(), UnsupportedOp> {
    if !supported_program(bytecode) || (use_f32 && !all_fields_f32(bytecode)) {
        return Err(UnsupportedOp);
    }
    if !use_f32 && let Some(lane_type) = integer_program_type(bytecode) {
        let (depth, _) = stack_layout(&bytecode.bytecode).expect("checked by supported_program");
        let mut scratch = TiledScratch::new();
        // Integer lanes use their exact tiled specialization. A parallel integer
        // splitter can be added independently without changing arithmetic semantics.
        // SAFETY: this function forwards its validated bases, strides, and entity
        // count unchanged to the matching integer-lane dispatcher.
        unsafe {
            run_integer_dispatch(
                lane_type,
                bytecode,
                field_bases,
                strides,
                entity_count,
                &mut scratch,
                depth,
            )
        };
        return Ok(());
    }
    // SAFETY: forwards this fn's identical pointer-validity contract to parallel_run.
    unsafe { parallel_run(bytecode, field_bases, strides, entity_count, use_f32) };
    Ok(())
}

#[cfg(feature = "parallel")]
unsafe fn parallel_run(
    bytecode: &CompiledBytecode,
    field_bases: &[*mut u8],
    strides: &[usize],
    entity_count: usize,
    use_f32: bool,
) {
    use rayon::prelude::*;

    let (depth, _) = stack_layout(&bytecode.bytecode).expect("validated by caller");
    let base_addrs: Vec<usize> = field_bases.iter().map(|p| *p as usize).collect();
    let nfields = field_bases.len();
    let num_chunks = entity_count.div_ceil(PCHUNK).max(1);

    (0..num_chunks).into_par_iter().for_each(|c| {
        let start = c * PCHUNK;
        if start >= entity_count {
            return;
        }
        let count = (entity_count - start).min(PCHUNK);
        // Chunks are disjoint entity ranges [start, start+count) with per-task scratch,
        // so no two rayon tasks touch the same field memory.
        if use_f32 {
            let mut bases: Vec<*mut u8> = Vec::with_capacity(nfields);
            for i in 0..nfields {
                // SAFETY: `start < entity_count`; the fn contract makes offsets up to
                // entity_count valid, so the per-chunk base of field i is in bounds.
                bases.push(unsafe { (base_addrs[i] as *mut u8).add(start * strides[i]) });
            }
            let mut stack = vec![0.0f32; depth.max(MAX_DEPTH) * TILE];
            // SAFETY: `bases`/`strides` are valid for 0..count within this chunk.
            unsafe { run_tiles_f32(bytecode, &bases, strides, count, &mut stack) };
        } else {
            let bases: Vec<*mut u8> = base_addrs.iter().map(|a| *a as *mut u8).collect();
            // SAFETY: `bases`/`strides` valid to entity_count; `start` keeps the chunk in bounds.
            let (srcs, dests) =
                unsafe { build_strided_columns(bytecode, &bases, strides, start, count) };
            let mut stack = vec![0.0f64; depth.max(MAX_DEPTH) * TILE];
            // SAFETY: columns valid over this chunk's 0..count.
            unsafe { run_tiles_f64(bytecode, &srcs, &dests, count, &mut stack) };
        }
    });
}

#[cfg(not(feature = "parallel"))]
unsafe fn parallel_run(
    bytecode: &CompiledBytecode,
    field_bases: &[*mut u8],
    strides: &[usize],
    entity_count: usize,
    use_f32: bool,
) {
    let mut scratch = TiledScratch::new();
    let (depth, _) = stack_layout(&bytecode.bytecode).expect("validated by caller");
    scratch.ensure_depth(depth);
    if use_f32 {
        // SAFETY: forwards this fn's pointer-validity contract to run_tiles_f32.
        unsafe {
            run_tiles_f32(
                bytecode,
                field_bases,
                strides,
                entity_count,
                &mut scratch.stack_f32,
            )
        };
    } else {
        // SAFETY: field_bases/strides valid over 0..entity_count by the fn contract.
        let (srcs, dests) =
            unsafe { build_strided_columns(bytecode, field_bases, strides, 0, entity_count) };
        // SAFETY: columns valid over 0..entity_count.
        unsafe { run_tiles_f64(bytecode, &srcs, &dests, entity_count, &mut scratch.stack) };
    }
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use bevy_ecs::component::ComponentId;

    use super::*;
    use crate::bytecode::{Compiler, FieldId, FieldType, Op, VM};

    fn fields() -> (FieldId, FieldId) {
        (
            FieldId {
                component_id: ComponentId::new(0),
                offset: 0,
                field_type: FieldType::F32,
            },
            FieldId {
                component_id: ComponentId::new(0),
                offset: 4,
                field_type: FieldType::F32,
            },
        )
    }

    fn finite_values(rng: &mut SplitMix, len: usize) -> Vec<f64> {
        (0..len)
            .map(|_| (rng.next_u64() as f64 / u64::MAX as f64) * 200.0 - 100.0)
            .collect()
    }

    #[test]
    fn reduction_sum_order_is_stable() {
        let mut values = vec![1.0f64; 16];
        values[0] = 1e16;
        values[8] = -1e16;

        assert_eq!(lane_reduce_slice(ReduceOp::Sum, &values), 14.0);

        let source = ColumnRef::from_f64_slice(&values);
        let mut scratch = TiledScratch::new();
        // SAFETY: `source` covers every row in `values`.
        let actual = unsafe { run_reduce(ReduceOp::Sum, &source, values.len(), &mut scratch) };
        assert_eq!(actual, 14.0);
    }

    #[test]
    fn column_reduction_matches_slice_order_for_f64() {
        let mut rng = SplitMix::new(0x5EED_0055_0001);
        let mut scratch = TiledScratch::new();
        for len in [0usize, 1, 7, 8, 255, 256, 257, 512, 1000, 100_000] {
            let values = finite_values(&mut rng, len);
            let source = ColumnRef::from_f64_slice(&values);
            for op in [ReduceOp::Sum, ReduceOp::Min, ReduceOp::Max] {
                // SAFETY: `source` covers `0..len`.
                let actual = unsafe { run_reduce(op, &source, len, &mut scratch) };
                let expected = lane_reduce_slice(op, &values);
                assert_eq!(actual.to_bits(), expected.to_bits(), "op={op:?} len={len}");
            }
        }
    }

    #[test]
    fn column_reduction_widens_f32_before_accumulating() {
        let mut rng = SplitMix::new(0x5EED_0055_F320);
        let mut scratch = TiledScratch::new();
        for len in [1usize, 8, 256, 257, 1000] {
            let values: Vec<f32> = finite_values(&mut rng, len)
                .into_iter()
                .map(|value| value as f32)
                .collect();
            let widened: Vec<f64> = values.iter().map(|&value| f64::from(value)).collect();
            let source = ColumnRef::from_f32_slice(&values);
            for op in [ReduceOp::Sum, ReduceOp::Min, ReduceOp::Max] {
                // SAFETY: `source` covers `0..len`.
                let actual = unsafe { run_reduce(op, &source, len, &mut scratch) };
                let expected = lane_reduce_slice(op, &widened);
                assert_eq!(actual.to_bits(), expected.to_bits(), "op={op:?} len={len}");
            }
        }
    }

    #[test]
    fn min_and_max_reductions_propagate_nan() {
        let mut scratch = TiledScratch::new();
        for position in [0usize, 1, 5, 254, 255, 256, 257, 299] {
            let mut values = vec![1.0f64; 300];
            values[position] = f64::NAN;
            let source = ColumnRef::from_f64_slice(&values);
            // SAFETY: `source` covers every row in `values`.
            let minimum = unsafe { run_reduce(ReduceOp::Min, &source, values.len(), &mut scratch) };
            // SAFETY: `source` covers every row in `values`.
            let maximum = unsafe { run_reduce(ReduceOp::Max, &source, values.len(), &mut scratch) };
            assert!(minimum.is_nan(), "position={position}");
            assert!(maximum.is_nan(), "position={position}");
        }
    }

    /// x = x + v * dt
    fn integrate_program(dt: f64) -> CompiledBytecode {
        let mut c = Compiler::new();
        let (x, v) = fields();
        let xi = c.add_field(x);
        let vi = c.add_field(v);
        let dti = c.add_constant(dt);
        c.emit(Op::PushField(xi));
        c.emit(Op::PushField(vi));
        c.emit(Op::PushConst(dti));
        c.emit(Op::Mul);
        c.emit(Op::Add);
        c.emit(Op::StoreField(xi));
        c.finalize()
    }

    unsafe fn run_scalar(bc: &CompiledBytecode, buf: &mut [f32], n: usize) {
        let mut vm = VM::new();
        let base = buf.as_mut_ptr() as *mut u8;
        for i in 0..n {
            let xp = unsafe { base.add(i * 8) };
            let vp = unsafe { base.add(i * 8 + 4) };
            unsafe { vm.execute(bc, &[xp, vp], i) };
        }
    }

    fn bases_strides(buf: &mut [f32]) -> ([*mut u8; 2], [usize; 2]) {
        let base = buf.as_mut_ptr() as *mut u8;
        ([base, unsafe { base.add(4) }], [8usize, 8usize])
    }

    #[test]
    fn tiled_matches_scalar_integrate() {
        let n = 1000;
        let bc = integrate_program(0.016);
        let mut a: Vec<f32> = (0..n)
            .flat_map(|i| [i as f32 * 0.5, (i as f32).sin()])
            .collect();
        let mut b = a.clone();
        unsafe { run_scalar(&bc, &mut a, n) };
        let (bases, strides) = bases_strides(&mut b);
        let mut s = TiledScratch::new();
        unsafe { execute_assignment_tiled(&bc, &bases, &strides, n, &mut s).unwrap() };
        assert_eq!(a, b, "serial f64 tiled must match scalar bit-for-bit");
    }

    #[test]
    fn tiled_f32_matches_naive_f32() {
        let n = 1000;
        let dt = 0.016f64;
        let bc = integrate_program(dt);
        let init: Vec<f32> = (0..n)
            .flat_map(|i| [i as f32 * 0.5, (i as f32).sin()])
            .collect();
        let mut got = init.clone();
        let (bases, strides) = bases_strides(&mut got);
        let mut s = TiledScratch::new();
        unsafe { execute_assignment_tiled_f32(&bc, &bases, &strides, n, &mut s).unwrap() };
        let mut want = init.clone();
        for i in 0..n {
            want[i * 2] += want[i * 2 + 1] * dt as f32;
        }
        assert_eq!(got, want, "f32-native must match naive f32 computation");
    }

    #[test]
    fn tiled_f32_supports_extended_operations() {
        let mut compiler = Compiler::new();
        let field = compiler.add_field(FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        });
        let exponent = compiler.add_constant(2.0);
        let minimum = compiler.add_constant(-0.25);
        let maximum = compiler.add_constant(0.75);
        compiler.emit(Op::PushField(field));
        compiler.emit(Op::Sin);
        compiler.emit(Op::PushConst(exponent));
        compiler.emit(Op::Pow);
        compiler.emit(Op::PushConst(minimum));
        compiler.emit(Op::PushConst(maximum));
        compiler.emit(Op::Clamp);
        compiler.emit(Op::StoreField(field));
        let bytecode = compiler.finalize();

        let input: Vec<f32> = (0..300).map(|index| index as f32 * 0.01 - 1.5).collect();
        let expected: Vec<f32> = input
            .iter()
            .map(|value| clip_f32(value.sin().powf(2.0), -0.25, 0.75))
            .collect();
        let mut actual = input;
        let bases = [actual.as_mut_ptr().cast::<u8>()];
        let strides = [size_of::<f32>()];
        let mut scratch = TiledScratch::new();
        // SAFETY: bases contains the live contiguous storage for every row in actual.
        unsafe {
            execute_assignment_tiled_f32(&bytecode, &bases, &strides, actual.len(), &mut scratch)
                .unwrap()
        };

        assert_eq!(actual, expected);
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn tiled_parallel_matches_serial() {
        let n = 100_003; // spans many PCHUNKs + a ragged tail
        let bc = integrate_program(0.016);
        let init: Vec<f32> = (0..n)
            .flat_map(|i| [i as f32 * 0.5, (i as f32).cos()])
            .collect();

        let mut a = init.clone();
        let (ba, sa) = bases_strides(&mut a);
        let mut s = TiledScratch::new();
        unsafe { execute_assignment_tiled(&bc, &ba, &sa, n, &mut s).unwrap() };

        let mut b = init.clone();
        let (bb, sb) = bases_strides(&mut b);
        unsafe { execute_assignment_tiled_parallel(&bc, &bb, &sb, n, false).unwrap() };

        assert_eq!(a, b, "parallel f64 tiled must match serial");
    }

    #[test]
    fn unsupported_op_falls_back() {
        let mut c = Compiler::new();
        let (x, _) = fields();
        let xi = c.add_field(x);
        c.emit(Op::PushField(xi));
        c.emit(Op::PushField(xi));
        c.emit(Op::Eq);
        c.emit(Op::StoreField(xi));
        let bc = c.finalize();
        let mut s = TiledScratch::new();
        let base = std::ptr::null_mut::<u8>();
        let r = unsafe { execute_assignment_tiled(&bc, &[base], &[8], 0, &mut s) };
        assert_eq!(r, Err(UnsupportedOp));
    }

    #[test]
    fn malformed_map_programs_are_rejected() {
        assert!(!map_supported(&[Op::Add]));
        assert!(!f32_native_eligible(&[Op::Add]));
        assert!(!map_supported(&[Op::PushConst(0), Op::PushConst(0)]));
        assert!(!map_supported(&[Op::PushField(0)]));

        let mut output = [0.0];
        let destination = ColumnMut::from_f64_slice(&mut output);
        let mut scratch = TiledScratch::new();
        let result =
            unsafe { run_map(&[Op::PushConst(0)], &[], &[], &destination, 1, &mut scratch) };
        assert_eq!(result, Err(UnsupportedOp));
    }

    #[test]
    fn min_max_propagate_nan_like_scalar() {
        // `f64::min/max` return the non-NaN operand; the scalar VM's
        // `python_minimum/maximum` helpers propagate NaN.
        let mut c = Compiler::new();
        let a = c.add_field(FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        });
        let b = c.add_field(FieldId {
            component_id: ComponentId::new(0),
            offset: 8,
            field_type: FieldType::F64,
        });
        // out = min(a, b); reuse `a`'s slot for the store.
        c.emit(Op::PushField(a));
        c.emit(Op::PushField(b));
        c.emit(Op::Min);
        c.emit(Op::StoreField(a));
        let bc = c.finalize();

        // (a, b) pairs exercising NaN in either position and both orders.
        let mut data: Vec<f64> = vec![f64::NAN, 1.0, 2.0, f64::NAN, f64::NAN, f64::NAN, 3.0, -4.0];
        let n = data.len() / 2;
        let base = data.as_mut_ptr() as *mut u8;
        let bases = [base, unsafe { base.add(8) }];
        let strides = [16usize, 16usize];
        let mut s = TiledScratch::new();
        unsafe { execute_assignment_tiled(&bc, &bases, &strides, n, &mut s).unwrap() };

        assert!(data[0].is_nan(), "min(NaN, 1.0) must be NaN");
        assert!(data[2].is_nan(), "min(2.0, NaN) must be NaN");
        assert!(data[4].is_nan(), "min(NaN, NaN) must be NaN");
        assert_eq!(data[6], -4.0, "min(3.0, -4.0) == -4.0");
    }

    /// SplitMix64: a tiny deterministic PRNG so the differential fuzz is reproducible
    /// without pulling in a `rand` dependency.
    struct SplitMix(u64);
    impl SplitMix {
        fn new(seed: u64) -> Self {
            SplitMix(seed)
        }
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next_u64() % n as u64) as usize
        }
        /// A value biased toward IEEE edge cases (NaN, +/-inf, +/-0, subnormal, 1),
        /// with arbitrary finite bit patterns for the rest.
        fn value(&mut self) -> f64 {
            match self.below(12) {
                0 => f64::NAN,
                1 => f64::INFINITY,
                2 => f64::NEG_INFINITY,
                3 => 0.0,
                4 => -0.0,
                5 => f64::MIN_POSITIVE,
                6 => 1.0,
                7 => -1.0,
                _ => {
                    let x = f64::from_bits(self.next_u64());
                    // Keep NaN reachable only through case 0 so equal-bit NaN payloads
                    // don't depend on the RNG's raw bits.
                    if x.is_nan() { 1.5 } else { x }
                }
            }
        }
    }

    /// Emit one random non-push op and return the new stack depth.
    fn emit_random_op(rng: &mut SplitMix, c: &mut Compiler, depth: usize) -> usize {
        let choice = rng.below(100);
        if depth >= 3 && choice < 15 {
            c.emit(if rng.below(2) == 0 {
                Op::Clamp
            } else {
                Op::Lerp
            });
            depth - 2
        } else if depth >= 2 && choice < 60 {
            let op = match rng.below(8) {
                0 => Op::Add,
                1 => Op::Sub,
                2 => Op::Mul,
                3 => Op::Div,
                4 => Op::Min,
                5 => Op::Max,
                6 => Op::Pow,
                _ => Op::Mod,
            };
            c.emit(op);
            depth - 1
        } else {
            let op = match rng.below(18) {
                0 => Op::Neg,
                1 => Op::Sqrt,
                2 => Op::Abs,
                3 => Op::Floor,
                4 => Op::Ceil,
                5 => Op::Round,
                6 => Op::Sign,
                7 => Op::Fract,
                8 => Op::Sin,
                9 => Op::Cos,
                10 => Op::Tan,
                11 => Op::Asin,
                12 => Op::Acos,
                13 => Op::Atan,
                14 => Op::Exp,
                15 => Op::Ln,
                16 => Op::Log10,
                _ => Op::Log2,
            };
            c.emit(op);
            depth
        }
    }

    /// Emit a random stack-valid program over `num_fields` F64 fields.
    fn random_program(rng: &mut SplitMix, num_fields: usize) -> CompiledBytecode {
        let mut c = Compiler::new();
        let fids: Vec<u16> = (0..num_fields)
            .map(|i| {
                c.add_field(FieldId {
                    component_id: ComponentId::new(0),
                    offset: i * 8,
                    field_type: FieldType::F64,
                })
            })
            .collect();
        let consts: Vec<u16> = (0..4).map(|_| c.add_constant(rng.value())).collect();

        let target_len = 6 + rng.below(24);
        let mut depth: usize = 0;
        let mut emitted = 0;
        while emitted < target_len || depth == 0 {
            if depth == 0 || (depth < MAX_DEPTH - 1 && rng.below(100) < 45) {
                if rng.below(2) == 0 {
                    c.emit(Op::PushField(fids[rng.below(num_fields)]));
                } else {
                    c.emit(Op::PushConst(consts[rng.below(consts.len())]));
                }
                depth += 1;
            } else {
                depth = emit_random_op(rng, &mut c, depth);
            }
            emitted += 1;
        }
        while depth > 1 {
            c.emit(Op::Add);
            depth -= 1;
        }
        c.emit(Op::StoreField(fids[rng.below(num_fields)]));
        c.finalize()
    }

    /// For thousands of random supported programs over adversarial f64 inputs, the
    /// serial f64 tiled executor must be bit-for-bit identical to the scalar VM. A
    /// padded row stride exercises the strided (non-contiguous) gather.
    #[test]
    fn differential_f64_tiled_matches_scalar() {
        let num_fields = 3;
        let n = 259; // > one TILE (256) with a ragged tail
        let row_slots = num_fields + 1; // one padding slot per row -> strided gather
        let stride = row_slots * 8;
        let mut rng = SplitMix::new(0xD1FF_C0DE_1234_5678);

        for prog in 0..3000 {
            let bc = random_program(&mut rng, num_fields);
            let mut scalar_buf: Vec<f64> = (0..n * row_slots).map(|_| rng.value()).collect();
            let mut tiled_buf = scalar_buf.clone();

            unsafe {
                let base = scalar_buf.as_mut_ptr() as *mut u8;
                let mut vm = VM::new();
                for i in 0..n {
                    let ptrs: Vec<*mut u8> = (0..num_fields)
                        .map(|f| base.add(i * stride + f * 8))
                        .collect();
                    vm.execute(&bc, &ptrs, i);
                }
            }
            unsafe {
                let base = tiled_buf.as_mut_ptr() as *mut u8;
                let bases: Vec<*mut u8> = (0..num_fields).map(|f| base.add(f * 8)).collect();
                let strides = vec![stride; num_fields];
                let mut s = TiledScratch::new();
                execute_assignment_tiled(&bc, &bases, &strides, n, &mut s).unwrap();
            }

            for (slot, (sc, ti)) in scalar_buf.iter().zip(&tiled_buf).enumerate() {
                // Contract: non-NaN results are bit-for-bit identical; a NaN result is
                // equal to a NaN result regardless of sign/payload. Autovectorized ops
                // can pick a different NaN sign than the scalar op, and NumPy does not
                // guarantee NaN bit patterns either, so only NaN-vs-finite is a bug.
                if sc.is_nan() && ti.is_nan() {
                    continue;
                }
                assert_eq!(
                    sc.to_bits(),
                    ti.to_bits(),
                    "prog {prog}, slot {slot}: scalar {sc:?} ({:#018x}) != tiled {ti:?} ({:#018x})",
                    sc.to_bits(),
                    ti.to_bits(),
                );
            }
        }
    }

    /// Emit a random stack-valid program reading from fields `0..num_read` and storing
    /// into a dedicated write-only field `num_read` (so the store never aliases a source,
    /// letting the test use safe contiguous columns).
    fn random_program_soa(rng: &mut SplitMix, num_read: usize) -> CompiledBytecode {
        let mut c = Compiler::new();
        let out = num_read;
        for i in 0..=num_read {
            c.add_field(FieldId {
                component_id: ComponentId::new(0),
                offset: i * 8,
                field_type: FieldType::F64,
            });
        }
        let consts: Vec<u16> = (0..4).map(|_| c.add_constant(rng.value())).collect();
        let target_len = 6 + rng.below(24);
        let mut depth = 0usize;
        let mut emitted = 0;
        while emitted < target_len || depth == 0 {
            if depth == 0 || (depth < MAX_DEPTH - 1 && rng.below(100) < 45) {
                if rng.below(2) == 0 {
                    c.emit(Op::PushField(rng.below(num_read) as u16));
                } else {
                    c.emit(Op::PushConst(consts[rng.below(consts.len())]));
                }
                depth += 1;
            } else {
                depth = emit_random_op(rng, &mut c, depth);
            }
            emitted += 1;
        }
        while depth > 1 {
            c.emit(Op::Add);
            depth -= 1;
        }
        c.emit(Op::StoreField(out as u16));
        c.finalize()
    }

    /// SoA columns (each field a separate contiguous f64 array) through
    /// `run_assignment` must match the scalar VM bit-for-bit (NaN carve-out),
    /// exercising the contiguous load/store kernels.
    #[test]
    fn differential_f64_contiguous_matches_scalar() {
        let num_read = 3;
        let n = 259;
        let mut rng = SplitMix::new(0xC047_1600_ABCD_0001);

        for prog in 0..2000 {
            let bc = random_program_soa(&mut rng, num_read);
            let mut read_cols: Vec<Vec<f64>> = (0..num_read)
                .map(|_| (0..n).map(|_| rng.value()).collect())
                .collect();
            let mut scalar_out = vec![0.0f64; n];
            let mut tiled_out = vec![0.0f64; n];

            unsafe {
                let mut vm = VM::new();
                for i in 0..n {
                    let mut ptrs: Vec<*mut u8> = read_cols
                        .iter_mut()
                        .map(|c| c.as_mut_ptr().add(i) as *mut u8)
                        .collect();
                    ptrs.push(scalar_out.as_mut_ptr().add(i) as *mut u8);
                    vm.execute(&bc, &ptrs, i);
                }
            }

            let mut srcs: Vec<ColumnRef> = read_cols
                .iter()
                .map(|c| ColumnRef::from_f64_slice(c))
                .collect();
            srcs.push(ColumnRef::broadcast(0.0)); // out-field index, never read
            let dests = [ColumnMut::from_f64_slice(&mut tiled_out)];
            let mut s = TiledScratch::new();
            // SAFETY: all columns are valid contiguous slices of length n; the output is
            // disjoint from every source.
            unsafe { run_assignment(&bc, &srcs, &dests, n, &mut s).unwrap() };

            for (slot, (sc, ti)) in scalar_out.iter().zip(&tiled_out).enumerate() {
                if sc.is_nan() && ti.is_nan() {
                    continue;
                }
                assert_eq!(
                    sc.to_bits(),
                    ti.to_bits(),
                    "prog {prog}, slot {slot}: scalar {sc:?} != tiled {ti:?}",
                );
            }
        }
    }

    /// `in_place_pair` over one contiguous column: `x = x*2 + 1` in place matches the
    /// naive computation (the sanctioned source/destination alias path).
    #[test]
    fn run_assignment_in_place_pair_contiguous() {
        let n = 300;
        let mut c = Compiler::new();
        let xi = c.add_field(FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        });
        let two = c.add_constant(2.0);
        let one = c.add_constant(1.0);
        c.emit(Op::PushField(xi));
        c.emit(Op::PushConst(two));
        c.emit(Op::Mul);
        c.emit(Op::PushConst(one));
        c.emit(Op::Add);
        c.emit(Op::StoreField(xi));
        let bc = c.finalize();

        let init: Vec<f64> = (0..n).map(|i| i as f64 * 0.25 - 3.0).collect();
        let mut got = init.clone();
        {
            // SAFETY: one contiguous run, exclusive for the duration; no other refs to `got`.
            let (cref, cmut) = unsafe {
                crate::columns::in_place_pair(got.as_mut_ptr() as *mut u8, 8, FieldType::F64, n)
            };
            let srcs = [cref];
            let dests = [cmut];
            let mut s = TiledScratch::new();
            // SAFETY: exact same-run in-place alias, the permitted aliasing case.
            unsafe { run_assignment(&bc, &srcs, &dests, n, &mut s).unwrap() };
        }
        for (i, g) in got.iter().enumerate() {
            assert_eq!(*g, init[i] * 2.0 + 1.0, "slot {i}");
        }
    }

    #[test]
    fn f32_native_rounds_after_each_operation() {
        let ops = [
            Op::PushInput(0),
            Op::PushConst(0),
            Op::Add,
            Op::PushConst(0),
            Op::Add,
        ];
        let constants = [1.0];
        let input = [16_777_216.0_f32];
        let source = [ColumnRef::from_f32_slice(&input)];
        let mut native = [0.0_f32];
        let mut widened = [0.0_f32];

        let mut scratch = TiledScratch::new();
        // SAFETY: source and destination are disjoint, live single-element columns.
        unsafe {
            run_map_f32(
                &ops,
                &constants,
                &source,
                &ColumnMut::from_f32_slice(&mut native),
                1,
                &mut scratch,
            )
            .unwrap()
        };
        // SAFETY: source and destination are disjoint, live single-element columns.
        unsafe {
            run_map(
                &ops,
                &constants,
                &source,
                &ColumnMut::from_f32_slice(&mut widened),
                1,
                &mut scratch,
            )
            .unwrap()
        };

        assert_eq!(native, [16_777_216.0]);
        assert_eq!(widened, [16_777_218.0]);
    }

    #[test]
    fn f32_native_narrows_constants_and_broadcasts() {
        let ops = [Op::PushInput(0), Op::PushConst(0), Op::Add];
        let constants = [std::f64::consts::PI];
        let source = [ColumnRef::broadcast(0.1)];
        let mut output = [0.0_f32];
        let destination = ColumnMut::from_f32_slice(&mut output);
        let mut scratch = TiledScratch::new();

        // SAFETY: broadcast has no pointer and destination is a live f32 column.
        unsafe { run_map_f32(&ops, &constants, &source, &destination, 1, &mut scratch).unwrap() };

        assert_eq!(output, [0.1_f32 + std::f32::consts::PI]);
    }

    #[test]
    fn f32_native_rejects_incompatible_columns() {
        let source_values = [1.0_f64, 2.0];
        let source = [ColumnRef::from_f64_slice(&source_values)];
        let mut output = [0.0_f32; 2];
        let destination = ColumnMut::from_f32_slice(&mut output);
        let mut scratch = TiledScratch::new();
        // SAFETY: all supplied columns are valid and disjoint; the dtype is rejected.
        let result = unsafe {
            run_map_f32(
                &[Op::PushInput(0)],
                &[],
                &source,
                &destination,
                2,
                &mut scratch,
            )
        };
        assert_eq!(result, Err(UnsupportedOp));
    }

    #[test]
    fn tiled_scratch_grows_for_deep_valid_programs() {
        let mut compiler = Compiler::new();
        let output = compiler.add_field(FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        });
        let one = compiler.add_constant(1.0);
        for _ in 0..40 {
            compiler.emit(Op::PushConst(one));
        }
        for _ in 1..40 {
            compiler.emit(Op::Add);
        }
        compiler.emit(Op::StoreField(output));
        let bytecode = compiler.finalize();
        assert!(supported_program(&bytecode));

        let mut values = vec![0.0_f64; 3];
        let bases = [values.as_mut_ptr().cast::<u8>()];
        let strides = [size_of::<f64>()];
        let mut scratch = TiledScratch::new();
        unsafe {
            execute_assignment_tiled(&bytecode, &bases, &strides, values.len(), &mut scratch)
                .unwrap()
        };

        assert_eq!(values, vec![40.0; 3]);
    }

    #[test]
    fn tiled_u8_specialization_wraps_without_f64_conversion() {
        let mut compiler = Compiler::new();
        let field = compiler.add_field(FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::U8,
        });
        let ten = compiler.add_constant(10.0);
        compiler.emit(Op::PushField(field));
        compiler.emit(Op::PushConst(ten));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field));
        let bytecode = compiler.finalize();
        assert_eq!(integer_program_type(&bytecode), Some(FieldType::U8));

        let mut values = vec![250_u8; TILE + 3];
        let bases = [values.as_mut_ptr()];
        let strides = [size_of::<u8>()];
        let mut scratch = TiledScratch::new();
        unsafe {
            execute_assignment_tiled(&bytecode, &bases, &strides, values.len(), &mut scratch)
                .unwrap()
        };

        assert!(values.iter().all(|value| *value == 4));
    }

    #[test]
    fn tiled_i64_identity_is_exact_above_f64_precision() {
        let mut compiler = Compiler::new();
        let field = compiler.add_field(FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::I64,
        });
        compiler.emit(Op::PushField(field));
        compiler.emit(Op::StoreField(field));
        let bytecode = compiler.finalize();

        let expected = (1_i64 << 53) + 1;
        let mut values = vec![expected; TILE + 3];
        let bases = [values.as_mut_ptr().cast::<u8>()];
        let strides = [size_of::<i64>()];
        let mut scratch = TiledScratch::new();
        unsafe {
            execute_assignment_tiled(&bytecode, &bases, &strides, values.len(), &mut scratch)
                .unwrap()
        };

        assert!(values.iter().all(|value| *value == expected));
    }

    macro_rules! integer_tiled_matches_scalar {
        ($name:ident, $ty:ty, $field_type:expr, $seeds:expr) => {
            #[test]
            fn $name() {
                let mut compiler = Compiler::new();
                let field = compiler.add_field(FieldId {
                    component_id: ComponentId::new(0),
                    offset: 0,
                    field_type: $field_type,
                });
                let three = compiler.add_constant(3.0);
                let five = compiler.add_constant(5.0);
                let two = compiler.add_constant(2.0);
                compiler.emit(Op::PushField(field));
                compiler.emit(Op::PushConst(three));
                compiler.emit(Op::Mul);
                compiler.emit(Op::PushConst(five));
                compiler.emit(Op::Add);
                compiler.emit(Op::Neg);
                compiler.emit(Op::Abs);
                compiler.emit(Op::PushConst(two));
                compiler.emit(Op::Max);
                compiler.emit(Op::StoreField(field));
                let bytecode = compiler.finalize();
                assert_eq!(integer_program_type(&bytecode), Some($field_type));

                let seeds: &[$ty] = &$seeds;
                let initial: Vec<$ty> = (0..TILE + 3)
                    .map(|index| seeds[index % seeds.len()])
                    .collect();
                let mut scalar = initial.clone();
                let mut vm = VM::new();
                for (index, value) in scalar.iter_mut().enumerate() {
                    let pointers = [(value as *mut $ty).cast::<u8>()];
                    // SAFETY: the pointer names one live value with the bytecode's
                    // declared type and is exclusively borrowed for this call.
                    unsafe { vm.execute(&bytecode, &pointers, index) };
                }

                let mut tiled = initial;
                let bases = [tiled.as_mut_ptr().cast::<u8>()];
                let strides = [size_of::<$ty>()];
                let mut scratch = TiledScratch::new();
                // SAFETY: `tiled` is one exclusive strided run of the declared type.
                unsafe {
                    execute_assignment_tiled(&bytecode, &bases, &strides, tiled.len(), &mut scratch)
                        .unwrap()
                };

                assert_eq!(tiled, scalar);
            }
        };
    }

    integer_tiled_matches_scalar!(
        tiled_i32_matches_scalar,
        i32,
        FieldType::I32,
        [i32::MIN, -17, -1, 0, 1, 23, i32::MAX]
    );
    integer_tiled_matches_scalar!(
        tiled_i64_matches_scalar,
        i64,
        FieldType::I64,
        [
            i64::MIN,
            -(1_i64 << 53) - 1,
            -1,
            0,
            1,
            (1_i64 << 53) + 1,
            i64::MAX
        ]
    );
    integer_tiled_matches_scalar!(
        tiled_u8_matches_scalar,
        u8,
        FieldType::U8,
        [0, 1, 17, 127, 250, u8::MAX]
    );
    integer_tiled_matches_scalar!(
        tiled_u32_matches_scalar,
        u32,
        FieldType::U32,
        [0, 1, 17, u32::MAX / 2, u32::MAX]
    );
    integer_tiled_matches_scalar!(
        tiled_u64_matches_scalar,
        u64,
        FieldType::U64,
        [0, 1, (1_u64 << 53) + 1, u64::MAX / 2, u64::MAX]
    );

    #[test]
    fn malformed_integer_indices_are_rejected() {
        let missing_field = CompiledBytecode {
            bytecode: vec![Op::PushField(0), Op::StoreField(0)],
            constants: vec![],
            field_map: vec![],
        };
        assert!(!supported_program(&missing_field));

        let field = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::I64,
        };
        let missing_constant = CompiledBytecode {
            bytecode: vec![
                Op::PushField(0),
                Op::PushConst(0),
                Op::Add,
                Op::StoreField(0),
            ],
            constants: vec![],
            field_map: vec![field],
        };
        assert!(!supported_program(&missing_constant));
    }
}
