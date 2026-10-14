//! Bytecode VM for executing lazy expression trees in batch operations.
//!
//! This module provides a stack-based bytecode interpreter optimized for
//! executing mathematical expressions on component fields in tight loops.
//!
//! # Performance
//!
//! The bytecode VM is designed for minimal overhead (~2-5ns per operation):
//! - Linear bytecode array (perfect cache locality)
//! - Simple stack operations (no recursion)
//! - Read-only bytecode (perfect for par_iter_mut)
//! - Pre-resolved component field offsets
//!
//! # Example Bytecode
//!
//! Expression: `pos.x = pos.x + vel.x * dt`
//!
//! Bytecode:
//! ```text
//! PushField(0)      // Push pos.x
//! PushField(1)      // Push vel.x
//! PushConst(0)      // Push dt from constant pool
//! Mul               // vel.x * dt
//! Add               // pos.x + (vel.x * dt)
//! StoreField(0)     // Store to pos.x
//! ```

use std::{cmp::Ordering, collections::HashMap};

use bevy_ecs::component::ComponentId;
use pybevy_storage::storage_error::expression_too_large;

/// Value types that can be stored on the VM stack
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StackValue {
    /// Native 32-bit floating-point number.
    F32(f32),
    /// 64-bit floating-point number.
    Float(f64),
    /// Python `int` literal. Like NumPy's weak Python scalars it adopts the
    /// other operand's lane type, including exact integer lanes.
    Constant(f64),
    /// Python `float` literal. It adopts an `f32` lane but promotes an
    /// integer operand to f64, as NumPy's weak float scalars do.
    FloatConstant(f64),
    /// Python `int` literal in the `i64` range that f64 cannot hold exactly.
    ExactInt(i64),
    /// Python `int` literal above `i64::MAX` that f64 cannot hold exactly.
    ExactUInt(u64),
    /// Signed 32-bit integer.
    I32(i32),
    /// Signed 64-bit integer.
    I64(i64),
    /// Unsigned 8-bit integer.
    U8(u8),
    /// Unsigned 32-bit integer.
    U32(u32),
    /// Unsigned 64-bit integer.
    U64(u64),
    /// Boolean value
    Bool(bool),
}

impl StackValue {
    /// Convert a numeric value to f64, panicking for booleans.
    #[inline]
    pub fn as_float(&self) -> f64 {
        match self {
            StackValue::F32(f) => f64::from(*f),
            StackValue::Float(f) | StackValue::Constant(f) | StackValue::FloatConstant(f) => *f,
            StackValue::I32(v) => f64::from(*v),
            StackValue::ExactInt(v) => *v as f64,
            StackValue::ExactUInt(v) => *v as f64,
            StackValue::I64(v) => *v as f64,
            StackValue::U8(v) => f64::from(*v),
            StackValue::U32(v) => f64::from(*v),
            StackValue::U64(v) => *v as f64,
            StackValue::Bool(_) => panic!("Expected float, got bool"),
        }
    }

    /// Unwrap as bool, panic if not a bool
    #[inline]
    pub fn as_bool(&self) -> bool {
        match self {
            StackValue::Bool(b) => *b,
            StackValue::F32(_)
            | StackValue::Float(_)
            | StackValue::Constant(_)
            | StackValue::FloatConstant(_)
            | StackValue::ExactInt(_)
            | StackValue::ExactUInt(_)
            | StackValue::I32(_)
            | StackValue::I64(_)
            | StackValue::U8(_)
            | StackValue::U32(_)
            | StackValue::U64(_) => panic!("Expected bool, got number"),
        }
    }

    /// An exact int literal as the stack value that holds it: an f64
    /// `Constant` when exact, otherwise the `i64`/`u64` exact variants.
    pub fn int_literal(value: i128) -> Self {
        if let Ok(value) = i64::try_from(value)
            && (value as f64) as i128 != i128::from(value)
        {
            return StackValue::ExactInt(value);
        }
        if let Ok(value) = u64::try_from(value)
            && (value as f64) as i128 != i128::from(value)
        {
            return StackValue::ExactUInt(value);
        }
        StackValue::Constant(value as f64)
    }

    /// The exact integer an integer lane or int literal holds.
    #[inline]
    fn integer_value(self) -> Option<i128> {
        match self {
            StackValue::I32(value) => Some(value.into()),
            StackValue::I64(value) | StackValue::ExactInt(value) => Some(value.into()),
            StackValue::U8(value) => Some(value.into()),
            StackValue::U32(value) => Some(value.into()),
            StackValue::U64(value) | StackValue::ExactUInt(value) => Some(value.into()),
            StackValue::Constant(value) => Some(value as i128),
            StackValue::F32(_)
            | StackValue::Float(_)
            | StackValue::FloatConstant(_)
            | StackValue::Bool(_) => None,
        }
    }

    #[inline]
    fn coerce_exact(value: i128, template: Self) -> Option<Self> {
        Some(match template {
            StackValue::F32(_) => StackValue::F32(value as f32),
            StackValue::Float(_) | StackValue::FloatConstant(_) => {
                StackValue::FloatConstant(value as f64)
            }
            StackValue::I32(_) => StackValue::I32(i32::try_from(value).ok()?),
            StackValue::I64(_) => StackValue::I64(i64::try_from(value).ok()?),
            StackValue::U8(_) => StackValue::U8(u8::try_from(value).ok()?),
            StackValue::U32(_) => StackValue::U32(u32::try_from(value).ok()?),
            StackValue::U64(_) => StackValue::U64(u64::try_from(value).ok()?),
            _ => return None,
        })
    }

    #[inline]
    fn coerce_constant(self, template: Self) -> Option<Self> {
        let value = match self {
            StackValue::ExactInt(value) => return Self::coerce_exact(value.into(), template),
            StackValue::ExactUInt(value) => return Self::coerce_exact(value.into(), template),
            StackValue::Constant(value) => value,
            StackValue::FloatConstant(value) if matches!(template, StackValue::F32(_)) => value,
            _ => return None,
        };
        if matches!(template, StackValue::F32(_)) {
            return Some(StackValue::F32(value as f32));
        }
        if !value.is_finite() || value.fract() != 0.0 {
            return None;
        }
        match template {
            StackValue::I32(_) if value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX) => {
                Some(StackValue::I32(value as i32))
            }
            StackValue::I64(_) if value >= i64::MIN as f64 && value < -(i64::MIN as f64) => {
                Some(StackValue::I64(value as i64))
            }
            StackValue::U8(_) if value >= 0.0 && value <= f64::from(u8::MAX) => {
                Some(StackValue::U8(value as u8))
            }
            StackValue::U32(_) if value >= 0.0 && value <= f64::from(u32::MAX) => {
                Some(StackValue::U32(value as u32))
            }
            StackValue::U64(_) if value >= 0.0 && value < 2_f64.powi(64) => {
                Some(StackValue::U64(value as u64))
            }
            _ => None,
        }
    }
}

#[inline]
fn specialize_constants(a: StackValue, b: StackValue) -> (StackValue, StackValue) {
    if let Some(a) = a.coerce_constant(b) {
        (a, b)
    } else if let Some(b) = b.coerce_constant(a) {
        (a, b)
    } else {
        (a, b)
    }
}

#[inline]
fn cmp_i64_f64(integer: i64, float: f64) -> Option<Ordering> {
    if float.is_nan() {
        return None;
    }
    if float < i64::MIN as f64 {
        return Some(Ordering::Greater);
    }
    if float >= -(i64::MIN as f64) {
        return Some(Ordering::Less);
    }

    let truncated = float as i64;
    match integer.cmp(&truncated) {
        Ordering::Equal if float.fract().is_sign_positive() && float.fract() != 0.0 => {
            Some(Ordering::Less)
        }
        Ordering::Equal if float.fract().is_sign_negative() && float.fract() != 0.0 => {
            Some(Ordering::Greater)
        }
        ordering => Some(ordering),
    }
}

#[inline]
fn cmp_u64_f64(integer: u64, float: f64) -> Option<Ordering> {
    if float.is_nan() {
        return None;
    }
    if float < 0.0 {
        return Some(Ordering::Greater);
    }
    if float >= 2_f64.powi(64) {
        return Some(Ordering::Less);
    }

    let truncated = float as u64;
    match integer.cmp(&truncated) {
        Ordering::Equal if float.fract() != 0.0 => Some(Ordering::Less),
        ordering => Some(ordering),
    }
}

/// Where a value came from, ordered so that combining two origins keeps the
/// greater one: literal-only inputs stay literals a later operand can specialize.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Origin {
    IntLiteral,
    FloatLiteral,
    Runtime,
}

impl Origin {
    #[inline]
    fn of(value: StackValue) -> Self {
        match value {
            StackValue::Constant(_) | StackValue::ExactInt(_) | StackValue::ExactUInt(_) => {
                Self::IntLiteral
            }
            StackValue::FloatConstant(_) => Self::FloatLiteral,
            _ => Self::Runtime,
        }
    }

    #[inline]
    fn of_all(values: &[StackValue]) -> Self {
        values
            .iter()
            .map(|value| Self::of(*value))
            .max()
            .unwrap_or(Self::IntLiteral)
    }

    /// The origin of a float-valued result computed from these inputs.
    #[inline]
    fn floating(self) -> Self {
        self.max(Self::FloatLiteral)
    }
}

/// Result of an op whose operands did not share a native lane.
#[inline]
fn numeric_result(origin: Origin, value: f64) -> StackValue {
    match origin {
        Origin::IntLiteral => StackValue::Constant(value),
        Origin::FloatLiteral => StackValue::FloatConstant(value),
        Origin::Runtime => StackValue::Float(value),
    }
}

macro_rules! signed_floor_divmod {
    ($name:ident, $ty:ty) => {
        #[inline]
        fn $name(a: $ty, b: $ty) -> ($ty, $ty) {
            if b == 0 {
                return (0, 0);
            }
            let quotient = a.wrapping_div(b);
            let remainder = a.wrapping_rem(b);
            if remainder != 0 && (remainder < 0) != (b < 0) {
                (quotient - 1, remainder + b)
            } else {
                (quotient, remainder)
            }
        }
    };
}

signed_floor_divmod!(floor_divmod_i32, i32);
signed_floor_divmod!(floor_divmod_i64, i64);

#[inline]
fn floor_divide(a: StackValue, b: StackValue) -> StackValue {
    let (a, b) = specialize_constants(a, b);
    match (a, b) {
        (StackValue::I32(a), StackValue::I32(b)) => StackValue::I32(floor_divmod_i32(a, b).0),
        (StackValue::I64(a), StackValue::I64(b)) => StackValue::I64(floor_divmod_i64(a, b).0),
        (StackValue::U8(a), StackValue::U8(b)) => StackValue::U8(a.checked_div(b).unwrap_or(0)),
        (StackValue::U32(a), StackValue::U32(b)) => StackValue::U32(a.checked_div(b).unwrap_or(0)),
        (StackValue::U64(a), StackValue::U64(b)) => StackValue::U64(a.checked_div(b).unwrap_or(0)),
        (StackValue::F32(a), StackValue::F32(b)) => StackValue::F32(numpy_floor_divide_f32(a, b)),
        (a, b) => numeric_result(
            Origin::of_all(&[a, b]),
            numpy_floor_divide(a.as_float(), b.as_float()),
        ),
    }
}

#[inline]
fn remainder(a: StackValue, b: StackValue) -> StackValue {
    let (a, b) = specialize_constants(a, b);
    match (a, b) {
        (StackValue::I32(a), StackValue::I32(b)) => StackValue::I32(floor_divmod_i32(a, b).1),
        (StackValue::I64(a), StackValue::I64(b)) => StackValue::I64(floor_divmod_i64(a, b).1),
        (StackValue::U8(a), StackValue::U8(b)) => StackValue::U8(a.checked_rem(b).unwrap_or(0)),
        (StackValue::U32(a), StackValue::U32(b)) => StackValue::U32(a.checked_rem(b).unwrap_or(0)),
        (StackValue::U64(a), StackValue::U64(b)) => StackValue::U64(a.checked_rem(b).unwrap_or(0)),
        (StackValue::F32(a), StackValue::F32(b)) => StackValue::F32(python_remainder_f32(a, b)),
        (a, b) => numeric_result(
            Origin::of_all(&[a, b]),
            python_remainder(a.as_float(), b.as_float()),
        ),
    }
}

macro_rules! signed_power {
    ($name:ident, $ty:ty) => {
        #[inline]
        fn $name(base: $ty, exponent: $ty) -> $ty {
            if exponent < 0 {
                return match base {
                    1 => 1,
                    -1 if exponent & 1 == 0 => 1,
                    -1 => -1,
                    _ => 0,
                };
            }
            unsigned_power_body!(base, exponent, $ty)
        }
    };
}

macro_rules! unsigned_power {
    ($name:ident, $ty:ty) => {
        #[inline]
        fn $name(base: $ty, exponent: $ty) -> $ty {
            unsigned_power_body!(base, exponent, $ty)
        }
    };
}

macro_rules! unsigned_power_body {
    ($base:expr, $exponent:expr, $ty:ty) => {{
        let (mut base, mut exponent, mut result): ($ty, $ty, $ty) = ($base, $exponent, 1);
        while exponent > 0 {
            if exponent & 1 == 1 {
                result = result.wrapping_mul(base);
            }
            base = base.wrapping_mul(base);
            exponent >>= 1;
        }
        result
    }};
}

signed_power!(power_i32, i32);
signed_power!(power_i64, i64);
unsigned_power!(power_u8, u8);
unsigned_power!(power_u32, u32);
unsigned_power!(power_u64, u64);

/// NumPy `power`: integer lanes wrap on overflow. The flag reports a signed
/// integer lane raised to a negative power, which NumPy rejects; the value
/// returned for it is truncated toward zero (0 unless the base is 1 or -1).
#[inline]
fn power(a: StackValue, b: StackValue) -> (StackValue, bool) {
    let (a, b) = specialize_constants(a, b);
    match (a, b) {
        (StackValue::I32(a), StackValue::I32(b)) => (StackValue::I32(power_i32(a, b)), b < 0),
        (StackValue::I64(a), StackValue::I64(b)) => (StackValue::I64(power_i64(a, b)), b < 0),
        (StackValue::U8(a), StackValue::U8(b)) => (StackValue::U8(power_u8(a, b)), false),
        (StackValue::U32(a), StackValue::U32(b)) => (StackValue::U32(power_u32(a, b)), false),
        (StackValue::U64(a), StackValue::U64(b)) => (StackValue::U64(power_u64(a, b)), false),
        (StackValue::F32(a), StackValue::F32(b)) => (StackValue::F32(a.powf(b)), false),
        (a, b) => {
            let origin = match Origin::of_all(&[a, b]) {
                Origin::IntLiteral if b.as_float() >= 0.0 => Origin::IntLiteral,
                origin => origin.floating(),
            };
            (
                numeric_result(origin, a.as_float().powf(b.as_float())),
                false,
            )
        }
    }
}

/// A row raised an integer to a negative integer power, which NumPy rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegativeIntegerPower;

impl std::fmt::Display for NegativeIntegerPower {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Integers to negative integer powers are not allowed.")
    }
}

impl std::error::Error for NegativeIntegerPower {}

macro_rules! integer_binary {
    ($a:expr, $b:expr, $method:ident, $float:expr) => {{
        let (a, b) = specialize_constants($a, $b);
        match (a, b) {
            (StackValue::F32(a), StackValue::F32(b)) => StackValue::F32($float(a, b)),
            (StackValue::I32(a), StackValue::I32(b)) => StackValue::I32(a.$method(b)),
            (StackValue::I64(a), StackValue::I64(b)) => StackValue::I64(a.$method(b)),
            (StackValue::U8(a), StackValue::U8(b)) => StackValue::U8(a.$method(b)),
            (StackValue::U32(a), StackValue::U32(b)) => StackValue::U32(a.$method(b)),
            (StackValue::U64(a), StackValue::U64(b)) => StackValue::U64(a.$method(b)),
            (a, b) => numeric_result(Origin::of_all(&[a, b]), $float(a.as_float(), b.as_float())),
        }
    }};
}

macro_rules! float_binary {
    ($a:expr, $b:expr, $f32:expr, $f64:expr) => {{
        let (a, b) = specialize_constants($a, $b);
        match (a, b) {
            (StackValue::F32(a), StackValue::F32(b)) => StackValue::F32($f32(a, b)),
            (a, b) => numeric_result(
                Origin::of_all(&[a, b]).floating(),
                $f64(a.as_float(), b.as_float()),
            ),
        }
    }};
}

macro_rules! float_unary {
    ($value:expr, $f32:expr, $f64:expr) => {{
        match $value {
            StackValue::F32(value) => StackValue::F32($f32(value)),
            value => numeric_result(Origin::of(value).floating(), $f64(value.as_float())),
        }
    }};
}

#[derive(Clone, Copy)]
enum ExactInteger {
    Signed(i64),
    Unsigned(u64),
}

impl ExactInteger {
    #[inline]
    fn from_stack(value: StackValue) -> Option<Self> {
        match value {
            StackValue::I32(value) => Some(Self::Signed(i64::from(value))),
            StackValue::I64(value) => Some(Self::Signed(value)),
            StackValue::U8(value) => Some(Self::Unsigned(u64::from(value))),
            StackValue::U32(value) => Some(Self::Unsigned(u64::from(value))),
            StackValue::U64(value) => Some(Self::Unsigned(value)),
            StackValue::ExactInt(value) => Some(Self::Signed(value)),
            StackValue::ExactUInt(value) => Some(Self::Unsigned(value)),
            StackValue::F32(_)
            | StackValue::Float(_)
            | StackValue::Constant(_)
            | StackValue::FloatConstant(_)
            | StackValue::Bool(_) => None,
        }
    }

    #[inline]
    fn cmp(self, other: Self) -> Ordering {
        match (self, other) {
            (Self::Signed(a), Self::Signed(b)) => a.cmp(&b),
            (Self::Unsigned(a), Self::Unsigned(b)) => a.cmp(&b),
            (Self::Signed(a), Self::Unsigned(_)) if a < 0 => Ordering::Less,
            (Self::Signed(a), Self::Unsigned(b)) => (a as u64).cmp(&b),
            (Self::Unsigned(_), Self::Signed(b)) if b < 0 => Ordering::Greater,
            (Self::Unsigned(a), Self::Signed(b)) => a.cmp(&(b as u64)),
        }
    }

    #[inline]
    fn cmp_float(self, float: f64) -> Option<Ordering> {
        match self {
            Self::Signed(value) => cmp_i64_f64(value, float),
            Self::Unsigned(value) => cmp_u64_f64(value, float),
        }
    }
}

#[inline]
fn numeric_cmp(a: StackValue, b: StackValue) -> Option<Ordering> {
    let (a, b) = specialize_constants(a, b);
    let a_integer = ExactInteger::from_stack(a);
    let b_integer = ExactInteger::from_stack(b);
    if let (Some(a), Some(b)) = (a_integer, b_integer) {
        return Some(a.cmp(b));
    }

    match (a, b) {
        (StackValue::Bool(a), StackValue::Bool(b)) => a.partial_cmp(&b),
        (StackValue::F32(a), StackValue::F32(b)) => a.partial_cmp(&b),
        (
            StackValue::F32(a),
            StackValue::Float(b) | StackValue::Constant(b) | StackValue::FloatConstant(b),
        ) => f64::from(a).partial_cmp(&b),
        (
            StackValue::Float(a) | StackValue::Constant(a) | StackValue::FloatConstant(a),
            StackValue::F32(b),
        ) => a.partial_cmp(&f64::from(b)),
        (
            StackValue::Float(a) | StackValue::Constant(a) | StackValue::FloatConstant(a),
            StackValue::Float(b) | StackValue::Constant(b) | StackValue::FloatConstant(b),
        ) => a.partial_cmp(&b),
        (_, StackValue::Float(b) | StackValue::Constant(b) | StackValue::FloatConstant(b)) => {
            a_integer?.cmp_float(b)
        }
        (_, StackValue::F32(b)) => a_integer?.cmp_float(f64::from(b)),
        (StackValue::Float(a) | StackValue::Constant(a) | StackValue::FloatConstant(a), _) => {
            b_integer?.cmp_float(a).map(Ordering::reverse)
        }
        (StackValue::F32(a), _) => b_integer?.cmp_float(f64::from(a)).map(Ordering::reverse),
        (a, b) => a.as_float().partial_cmp(&b.as_float()),
    }
}

/// Bytecode instruction for the stack-based VM
#[derive(Debug, Clone, Copy)]
pub enum Op {
    /// Push a component field value onto the stack
    /// u16 is an index into the field map
    PushField(u16),

    /// Push a dense input column value onto the stack.
    /// u16 is an index into the caller-provided input columns.
    PushInput(u16),

    /// Push a Python `int` literal from the constant pool onto the stack
    /// u16 is an index into the constants array
    PushConst(u16),

    /// Push a Python `float` literal from the constant pool onto the stack
    /// u16 is an index into the constants array
    PushFloatConst(u16),

    /// Push a Python `int` literal that f64 cannot hold exactly
    /// u16 is an index into the integers array
    PushInt(u16),

    /// Pop two values, add them, push result
    Add,

    /// Pop two values, subtract (stack[n-1] - stack[n]), push result
    Sub,

    /// Pop two values, multiply them, push result
    Mul,

    /// Pop two values, divide (stack[n-1] / stack[n]), push result
    Div,

    /// Pop two values, floor-divide (stack[n-1] // stack[n]), push result
    FloorDiv,

    /// Pop two values, raise to power (stack[n-1] ** stack[n]), push result
    Pow,

    /// Pop value and store to a component field
    /// u16 is an index into the field map
    StoreField(u16),

    /// Negate the top stack value (unary minus)
    Neg,

    /// Pop value, push sin(value)
    Sin,

    /// Pop value, push cos(value)
    Cos,

    /// Pop value, push tan(value)
    Tan,

    /// Pop value, push asin(value)
    Asin,

    /// Pop value, push acos(value)
    Acos,

    /// Pop value, push atan(value)
    Atan,

    /// Pop value, push sqrt(value)
    Sqrt,

    /// Pop value, push abs(value)
    Abs,

    /// Pop value, push floor(value)
    Floor,

    /// Pop value, push ceil(value)
    Ceil,

    /// Pop value, push ties-to-even round(value)
    Round,

    /// Pop two values, push NaN-propagating min(a, b)
    Min,

    /// Pop two values, push NaN-propagating max(a, b)
    Max,

    /// Pop three values (max, min, value), push NumPy-style clip(value, min, max)
    Clamp,

    /// Pop two floats, push bool (a == b)
    Eq,

    /// Pop two floats, push bool (a != b)
    Ne,

    /// Pop two floats, push bool (a < b)
    Lt,

    /// Pop two floats, push bool (a <= b)
    Le,

    /// Pop two floats, push bool (a > b)
    Gt,

    /// Pop two floats, push bool (a >= b)
    Ge,

    /// Conditional selection: where(condition, true_value, false_value)
    /// Stack before: [false_value: float, true_value: float, condition: bool]
    /// Stack after: [result: float]
    Where,

    /// Pop two bools, push (a && b)
    And,

    /// Pop two bools, push (a || b)
    Or,

    /// Pop one bool, push (!a)
    Not,

    /// Pop value, push e^value
    Exp,

    /// Pop value, push ln(value) (natural logarithm)
    Ln,

    /// Pop value, push log10(value)
    Log10,

    /// Pop value, push log2(value)
    Log2,

    /// Pop value, push the Python-facing sign of the value.
    Sign,

    /// Linear interpolation: lerp(a, b, t) = a + t * (b - a)
    /// Stack before: [t: float, b: float, a: float]
    /// Stack after: [result: float]
    Lerp,

    /// Pop value, push fract(value) (fractional part)
    Fract,

    /// Python modulo operation: a % b
    /// Stack before: [b: float, a: float]
    /// Stack after: [result: float]
    Mod,

    /// Generate random float in [0.0, 1.0)
    /// Uses entity identity and the stream for deterministic per-entity randomness
    Random(u32),

    /// Generate random float in [min, max)
    /// Stack before: [max: float, min: float]
    /// Stack after: [result: float]
    RandomRange(u32),
}

pub use pybevy_storage::FieldType;

/// Identifies a specific field within a component
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldId {
    /// The Bevy ComponentId
    pub component_id: ComponentId,
    /// Byte offset within the component struct
    pub offset: usize,
    /// Type of the field
    pub field_type: FieldType,
}

/// Compiled bytecode ready for execution
#[derive(Debug, Clone)]
pub struct CompiledBytecode {
    /// The bytecode instructions
    pub bytecode: Vec<Op>,
    /// Constant pool (f64 values referenced by PushConst)
    pub constants: Vec<f64>,
    /// Exact integer pool referenced by PushInt
    pub integers: Vec<i128>,
    /// Maps field indices (used in bytecode) to actual component field locations
    pub field_map: Vec<FieldId>,
}

/// Compiler that converts expression AST to bytecode
pub struct Compiler {
    bytecode: Vec<Op>,
    constants: Vec<f64>,
    constant_map: HashMap<u64, u16>, // f64 bits -> constant pool index
    integers: Vec<i128>,
    field_map: Vec<FieldId>,
    field_index_map: HashMap<FieldId, u16>, // FieldId -> field map index
    overflowed: bool,
}

/// A program referenced more distinct literals or fields than its `u16`
/// instruction indices can address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramTooLarge;

impl std::fmt::Display for ProgramTooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&expression_too_large())
    }
}

impl std::error::Error for ProgramTooLarge {}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    /// Create a new bytecode compiler
    pub fn new() -> Self {
        Self {
            bytecode: Vec::new(),
            constants: Vec::new(),
            constant_map: HashMap::new(),
            integers: Vec::new(),
            field_map: Vec::new(),
            field_index_map: HashMap::new(),
            overflowed: false,
        }
    }

    /// Index for the next entry of a pool holding `len` entries. On overflow
    /// the program is marked too large and a placeholder index is returned.
    fn next_index(&mut self, len: usize) -> Option<u16> {
        let index = u16::try_from(len).ok();
        self.overflowed |= index.is_none();
        index
    }

    /// Emit a bytecode instruction
    pub fn emit(&mut self, op: Op) {
        self.bytecode.push(op);
    }

    /// Add a constant to the pool and return its index
    pub fn add_constant(&mut self, value: f64) -> u16 {
        // Use bit representation for HashMap key (handles NaN/±0.0 correctly)
        let bits = value.to_bits();

        if let Some(&index) = self.constant_map.get(&bits) {
            return index;
        }

        let Some(index) = self.next_index(self.constants.len()) else {
            return 0;
        };
        self.constants.push(value);
        self.constant_map.insert(bits, index);
        index
    }

    /// Add an exact Python `int` literal and return the instruction pushing it:
    /// `PushConst` when f64 holds it exactly, otherwise `PushInt`.
    pub fn int_literal(&mut self, value: i128) -> Op {
        if (value as f64) as i128 == value {
            return Op::PushConst(self.add_constant(value as f64));
        }
        if let Some(index) = self.integers.iter().position(|existing| *existing == value) {
            return Op::PushInt(index as u16);
        }
        let Some(index) = self.next_index(self.integers.len()) else {
            return Op::PushInt(0);
        };
        self.integers.push(value);
        Op::PushInt(index)
    }

    /// Replace `PushInt` with the nearest f64 `PushConst`, for executors
    /// whose numeric domain is floating point.
    pub fn lower_integers_to_f64(&mut self) {
        for index in 0..self.bytecode.len() {
            if let Op::PushInt(integer) = self.bytecode[index] {
                let value = self.integers[usize::from(integer)] as f64;
                self.bytecode[index] = Op::PushConst(self.add_constant(value));
            }
        }
    }

    /// Add a field to the field map and return its index
    pub fn add_field(&mut self, field_id: FieldId) -> u16 {
        if let Some(&index) = self.field_index_map.get(&field_id) {
            return index;
        }

        let Some(index) = self.next_index(self.field_map.len()) else {
            return 0;
        };
        self.field_map.push(field_id);
        self.field_index_map.insert(field_id, index);
        index
    }

    /// Finalize compilation, rejecting a program whose pools overflowed.
    pub fn try_finalize(self) -> Result<CompiledBytecode, ProgramTooLarge> {
        if self.overflowed {
            return Err(ProgramTooLarge);
        }
        Ok(CompiledBytecode {
            bytecode: self.bytecode,
            constants: self.constants,
            integers: self.integers,
            field_map: self.field_map,
        })
    }

    /// Finalize a program known to fit its pools.
    ///
    /// # Panics
    /// If more than 65,536 distinct literals or fields were added. Use
    /// [`Compiler::try_finalize`] for user expressions.
    pub fn finalize(self) -> CompiledBytecode {
        self.try_finalize().expect("program fits its u16 pools")
    }

    /// Simple peephole optimization pass: fold an arithmetic op whose two
    /// operands are literals into one literal of the resulting kind.
    pub fn optimize(&mut self) {
        let mut i = 0;
        while i + 1 < self.bytecode.len() {
            let Some(a) = self.literal_operand(self.bytecode[i]) else {
                i += 1;
                continue;
            };
            let folded = match fold_unary(self.bytecode[i + 1], a) {
                Some(value) => Some((value, 2)),
                None => self
                    .literal_operand(self.bytecode[i + 1])
                    .zip(self.bytecode.get(i + 2).copied())
                    .and_then(|(b, op)| fold_literals(op, a, b))
                    .map(|value| (value, 3)),
            };
            let Some((folded, width)) = folded else {
                i += 1;
                continue;
            };
            self.bytecode[i] = match folded {
                Literal::Int(value) => self.int_literal(value),
                Literal::Float(value) => Op::PushFloatConst(self.add_constant(value)),
            };
            self.bytecode.drain(i + 1..i + width);
            i = i.saturating_sub(1);
        }
    }
}

/// A literal operand seen by the constant folder.
#[derive(Clone, Copy)]
enum Literal {
    Int(i128),
    Float(f64),
}

impl Literal {
    fn as_f64(self) -> f64 {
        match self {
            Literal::Int(value) => value as f64,
            Literal::Float(value) => value,
        }
    }
}

impl Compiler {
    fn literal_operand(&self, op: Op) -> Option<Literal> {
        match op {
            Op::PushConst(index) => {
                let value = self.constants[usize::from(index)];
                exact_i128(value).map(Literal::Int)
            }
            Op::PushInt(index) => Some(Literal::Int(self.integers[usize::from(index)])),
            Op::PushFloatConst(index) => Some(Literal::Float(self.constants[usize::from(index)])),
            _ => None,
        }
    }
}

/// The exact integer an integral f64 holds, if it fits `i128`.
pub fn exact_i128(value: f64) -> Option<i128> {
    (value.is_finite() && value.fract() == 0.0 && value.abs() < 2_f64.powi(127))
        .then_some(value as i128)
}

/// Fold a unary `op` applied to a literal, as the VM evaluates it.
fn fold_unary(op: Op, a: Literal) -> Option<Literal> {
    Some(match (op, a) {
        (Op::Neg, Literal::Int(value)) => Literal::Int(value.checked_neg()?),
        (Op::Abs, Literal::Int(value)) => Literal::Int(value.checked_abs()?),
        (Op::Sign, Literal::Int(value)) => Literal::Int(value.signum()),
        (Op::Floor | Op::Ceil | Op::Round, Literal::Int(value)) => Literal::Int(value),
        (Op::Neg, Literal::Float(value)) => Literal::Float(-value),
        (Op::Abs, Literal::Float(value)) => Literal::Float(value.abs()),
        (Op::Sign, Literal::Float(value)) => Literal::Float(python_sign(value)),
        (Op::Floor, Literal::Float(value)) => Literal::Float(value.floor()),
        (Op::Ceil, Literal::Float(value)) => Literal::Float(value.ceil()),
        (Op::Round, Literal::Float(value)) => Literal::Float(python_round(value)),
        _ => return None,
    })
}

/// Python's floor quotient and remainder, or `None` for a zero divisor or
/// an overflowing quotient.
fn floor_divmod_i128(a: i128, b: i128) -> Option<(i128, i128)> {
    let quotient = a.checked_div(b)?;
    let remainder = a.checked_rem(b)?;
    Some(if remainder != 0 && (remainder < 0) != (b < 0) {
        (quotient - 1, remainder + b)
    } else {
        (quotient, remainder)
    })
}

/// Fold `a op b` for two literals into a literal of the resulting kind, as the
/// VM evaluates it. Integer arithmetic folds exactly; integer powers only while
/// non-negative; division by a zero literal is left to the VM.
fn fold_literals(op: Op, a: Literal, b: Literal) -> Option<Literal> {
    if let (Literal::Int(a), Literal::Int(b)) = (a, b) {
        return match op {
            Op::Add => a.checked_add(b).map(Literal::Int),
            Op::Sub => a.checked_sub(b).map(Literal::Int),
            Op::Mul => a.checked_mul(b).map(Literal::Int),
            Op::Min => Some(Literal::Int(a.min(b))),
            Op::Max => Some(Literal::Int(a.max(b))),
            Op::FloorDiv => floor_divmod_i128(a, b).map(|(quotient, _)| Literal::Int(quotient)),
            Op::Mod => floor_divmod_i128(a, b).map(|(_, remainder)| Literal::Int(remainder)),
            Op::Div if b != 0 => Some(Literal::Float(a as f64 / b as f64)),
            Op::Pow => a.checked_pow(u32::try_from(b).ok()?).map(Literal::Int),
            _ => None,
        };
    }
    let (a, b) = (a.as_f64(), b.as_f64());
    Some(Literal::Float(match op {
        Op::Add => a + b,
        Op::Sub => a - b,
        Op::Mul => a * b,
        Op::Min => python_minimum(a, b),
        Op::Max => python_maximum(a, b),
        Op::Div if b != 0.0 => a / b,
        Op::FloorDiv if b != 0.0 => numpy_floor_divide(a, b),
        Op::Mod if b != 0.0 => python_remainder(a, b),
        Op::Pow => a.powf(b),
        _ => return None,
    }))
}

/// Return the sign convention exposed by PyBevy's Python expression APIs.
///
/// Python numeric, tensor, and column-expression libraries define `sign(0)`
/// as zero and propagate NaN. Returning `x` in the unordered/equal branch also
/// preserves signed zero. Do not replace this with [`f64::signum`], which maps
/// signed zero to `-1.0` or `1.0` and would be surprising for a method named
/// `sign` in a Python API.
#[inline]
pub fn python_sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}

#[inline]
fn python_sign_f32(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}

/// Return Python's `%` result for floating-point operands.
///
/// Rust's remainder keeps the dividend's sign, while Python and array
/// expression libraries give a nonzero result the divisor's sign. A zero
/// result also inherits the divisor's sign. NaN inputs and a zero divisor
/// naturally remain NaN, which is appropriate for element-wise evaluation.
#[inline]
pub fn python_remainder(dividend: f64, divisor: f64) -> f64 {
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
pub fn python_remainder_f32(dividend: f32, divisor: f32) -> f32 {
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

macro_rules! numpy_float_floor_divide {
    ($(#[$meta:meta])* $name:ident, $ty:ty) => {
        $(#[$meta])*
        #[inline]
        pub fn $name(dividend: $ty, divisor: $ty) -> $ty {
            if divisor == 0.0 {
                return dividend / divisor;
            }
            let remainder = dividend % divisor;
            let mut quotient = (dividend - remainder) / divisor;
            if remainder != 0.0 && (divisor < 0.0) != (remainder < 0.0) {
                quotient -= 1.0;
            }
            if quotient == 0.0 {
                return (0.0 as $ty).copysign(dividend / divisor);
            }
            let floor = quotient.floor();
            if quotient - floor > 0.5 { floor + 1.0 } else { floor }
        }
    };
}

numpy_float_floor_divide!(
    /// Return NumPy's `floor_divide` result for floating-point operands.
    ///
    /// The quotient is snapped from `(a - a % b) / b` so it agrees with
    /// [`python_remainder`]; a zero divisor yields `a / b` (signed infinity or NaN).
    numpy_floor_divide,
    f64
);
numpy_float_floor_divide!(
    /// `f32` lane of [`numpy_floor_divide`].
    numpy_floor_divide_f32,
    f32
);

/// Round to the nearest integer using Python's ties-to-even convention.
#[inline]
pub fn python_round(x: f64) -> f64 {
    x.round_ties_even()
}

/// Return the element-wise minimum while propagating NaN.
///
/// This matches Python array-expression `minimum`; Rust's [`f64::min`] instead
/// treats a single NaN as missing and returns the other operand.
#[inline]
pub fn python_minimum(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else {
        a.min(b)
    }
}

#[inline]
fn python_minimum_f32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else {
        a.min(b)
    }
}

/// Return the element-wise maximum while propagating NaN.
#[inline]
pub fn python_maximum(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else {
        a.max(b)
    }
}

#[inline]
fn python_maximum_f32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else {
        a.max(b)
    }
}

/// Clip a value using NumPy-compatible element-wise semantics.
///
/// NaN values or bounds propagate. If `min > max`, the upper bound is returned
/// instead of allowing [`f64::clamp`] to panic across a Python call boundary.
#[inline]
pub fn python_clip(value: f64, min: f64, max: f64) -> f64 {
    python_minimum(python_maximum(value, min), max)
}

#[inline]
fn python_clip_f32(value: f32, min: f32, max: f32) -> f32 {
    python_minimum_f32(python_maximum_f32(value, min), max)
}

/// Read a field through the explicit f64 conversion path.
///
/// This helper serves APIs whose result domain is already f64 and the f64
/// tiled executor. The scalar VM uses [`read_field_stack_value`] so integer
/// fields do not pass through this conversion.
///
/// Uses `read_unaligned` for types wider than 4 bytes because ECS column
/// storage may not guarantee 8-byte alignment on 32-bit platforms.
///
/// # Safety
/// The pointer must be valid and not concurrently mutated.
#[inline(always)]
pub unsafe fn read_field_value(ptr: *const u8, field_type: FieldType) -> f64 {
    // SAFETY: the caller guarantees that `ptr` is valid for the primitive layout
    // selected by `field_type`; unaligned reads avoid imposing extra alignment.
    unsafe {
        match field_type {
            FieldType::F32 => (ptr as *const f32).read_unaligned() as f64,
            FieldType::F64 => (ptr as *const f64).read_unaligned(),
            FieldType::I32 => (ptr as *const i32).read_unaligned() as f64,
            FieldType::I64 => (ptr as *const i64).read_unaligned() as f64,
            FieldType::U8 => f64::from(ptr.read_unaligned()),
            FieldType::U32 => (ptr as *const u32).read_unaligned() as f64,
            FieldType::U64 => (ptr as *const u64).read_unaligned() as f64,
            // Read the byte as `u8`, not `bool`: a `bool` whose byte is not 0/1 is an
            // invalid bit pattern (instant UB). `validate_bytecode_field_types` should
            // already reject a `Bool` field aimed at non-bool bytes, but keep this read
            // sound on its own. Any non-zero byte reads as true.
            FieldType::Bool => {
                if ptr.read_unaligned() != 0 {
                    1.0
                } else {
                    0.0
                }
            }
            // Vec2/Vec3/Vec4 are composite signal types - the VM decomposes them to individual F32 sub-fields
            // before execution, so these should never appear in read_field_value
            FieldType::Vec2 | FieldType::Vec3 | FieldType::Vec4 => {
                unreachable!("VM should decompose Vec2/Vec3/Vec4 to F32 sub-fields")
            }
        }
    }
}

/// Write an f64 value to a raw pointer, converting to the target field type.
///
/// Uses `write_unaligned` for types wider than 4 bytes (see [`read_field_value`]).
///
/// # Safety
/// The pointer must be valid and not concurrently read.
#[inline(always)]
pub unsafe fn write_field_value(ptr: *mut u8, value: f64, field_type: FieldType) {
    // SAFETY: the caller guarantees that `ptr` is valid and uniquely writable
    // for the primitive layout selected by `field_type`.
    unsafe {
        match field_type {
            FieldType::F32 => (ptr as *mut f32).write_unaligned(value as f32),
            FieldType::F64 => (ptr as *mut f64).write_unaligned(value),
            FieldType::I32 => (ptr as *mut i32).write_unaligned(value as i32),
            FieldType::I64 => (ptr as *mut i64).write_unaligned(value as i64),
            FieldType::U8 => ptr.write_unaligned(value as u8),
            FieldType::U32 => (ptr as *mut u32).write_unaligned(value as u32),
            FieldType::U64 => (ptr as *mut u64).write_unaligned(value as u64),
            FieldType::Bool => {
                // Write through `u8` (mirrors the `u8` read) so the store never depends
                // on `*mut bool` provenance; a bool is stored as 1/0.
                ptr.write_unaligned(u8::from(value != 0.0));
            }
            FieldType::Vec2 | FieldType::Vec3 | FieldType::Vec4 => {
                unreachable!("VM should decompose Vec2/Vec3/Vec4 to F32 sub-fields")
            }
        }
    }
}

/// Read a primitive field into its exact VM execution domain.
///
/// The scalar VM evaluates floating-point fields in f64; native-f32 tiled
/// programs bypass this helper. Integers remain in their declared width so
/// identity, arithmetic, and comparisons do not silently round through f64.
///
/// # Safety
/// The pointer must be valid for a value of `field_type` and not concurrently mutated.
#[inline(always)]
pub unsafe fn read_field_stack_value(ptr: *const u8, field_type: FieldType) -> StackValue {
    // SAFETY: the caller guarantees that `ptr` points to a live value whose layout
    // matches `field_type`; every wider access is deliberately unaligned.
    unsafe {
        match field_type {
            FieldType::F32 => StackValue::Float(f64::from((ptr as *const f32).read_unaligned())),
            FieldType::F64 => StackValue::Float((ptr as *const f64).read_unaligned()),
            FieldType::I32 => StackValue::I32((ptr as *const i32).read_unaligned()),
            FieldType::I64 => StackValue::I64((ptr as *const i64).read_unaligned()),
            FieldType::U8 => StackValue::U8(ptr.read_unaligned()),
            FieldType::U32 => StackValue::U32((ptr as *const u32).read_unaligned()),
            FieldType::U64 => StackValue::U64((ptr as *const u64).read_unaligned()),
            FieldType::Bool => StackValue::Bool(ptr.read_unaligned() != 0),
            FieldType::Vec2 | FieldType::Vec3 | FieldType::Vec4 => {
                unreachable!("VM should decompose Vec2/Vec3/Vec4 to F32 sub-fields")
            }
        }
    }
}

/// Write a VM value without losing exact integer values.
///
/// Every integer value, including an int literal reaching the field
/// unconverted (a `where` branch), is cast exactly and wraps into an integer
/// field as NumPy casts it. Other cross-domain stores convert through f64.
///
/// # Safety
/// The pointer must be valid for a value of `field_type` and not concurrently read.
#[inline(always)]
pub unsafe fn write_field_stack_value(ptr: *mut u8, value: StackValue, field_type: FieldType) {
    // SAFETY: the caller guarantees exclusive writable access to a live value whose
    // layout matches `field_type`; conversion stores preserve that same contract.
    unsafe {
        if matches!(
            field_type,
            FieldType::I32 | FieldType::I64 | FieldType::U8 | FieldType::U32 | FieldType::U64
        ) && let Some(value) = value.integer_value()
        {
            match field_type {
                FieldType::I32 => (ptr as *mut i32).write_unaligned(value as i32),
                FieldType::I64 => (ptr as *mut i64).write_unaligned(value as i64),
                FieldType::U8 => ptr.write_unaligned(value as u8),
                FieldType::U32 => (ptr as *mut u32).write_unaligned(value as u32),
                _ => (ptr as *mut u64).write_unaligned(value as u64),
            }
            return;
        }
        match (field_type, value) {
            (FieldType::Bool, StackValue::Bool(value)) => {
                ptr.write_unaligned(u8::from(value));
            }
            (FieldType::Bool, value) => {
                ptr.write_unaligned(u8::from(value.as_float() != 0.0));
            }
            (field_type, StackValue::Bool(_)) => {
                panic!("cannot store bool in numeric field {field_type:?}")
            }
            (field_type, value) => write_field_value(ptr, value.as_float(), field_type),
        }
    }
}

/// Stack-based virtual machine for executing bytecode
pub struct VM {
    /// Evaluation stack for typed numeric and boolean values.
    pub(crate) stack: Vec<StackValue>,
    /// Current row's random seed, derived from its entity identity.
    seed: u64,
    /// Execution domain for dense programs whose numeric inputs are all f32.
    native_f32: bool,
    /// Set by an integer power with a negative exponent; read after a pass.
    negative_power: bool,
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

// Thread-local VM pool for reuse across entities
thread_local! {
    static VM_POOL: std::cell::RefCell<Vec<VM>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// RAII guard that returns VM to pool on drop
pub struct PooledVM {
    vm: Option<VM>,
}

impl PooledVM {
    /// Get a reset VM from the thread-local pool, or create a new one
    pub fn acquire() -> Self {
        let mut vm = VM_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default();
        vm.reset();
        Self { vm: Some(vm) }
    }

    /// Get mutable reference to the VM
    #[inline]
    pub fn get_mut(&mut self) -> &mut VM {
        self.vm.as_mut().expect("VM already released")
    }

    /// Check if this guard still holds a VM (for testing)
    #[cfg(test)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn has_vm(&self) -> bool {
        self.vm.is_some()
    }
}

impl Drop for PooledVM {
    fn drop(&mut self) {
        if let Some(vm) = self.vm.take() {
            VM_POOL.with(|pool| {
                let mut pool = pool.borrow_mut();
                // Keep pool size bounded to avoid memory leaks
                if pool.len() < 16 {
                    pool.push(vm);
                }
            });
        }
    }
}

impl VM {
    /// Create a new VM with pre-allocated stack
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(32), // Most expressions need < 32 stack slots
            seed: 0,
            native_f32: false,
            negative_power: false,
        }
    }

    /// Reset VM state for reuse (keeps stack allocation)
    #[inline]
    pub fn reset(&mut self) {
        self.stack.clear();
        self.seed = 0;
        self.native_f32 = false;
        self.negative_power = false;
    }

    /// Report and clear whether any row since the last call raised an
    /// integer to a negative power.
    #[inline]
    pub fn take_negative_power(&mut self) -> Result<(), NegativeIntegerPower> {
        if std::mem::take(&mut self.negative_power) {
            Err(NegativeIntegerPower)
        } else {
            Ok(())
        }
    }

    #[inline]
    pub(crate) fn set_native_f32(&mut self, native_f32: bool) {
        self.native_f32 = native_f32;
    }

    /// Deterministic draw in `[0.0, 1.0)` for the row's seed and `stream`.
    #[inline]
    fn random(&self, stream: u32) -> f64 {
        let mut x = self.seed ^ u64::from(stream).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^= x >> 31;
        // The top 53 bits fill an f64 mantissa exactly, so the draw stays below 1.
        (x >> 11) as f64 / 9_007_199_254_740_992.0
    }

    /// Dispatch a single stack-only operation (everything except external loads/stores).
    ///
    /// Returns `false` for component fields, dense inputs, and stores, whose
    /// storage is supplied by the caller. Returns `true` for all other ops.
    #[inline(always)]
    pub(crate) fn dispatch_stack_op(&mut self, op: &Op, bytecode: &CompiledBytecode) -> bool {
        match op {
            Op::PushField(_) | Op::PushInput(_) | Op::StoreField(_) => return false,

            Op::PushConst(const_idx) => {
                let value = bytecode.constants[*const_idx as usize];
                self.stack.push(if self.native_f32 {
                    StackValue::F32(value as f32)
                } else {
                    StackValue::Constant(value)
                });
            }
            Op::PushFloatConst(const_idx) => {
                let value = bytecode.constants[*const_idx as usize];
                self.stack.push(if self.native_f32 {
                    StackValue::F32(value as f32)
                } else {
                    StackValue::FloatConstant(value)
                });
            }
            Op::PushInt(index) => {
                let value = bytecode.integers[usize::from(*index)];
                self.stack.push(if self.native_f32 {
                    StackValue::F32(value as f32)
                } else {
                    StackValue::int_literal(value)
                });
            }

            Op::Add => {
                let b = self.stack.pop().expect("Stack underflow on Add");
                let a = self.stack.pop().expect("Stack underflow on Add");
                self.stack
                    .push(integer_binary!(a, b, wrapping_add, |a, b| a + b));
            }
            Op::Sub => {
                let b = self.stack.pop().expect("Stack underflow on Sub");
                let a = self.stack.pop().expect("Stack underflow on Sub");
                self.stack
                    .push(integer_binary!(a, b, wrapping_sub, |a, b| a - b));
            }
            Op::Mul => {
                let b = self.stack.pop().expect("Stack underflow on Mul");
                let a = self.stack.pop().expect("Stack underflow on Mul");
                self.stack
                    .push(integer_binary!(a, b, wrapping_mul, |a, b| a * b));
            }
            Op::Div => {
                let b = self.stack.pop().expect("Stack underflow on Div");
                let a = self.stack.pop().expect("Stack underflow on Div");
                self.stack
                    .push(float_binary!(a, b, |a, b| a / b, |a, b| a / b));
            }
            Op::FloorDiv => {
                let b = self.stack.pop().expect("Stack underflow on FloorDiv");
                let a = self.stack.pop().expect("Stack underflow on FloorDiv");
                self.stack.push(floor_divide(a, b));
            }
            Op::Pow => {
                let b = self.stack.pop().expect("Stack underflow on Pow");
                let a = self.stack.pop().expect("Stack underflow on Pow");
                let (value, negative) = power(a, b);
                self.negative_power |= negative;
                self.stack.push(value);
            }
            Op::Mod => {
                let b = self.stack.pop().expect("Stack underflow on Mod");
                let a = self.stack.pop().expect("Stack underflow on Mod");
                self.stack.push(remainder(a, b));
            }
            Op::Neg => {
                let a = self.stack.pop().expect("Stack underflow on Neg");
                self.stack.push(match a {
                    StackValue::F32(value) => StackValue::F32(-value),
                    StackValue::I32(v) => StackValue::I32(v.wrapping_neg()),
                    StackValue::I64(v) => StackValue::I64(v.wrapping_neg()),
                    StackValue::U8(v) => StackValue::U8(v.wrapping_neg()),
                    StackValue::U32(v) => StackValue::U32(v.wrapping_neg()),
                    StackValue::U64(v) => StackValue::U64(v.wrapping_neg()),
                    StackValue::ExactInt(v) => StackValue::int_literal(-i128::from(v)),
                    StackValue::ExactUInt(v) => StackValue::int_literal(-i128::from(v)),
                    value => numeric_result(Origin::of(value), -value.as_float()),
                });
            }

            // Trigonometric
            Op::Sin => {
                let x = self.stack.pop().expect("Stack underflow on Sin");
                self.stack
                    .push(float_unary!(x, |x: f32| x.sin(), |x: f64| x.sin()));
            }
            Op::Cos => {
                let x = self.stack.pop().expect("Stack underflow on Cos");
                self.stack
                    .push(float_unary!(x, |x: f32| x.cos(), |x: f64| x.cos()));
            }
            Op::Tan => {
                let x = self.stack.pop().expect("Stack underflow on Tan");
                self.stack
                    .push(float_unary!(x, |x: f32| x.tan(), |x: f64| x.tan()));
            }
            Op::Asin => {
                let x = self.stack.pop().expect("Stack underflow on Asin");
                self.stack
                    .push(float_unary!(x, |x: f32| x.asin(), |x: f64| x.asin()));
            }
            Op::Acos => {
                let x = self.stack.pop().expect("Stack underflow on Acos");
                self.stack
                    .push(float_unary!(x, |x: f32| x.acos(), |x: f64| x.acos()));
            }
            Op::Atan => {
                let x = self.stack.pop().expect("Stack underflow on Atan");
                self.stack
                    .push(float_unary!(x, |x: f32| x.atan(), |x: f64| x.atan()));
            }

            // Math
            Op::Sqrt => {
                let x = self.stack.pop().expect("Stack underflow on Sqrt");
                self.stack
                    .push(float_unary!(x, |x: f32| x.sqrt(), |x: f64| x.sqrt()));
            }
            Op::Abs => {
                let x = self.stack.pop().expect("Stack underflow on Abs");
                self.stack.push(match x {
                    StackValue::F32(value) => StackValue::F32(value.abs()),
                    StackValue::I32(v) => StackValue::I32(v.wrapping_abs()),
                    StackValue::I64(v) => StackValue::I64(v.wrapping_abs()),
                    StackValue::U8(v) => StackValue::U8(v),
                    StackValue::U32(v) => StackValue::U32(v),
                    StackValue::U64(v) => StackValue::U64(v),
                    value => numeric_result(Origin::of(value), value.as_float().abs()),
                });
            }
            Op::Floor => {
                let x = self.stack.pop().expect("Stack underflow on Floor");
                self.stack.push(match x {
                    StackValue::I32(_)
                    | StackValue::I64(_)
                    | StackValue::U8(_)
                    | StackValue::U32(_)
                    | StackValue::U64(_) => x,
                    StackValue::F32(value) => StackValue::F32(value.floor()),
                    value => numeric_result(Origin::of(value), value.as_float().floor()),
                });
            }
            Op::Ceil => {
                let x = self.stack.pop().expect("Stack underflow on Ceil");
                self.stack.push(match x {
                    StackValue::I32(_)
                    | StackValue::I64(_)
                    | StackValue::U8(_)
                    | StackValue::U32(_)
                    | StackValue::U64(_) => x,
                    StackValue::F32(value) => StackValue::F32(value.ceil()),
                    value => numeric_result(Origin::of(value), value.as_float().ceil()),
                });
            }
            Op::Round => {
                let x = self.stack.pop().expect("Stack underflow on Round");
                self.stack.push(match x {
                    StackValue::I32(_)
                    | StackValue::I64(_)
                    | StackValue::U8(_)
                    | StackValue::U32(_)
                    | StackValue::U64(_) => x,
                    StackValue::F32(value) => StackValue::F32(value.round_ties_even()),
                    value => numeric_result(Origin::of(value), python_round(value.as_float())),
                });
            }
            Op::Exp => {
                let x = self.stack.pop().expect("Stack underflow on Exp");
                self.stack
                    .push(float_unary!(x, |x: f32| x.exp(), |x: f64| x.exp()));
            }
            Op::Ln => {
                let x = self.stack.pop().expect("Stack underflow on Ln");
                self.stack
                    .push(float_unary!(x, |x: f32| x.ln(), |x: f64| x.ln()));
            }
            Op::Log10 => {
                let x = self.stack.pop().expect("Stack underflow on Log10");
                self.stack
                    .push(float_unary!(x, |x: f32| x.log10(), |x: f64| x.log10()));
            }
            Op::Log2 => {
                let x = self.stack.pop().expect("Stack underflow on Log2");
                self.stack
                    .push(float_unary!(x, |x: f32| x.log2(), |x: f64| x.log2()));
            }
            Op::Sign => {
                let x = self.stack.pop().expect("Stack underflow on Sign");
                self.stack.push(match x {
                    StackValue::I32(v) => StackValue::I32(v.signum()),
                    StackValue::I64(v) => StackValue::I64(v.signum()),
                    StackValue::U8(v) => StackValue::U8(u8::from(v != 0)),
                    StackValue::U32(v) => StackValue::U32(u32::from(v != 0)),
                    StackValue::U64(v) => StackValue::U64(u64::from(v != 0)),
                    StackValue::F32(value) => StackValue::F32(python_sign_f32(value)),
                    value => numeric_result(Origin::of(value), python_sign(value.as_float())),
                });
            }
            Op::Fract => {
                let x = self.stack.pop().expect("Stack underflow on Fract");
                self.stack
                    .push(float_unary!(x, |x: f32| x.fract(), |x: f64| x.fract()));
            }
            Op::Lerp => {
                let t = self.stack.pop().expect("Stack underflow on Lerp");
                let b = self.stack.pop().expect("Stack underflow on Lerp");
                let a = self.stack.pop().expect("Stack underflow on Lerp");
                let (a, b) = specialize_constants(a, b);
                let (a, t) = specialize_constants(a, t);
                let (b, t) = specialize_constants(b, t);
                self.stack.push(match (a, b, t) {
                    (StackValue::F32(a), StackValue::F32(b), StackValue::F32(t)) => {
                        StackValue::F32(a + t * (b - a))
                    }
                    (a, b, t) => {
                        let origin = Origin::of_all(&[a, b, t]).floating();
                        let (a, b, t) = (a.as_float(), b.as_float(), t.as_float());
                        numeric_result(origin, a + t * (b - a))
                    }
                });
            }

            // Min/Max/Clamp
            Op::Min => {
                let b = self.stack.pop().expect("Stack underflow on Min");
                let a = self.stack.pop().expect("Stack underflow on Min");
                let (a, b) = specialize_constants(a, b);
                self.stack.push(match (a, b) {
                    (StackValue::I32(a), StackValue::I32(b)) => StackValue::I32(a.min(b)),
                    (StackValue::I64(a), StackValue::I64(b)) => StackValue::I64(a.min(b)),
                    (StackValue::U8(a), StackValue::U8(b)) => StackValue::U8(a.min(b)),
                    (StackValue::U32(a), StackValue::U32(b)) => StackValue::U32(a.min(b)),
                    (StackValue::U64(a), StackValue::U64(b)) => StackValue::U64(a.min(b)),
                    (StackValue::F32(a), StackValue::F32(b)) => {
                        StackValue::F32(python_minimum_f32(a, b))
                    }
                    (a, b) => numeric_result(
                        Origin::of_all(&[a, b]),
                        python_minimum(a.as_float(), b.as_float()),
                    ),
                });
            }
            Op::Max => {
                let b = self.stack.pop().expect("Stack underflow on Max");
                let a = self.stack.pop().expect("Stack underflow on Max");
                let (a, b) = specialize_constants(a, b);
                self.stack.push(match (a, b) {
                    (StackValue::I32(a), StackValue::I32(b)) => StackValue::I32(a.max(b)),
                    (StackValue::I64(a), StackValue::I64(b)) => StackValue::I64(a.max(b)),
                    (StackValue::U8(a), StackValue::U8(b)) => StackValue::U8(a.max(b)),
                    (StackValue::U32(a), StackValue::U32(b)) => StackValue::U32(a.max(b)),
                    (StackValue::U64(a), StackValue::U64(b)) => StackValue::U64(a.max(b)),
                    (StackValue::F32(a), StackValue::F32(b)) => {
                        StackValue::F32(python_maximum_f32(a, b))
                    }
                    (a, b) => numeric_result(
                        Origin::of_all(&[a, b]),
                        python_maximum(a.as_float(), b.as_float()),
                    ),
                });
            }
            Op::Clamp => {
                let max = self.stack.pop().expect("Stack underflow on Clamp");
                let min = self.stack.pop().expect("Stack underflow on Clamp");
                let value = self.stack.pop().expect("Stack underflow on Clamp");
                let (value, min) = specialize_constants(value, min);
                let (value, max) = specialize_constants(value, max);
                let (min, max) = specialize_constants(min, max);
                self.stack.push(match (value, min, max) {
                    (StackValue::I32(v), StackValue::I32(min), StackValue::I32(max)) => {
                        StackValue::I32(v.max(min).min(max))
                    }
                    (StackValue::I64(v), StackValue::I64(min), StackValue::I64(max)) => {
                        StackValue::I64(v.max(min).min(max))
                    }
                    (StackValue::U8(v), StackValue::U8(min), StackValue::U8(max)) => {
                        StackValue::U8(v.max(min).min(max))
                    }
                    (StackValue::U32(v), StackValue::U32(min), StackValue::U32(max)) => {
                        StackValue::U32(v.max(min).min(max))
                    }
                    (StackValue::U64(v), StackValue::U64(min), StackValue::U64(max)) => {
                        StackValue::U64(v.max(min).min(max))
                    }
                    (StackValue::F32(value), StackValue::F32(min), StackValue::F32(max)) => {
                        StackValue::F32(python_clip_f32(value, min, max))
                    }
                    (value, min, max) => numeric_result(
                        Origin::of_all(&[value, min, max]),
                        python_clip(value.as_float(), min.as_float(), max.as_float()),
                    ),
                });
            }

            // Comparison (exact equality - consistent across all execution modes)
            Op::Eq => {
                let b = self.stack.pop().expect("Stack underflow on Eq");
                let a = self.stack.pop().expect("Stack underflow on Eq");
                self.stack
                    .push(StackValue::Bool(numeric_cmp(a, b) == Some(Ordering::Equal)));
            }
            Op::Ne => {
                let b = self.stack.pop().expect("Stack underflow on Ne");
                let a = self.stack.pop().expect("Stack underflow on Ne");
                self.stack
                    .push(StackValue::Bool(numeric_cmp(a, b) != Some(Ordering::Equal)));
            }
            Op::Lt => {
                let b = self.stack.pop().expect("Stack underflow on Lt");
                let a = self.stack.pop().expect("Stack underflow on Lt");
                self.stack
                    .push(StackValue::Bool(numeric_cmp(a, b) == Some(Ordering::Less)));
            }
            Op::Le => {
                let b = self.stack.pop().expect("Stack underflow on Le");
                let a = self.stack.pop().expect("Stack underflow on Le");
                self.stack.push(StackValue::Bool(matches!(
                    numeric_cmp(a, b),
                    Some(Ordering::Less | Ordering::Equal)
                )));
            }
            Op::Gt => {
                let b = self.stack.pop().expect("Stack underflow on Gt");
                let a = self.stack.pop().expect("Stack underflow on Gt");
                self.stack.push(StackValue::Bool(
                    numeric_cmp(a, b) == Some(Ordering::Greater),
                ));
            }
            Op::Ge => {
                let b = self.stack.pop().expect("Stack underflow on Ge");
                let a = self.stack.pop().expect("Stack underflow on Ge");
                self.stack.push(StackValue::Bool(matches!(
                    numeric_cmp(a, b),
                    Some(Ordering::Greater | Ordering::Equal)
                )));
            }

            // Logical
            Op::And => {
                let b = self.stack.pop().expect("Stack underflow on And").as_bool();
                let a = self.stack.pop().expect("Stack underflow on And").as_bool();
                self.stack.push(StackValue::Bool(a && b));
            }
            Op::Or => {
                let b = self.stack.pop().expect("Stack underflow on Or").as_bool();
                let a = self.stack.pop().expect("Stack underflow on Or").as_bool();
                self.stack.push(StackValue::Bool(a || b));
            }
            Op::Not => {
                let a = self.stack.pop().expect("Stack underflow on Not").as_bool();
                self.stack.push(StackValue::Bool(!a));
            }

            // Conditional
            Op::Where => {
                let false_val = self.stack.pop().expect("Stack underflow on Where");
                let true_val = self.stack.pop().expect("Stack underflow on Where");
                let condition = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Where")
                    .as_bool();
                self.stack
                    .push(if condition { true_val } else { false_val });
            }

            // Random
            Op::Random(stream) => {
                let random = self.random(*stream);
                self.stack.push(if self.native_f32 {
                    StackValue::F32(random as f32)
                } else {
                    StackValue::Float(random)
                });
            }
            Op::RandomRange(stream) => {
                let rand = self.random(*stream);
                let max = self.stack.pop().expect("Stack underflow on RandomRange");
                let min = self.stack.pop().expect("Stack underflow on RandomRange");
                self.stack.push(if self.native_f32 {
                    let (min, max, rand) =
                        (min.as_float() as f32, max.as_float() as f32, rand as f32);
                    StackValue::F32(min + rand * (max - min))
                } else {
                    let (min, max) = (min.as_float(), max.as_float());
                    StackValue::Float(min + rand * (max - min))
                });
            }
        }
        true
    }

    /// Execute bytecode on a single entity
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `field_ptrs` holds one pointer per `bytecode.field_map` entry, valid for
    ///   reads of its `FieldType` and for writes where the program stores to it;
    ///   no alignment is required
    /// - Field pointers remain valid for the duration of this call
    /// - No other code mutates the same memory during execution
    pub unsafe fn execute(
        &mut self,
        bytecode: &CompiledBytecode,
        field_ptrs: &[*mut u8],
        seed: u64,
    ) {
        self.stack.clear();
        self.seed = seed;

        for op in &bytecode.bytecode {
            if self.dispatch_stack_op(op, bytecode) {
                continue;
            }
            match op {
                Op::PushField(field_idx) => {
                    let field_id = &bytecode.field_map[*field_idx as usize];
                    let ptr = field_ptrs[*field_idx as usize];
                    // SAFETY: forwarded from `execute`'s pointer/layout contract.
                    let value = unsafe { read_field_stack_value(ptr, field_id.field_type) };
                    self.stack.push(value);
                }
                Op::StoreField(field_idx) => {
                    let field_id = &bytecode.field_map[*field_idx as usize];
                    let value = self.stack.pop().expect("Stack underflow on StoreField");
                    let ptr = field_ptrs[*field_idx as usize];
                    // SAFETY: forwarded from `execute`'s exclusive pointer contract.
                    unsafe { write_field_stack_value(ptr, value, field_id.field_type) };
                }
                _ => unreachable!(),
            }
        }
    }

    /// Execute bytecode in reduction mode and return the final value
    ///
    /// Evaluates the expression but does not store results - instead
    /// returns the top of the stack after execution.
    ///
    /// # Safety
    ///
    /// Same safety requirements as `execute()`:
    /// - `field_ptrs` holds one pointer per `bytecode.field_map` entry, valid for
    ///   reads of its `FieldType`; no alignment is required
    /// - Field pointers must remain valid for the duration of this call
    pub unsafe fn execute_and_reduce(
        &mut self,
        bytecode: &CompiledBytecode,
        field_ptrs: &[*mut u8],
        seed: u64,
    ) -> f64 {
        self.stack.clear();
        self.seed = seed;

        for op in &bytecode.bytecode {
            if self.dispatch_stack_op(op, bytecode) {
                continue;
            }
            match op {
                Op::PushField(field_idx) => {
                    let field_id = &bytecode.field_map[*field_idx as usize];
                    let ptr = field_ptrs[*field_idx as usize];
                    // SAFETY: forwarded from `execute_and_reduce`'s pointer contract.
                    let value = unsafe { read_field_stack_value(ptr, field_id.field_type) };
                    self.stack.push(value);
                }
                // In reduction mode, we don't store - just pop the value
                Op::StoreField(_) => {
                    self.stack
                        .pop()
                        .expect("Stack underflow on StoreField in reduce mode");
                }
                _ => unreachable!(),
            }
        }

        // Return the top of stack as the final result
        // Convert bool to float (true=1.0, false=0.0) for reduction operations
        let result = self.stack.pop().expect("Stack empty after reduction");
        match result {
            StackValue::F32(f) => f64::from(f),
            StackValue::Float(f) | StackValue::Constant(f) | StackValue::FloatConstant(f) => f,
            StackValue::ExactInt(v) => v as f64,
            StackValue::ExactUInt(v) => v as f64,
            StackValue::I32(v) => f64::from(v),
            StackValue::I64(v) => v as f64,
            StackValue::U8(v) => f64::from(v),
            StackValue::U32(v) => f64::from(v),
            StackValue::U64(v) => v as f64,
            StackValue::Bool(b) => {
                if b {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    /// Execute bytecode in batch mode with multiple components.
    ///
    /// Each field can have a different base pointer and stride, allowing
    /// cross-component expressions like `pos.x = pos.x + vel.x`.
    ///
    /// `row_seeds` gives row `i` its `Op::Random` seed, which must come from
    /// the row's stable identity. `None` falls back to the row index.
    ///
    /// # Safety
    /// - All pointers in field_bases must be valid for count * stride bytes
    /// - All field types must match the actual memory layout
    #[inline]
    pub unsafe fn execute_batch_multi(
        &mut self,
        bytecode: &CompiledBytecode,
        field_bases: &[*mut u8],
        field_strides: &[usize],
        count: usize,
        row_seeds: Option<&[u64]>,
    ) {
        // SAFETY: the caller provides one correctly typed base and stride for
        // every bytecode field, each spanning all `count` entities.
        unsafe {
            debug_assert_eq!(field_bases.len(), bytecode.field_map.len());
            debug_assert_eq!(field_strides.len(), bytecode.field_map.len());

            for entity_idx in 0..count {
                self.stack.clear();
                self.seed = match row_seeds {
                    Some(seeds) => seeds[entity_idx],
                    None => entity_idx as u64,
                };

                for op in &bytecode.bytecode {
                    if self.dispatch_stack_op(op, bytecode) {
                        continue;
                    }
                    match op {
                        Op::PushField(field_idx) => {
                            let idx = *field_idx as usize;
                            let field_id = &bytecode.field_map[idx];
                            let ptr = field_bases[idx].add(entity_idx * field_strides[idx]);
                            let value = read_field_stack_value(ptr, field_id.field_type);
                            self.stack.push(value);
                        }
                        Op::StoreField(field_idx) => {
                            let idx = *field_idx as usize;
                            let field_id = &bytecode.field_map[idx];
                            let value = self.stack.pop().expect("Stack underflow on StoreField");
                            let ptr = field_bases[idx].add(entity_idx * field_strides[idx]);
                            write_field_stack_value(ptr, value, field_id.field_type);
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use pybevy_storage::storage_error::EXPRESSION_MAX_POOL_ENTRIES;

    use super::*;

    #[test]
    fn test_simple_arithmetic() {
        let mut compiler = Compiler::new();

        // Expression: 5.0 + 3.0 * 2.0 = 11.0
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);
        let const_2 = compiler.add_constant(2.0);

        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_2));
        compiler.emit(Op::Mul); // 3.0 * 2.0 = 6.0
        compiler.emit(Op::Add); // 5.0 + 6.0 = 11.0

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // No field access, just check the stack
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 11.0);
    }

    #[test]
    fn test_constant_folding() {
        let mut compiler = Compiler::new();

        // Expression: 5.0 + 3.0 (should fold to 8.0)
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);

        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::Add);

        compiler.optimize();

        let bytecode = compiler.finalize();

        // After optimization, should be just PushConst(8.0)
        assert_eq!(bytecode.bytecode.len(), 1);
        assert!(matches!(bytecode.bytecode[0], Op::PushConst(_)));
    }

    #[test]
    fn test_field_store() {
        let mut compiler = Compiler::new();

        // Expression: field[0] = field[0] + 10.0
        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        };
        let field_idx = compiler.add_field(field_id);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // Create a test field
        let mut field_value = 5.0_f32;
        let field_ptr = &mut field_value as *mut f32 as *mut u8;

        // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
        unsafe {
            vm.execute(&bytecode, &[field_ptr], 0);
        }

        assert_eq!(field_value, 15.0);
    }

    #[test]
    fn test_trig_functions() {
        use std::f64::consts::PI;
        let mut compiler = Compiler::new();

        // Test sin(PI/2) = 1.0
        let const_pi_2 = compiler.add_constant(PI / 2.0);
        compiler.emit(Op::PushConst(const_pi_2));
        compiler.emit(Op::Sin);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert!((vm.stack[0].as_float() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_numeric_functions() {
        let mut compiler = Compiler::new();

        // Test sqrt(16.0) = 4.0
        let const_16 = compiler.add_constant(16.0);
        compiler.emit(Op::PushConst(const_16));
        compiler.emit(Op::Sqrt);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 4.0);
    }

    #[test]
    fn test_abs_floor_ceil() {
        let mut compiler = Compiler::new();

        // Test abs(-5.3)
        let const_neg = compiler.add_constant(-5.3);
        compiler.emit(Op::PushConst(const_neg));
        compiler.emit(Op::Abs);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 5.3);
    }

    #[test]
    fn test_min_max() {
        let mut compiler = Compiler::new();

        // Test min(10.0, 5.0) = 5.0
        let const_10 = compiler.add_constant(10.0);
        let const_5 = compiler.add_constant(5.0);
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::Min);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 5.0);
    }

    #[test]
    fn test_clamp() {
        let mut compiler = Compiler::new();

        // Test clamp(15.0, 0.0, 10.0) = 10.0
        let const_15 = compiler.add_constant(15.0);
        let const_0 = compiler.add_constant(0.0);
        let const_10 = compiler.add_constant(10.0);
        compiler.emit(Op::PushConst(const_15));
        compiler.emit(Op::PushConst(const_0));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Clamp);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 10.0);
    }

    #[test]
    fn test_complex_expression() {
        let mut compiler = Compiler::new();

        // Test: sqrt(abs(-16.0)) + 2.0 = 6.0
        let const_neg16 = compiler.add_constant(-16.0);
        let const_2 = compiler.add_constant(2.0);

        compiler.emit(Op::PushConst(const_neg16));
        compiler.emit(Op::Abs); // 16.0
        compiler.emit(Op::Sqrt); // 4.0
        compiler.emit(Op::PushConst(const_2));
        compiler.emit(Op::Add); // 6.0

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 6.0);
    }

    #[test]
    fn test_comparison_operators() {
        let mut compiler = Compiler::new();

        // Test: 5.0 < 3.0 => false
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);

        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::Lt);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert!(!vm.stack[0].as_bool());

        // Test: 3.0 <= 5.0 => true
        let mut compiler = Compiler::new();
        let const_3 = compiler.add_constant(3.0);
        let const_5 = compiler.add_constant(5.0);

        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::Le);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert!(vm.stack[0].as_bool());

        // Test: 5.0 == 5.0 => true
        let mut compiler = Compiler::new();
        let const_5a = compiler.add_constant(5.0);
        let const_5b = compiler.add_constant(5.0);

        compiler.emit(Op::PushConst(const_5a));
        compiler.emit(Op::PushConst(const_5b));
        compiler.emit(Op::Eq);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert!(vm.stack[0].as_bool());
    }

    #[test]
    fn test_where_conditional() {
        let mut compiler = Compiler::new();

        // Test: where(5.0 > 3.0, 100.0, 200.0) => 100.0 (true branch)
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);
        let const_100 = compiler.add_constant(100.0);
        let const_200 = compiler.add_constant(200.0);

        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::Gt); // 5.0 > 3.0 => true
        compiler.emit(Op::PushConst(const_100)); // true value
        compiler.emit(Op::PushConst(const_200)); // false value
        compiler.emit(Op::Where);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 100.0);

        // Test: where(2.0 > 5.0, 100.0, 200.0) => 200.0 (false branch)
        let mut compiler = Compiler::new();
        let const_2 = compiler.add_constant(2.0);
        let const_5 = compiler.add_constant(5.0);
        let const_100 = compiler.add_constant(100.0);
        let const_200 = compiler.add_constant(200.0);

        compiler.emit(Op::PushConst(const_2));
        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::Gt); // 2.0 > 5.0 => false
        compiler.emit(Op::PushConst(const_100)); // true value
        compiler.emit(Op::PushConst(const_200)); // false value
        compiler.emit(Op::Where);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 200.0);
    }

    #[test]
    fn test_logical_operators() {
        // Test: true && false => false
        let mut compiler = Compiler::new();
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::Gt); // 5.0 > 3.0 => true

        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Gt); // 3.0 > 10.0 => false

        compiler.emit(Op::And); // true && false => false

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert!(!vm.stack[0].as_bool());

        // Test: true || false => true
        let mut compiler = Compiler::new();
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::Gt); // 5.0 > 3.0 => true

        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Gt); // 3.0 > 10.0 => false

        compiler.emit(Op::Or); // true || false => true

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert!(vm.stack[0].as_bool());

        // Test: !false => true
        let mut compiler = Compiler::new();
        let const_3 = compiler.add_constant(3.0);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Gt); // 3.0 > 10.0 => false
        compiler.emit(Op::Not); // !false => true

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert!(vm.stack[0].as_bool());

        // Test complex: (true && false) || !false => true
        let mut compiler = Compiler::new();
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);
        let const_10 = compiler.add_constant(10.0);

        // Left side: true && false
        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::Gt); // 5.0 > 3.0 => true

        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Gt); // 3.0 > 10.0 => false

        compiler.emit(Op::And); // true && false => false

        // Right side: !false
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Gt); // 3.0 > 10.0 => false
        compiler.emit(Op::Not); // !false => true

        compiler.emit(Op::Or); // false || true => true

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert!(vm.stack[0].as_bool());
    }

    #[test]
    fn random_stays_below_one_at_the_largest_draw() {
        fn unshift(value: u64, shift: u32) -> u64 {
            let mut result = value;
            for _ in 0..64 / shift {
                result = value ^ (result >> shift);
            }
            result
        }
        fn inverse(odd: u64) -> u64 {
            let mut inverse = odd;
            for _ in 0..6 {
                inverse = inverse.wrapping_mul(2_u64.wrapping_sub(odd.wrapping_mul(inverse)));
            }
            inverse
        }
        let mut seed = unshift(u64::MAX, 31);
        seed = unshift(seed.wrapping_mul(inverse(0x94D0_49BB_1331_11EB)), 27);
        seed = unshift(seed.wrapping_mul(inverse(0xBF58_476D_1CE4_E5B9)), 30);

        let mut compiler = Compiler::new();
        compiler.emit(Op::Random(0));
        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        // SAFETY: this stack-only program reads no component fields.
        unsafe { vm.execute(&bytecode, &[], seed) };
        assert_eq!(vm.stack[0].as_float(), 1.0 - f64::EPSILON / 2.0);
    }

    #[test]
    fn random_streams_select_distinct_deterministic_draws() {
        for seed in [0, 1, 37, u64::MAX] {
            let mut values = Vec::new();
            for stream in [0, 1, 2] {
                let mut compiler = Compiler::new();
                compiler.emit(Op::Random(stream));
                let bytecode = compiler.finalize();
                let mut vm = VM::new();
                // SAFETY: this stack-only program reads no component fields.
                unsafe { vm.execute(&bytecode, &[], seed) };
                values.push(vm.stack[0].as_float());
            }
            assert_eq!(values.len(), 3);
            assert_ne!(values[0], values[1]);
            assert_ne!(values[0], values[2]);
            assert_ne!(values[1], values[2]);
        }
    }

    #[test]
    fn test_pooled_vm_acquire_and_release() {
        // Acquire a VM from pool
        let mut pooled = PooledVM::acquire();
        let vm = pooled.get_mut();

        // VM should be valid with pre-allocated stack
        assert_eq!(vm.stack.capacity(), 32);

        // Drop should return to pool (no panic)
        drop(pooled);

        // Acquire again - should get a recycled VM
        let pooled2 = PooledVM::acquire();
        drop(pooled2);
    }

    #[test]
    fn pooled_vm_acquire_clears_stale_state() {
        {
            let mut pooled = PooledVM::acquire();
            let vm = pooled.get_mut();
            vm.stack.push(StackValue::Float(42.0));
            vm.seed = 7;
            vm.set_native_f32(true);
        }

        // The dirty VM went back to the thread-local pool; the next acquire on
        // this thread must not observe its state.
        let mut pooled = PooledVM::acquire();
        let vm = pooled.get_mut();
        assert!(vm.stack.is_empty());
        assert_eq!(vm.seed, 0);
        assert!(
            !vm.native_f32,
            "stale native-f32 mode would change rounding"
        );
    }

    #[test]
    fn test_pooled_vm_reset() {
        let mut pooled = PooledVM::acquire();
        let vm = pooled.get_mut();

        // Add some state
        vm.stack.push(StackValue::Float(42.0));
        vm.seed = 999;

        // Reset should clear state
        vm.reset();

        assert!(vm.stack.is_empty());
        assert_eq!(vm.seed, 0);

        // But stack capacity should be preserved
        assert_eq!(vm.stack.capacity(), 32);
    }

    #[test]
    fn test_pooled_vm_executes_correctly() {
        // Create simple bytecode: value = value + 10
        let mut compiler = Compiler::new();
        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        };
        let field_idx = compiler.add_field(field_id);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();

        // Test with pooled VM
        let mut value = 5.0_f32;
        let ptr = &mut value as *mut f32 as *mut u8;

        {
            let mut pooled = PooledVM::acquire();
            let vm = pooled.get_mut();
            vm.reset();
            // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
            unsafe {
                vm.execute(&bytecode, &[ptr], 0);
            }
        }

        assert_eq!(value, 15.0);

        // Execute again with same pooled VM pattern
        {
            let mut pooled = PooledVM::acquire();
            let vm = pooled.get_mut();
            vm.reset();
            // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
            unsafe {
                vm.execute(&bytecode, &[ptr], 0);
            }
        }

        assert_eq!(value, 25.0);
    }

    #[test]
    fn test_pooled_vm_multiple_entities() {
        // Simulate processing multiple entities with pooled VM
        let mut compiler = Compiler::new();
        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        };
        let field_idx = compiler.add_field(field_id);
        let const_1 = compiler.add_constant(1.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_1));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();

        // Process 100 entities
        let mut values: Vec<f32> = (0..100).map(|i| i as f32).collect();

        for (i, value) in values.iter_mut().enumerate() {
            let mut pooled = PooledVM::acquire();
            let vm = pooled.get_mut();
            vm.reset();
            let ptr = value as *mut f32 as *mut u8;
            // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
            unsafe {
                vm.execute(&bytecode, &[ptr], i as u64);
            }
        }

        // Each value should be incremented by 1
        for (i, value) in values.iter().enumerate() {
            assert_eq!(*value, (i + 1) as f32);
        }
    }

    #[test]
    fn test_pool_bounded_size() {
        // Acquire many VMs to test pool size bounding
        let mut vms: Vec<PooledVM> = Vec::new();

        // Acquire 20 VMs (more than pool limit of 16)
        for _ in 0..20 {
            vms.push(PooledVM::acquire());
        }

        // Drop all - only 16 should go back to pool
        drop(vms);

        // Verify pool works after this
        let pooled = PooledVM::acquire();
        assert!(pooled.has_vm());
    }

    #[test]
    fn test_f64_field_store() {
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let mut field_value = 5.0_f64;
        let field_ptr = &mut field_value as *mut f64 as *mut u8;

        // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
        unsafe {
            vm.execute(&bytecode, &[field_ptr], 0);
        }

        assert_eq!(field_value, 15.0);
    }

    #[test]
    fn test_f64_precision_preserved() {
        // 0.1 + 0.2 in f64 has a specific representation that differs from f32
        // This value would lose precision if truncated to f32 and back
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);

        // Store a value that requires f64 precision: 1_000_000.123_456_789
        let precise_value: f64 = 1_000_000.123_456_789;
        let const_val = compiler.add_constant(precise_value);

        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let mut field_value = 0.0_f64;
        let field_ptr = &mut field_value as *mut f64 as *mut u8;

        // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
        unsafe {
            vm.execute(&bytecode, &[field_ptr], 0);
        }

        // f64 preserves this exactly; f32 would give 1000000.125
        assert_eq!(field_value, precise_value);
    }

    #[test]
    fn test_f32_roundtrip_lossless() {
        // f32 → f64 (load) → f64 arithmetic → f32 (store) should be lossless
        // for values that are exactly representable in f32
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(0.0);

        // field = field + 0.0 (identity operation to force roundtrip)
        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // Test with various f32 values
        let test_values: Vec<f32> = vec![
            0.0,
            1.0,
            -1.0,
            0.5,
            f32::MAX,
            f32::MIN,
            f32::MIN_POSITIVE,
            std::f32::consts::PI,
            123.456,
        ];

        for &original in &test_values {
            let mut field_value = original;
            let field_ptr = &mut field_value as *mut f32 as *mut u8;

            // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
            unsafe {
                vm.reset();
                vm.execute(&bytecode, &[field_ptr], 0);
            }

            assert_eq!(
                field_value, original,
                "f32 roundtrip failed for {}",
                original
            );
        }
    }

    #[test]
    fn test_i64_field_load_store() {
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::I64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let mut field_value = 42_i64;
        let field_ptr = &mut field_value as *mut i64 as *mut u8;

        // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
        unsafe {
            vm.execute(&bytecode, &[field_ptr], 0);
        }

        assert_eq!(field_value, 52);
    }

    #[test]
    fn test_bool_field_load_store() {
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::Bool,
        };
        let field_idx = compiler.add_field(field_id);

        // Load bool into its native boolean stack domain.
        compiler.emit(Op::PushField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let mut field_value = true;
        let field_ptr = &mut field_value as *mut bool as *mut u8;

        // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
        unsafe {
            vm.execute(&bytecode, &[field_ptr], 0);
        }

        assert!(vm.stack[0].as_bool());

        for (value, expected) in [(0.0, false), (0.3, true), (-1.0, true), (f64::NAN, true)] {
            let mut compiler = Compiler::new();
            let field_idx = compiler.add_field(FieldId {
                component_id: ComponentId::new(0),
                offset: 0,
                field_type: FieldType::Bool,
            });
            let const_val = compiler.add_constant(value);
            compiler.emit(Op::PushFloatConst(const_val));
            compiler.emit(Op::StoreField(field_idx));
            let bytecode = compiler.finalize();

            let mut field_value = !expected;
            let field_ptr = &mut field_value as *mut bool as *mut u8;
            // SAFETY: The pointer is to the live exclusive bool and matches the declared field type.
            unsafe {
                VM::new().execute(&bytecode, &[field_ptr], 0);
            }
            assert_eq!(field_value, expected, "{value}");

            let mut eager = !expected;
            // SAFETY: The live typed allocation covers the batch count and stride; writes are exclusive.
            unsafe { write_field_value((&mut eager as *mut bool).cast(), value, FieldType::Bool) };
            assert_eq!(eager, expected, "{value}");
        }
    }

    #[test]
    fn test_execute_batch_f64_add_const() {
        // field += const over a contiguous f64 run
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(100.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // Contiguous f64 values (stride = 8 bytes)
        let count = 10;
        let mut values: Vec<f64> = (0..count).map(|i| i as f64).collect();
        let base_ptr = values.as_mut_ptr() as *mut u8;

        // SAFETY: The live typed allocation covers the batch count and stride; writes are exclusive.
        unsafe {
            vm.execute_batch_multi(
                &bytecode,
                &[base_ptr],
                &[std::mem::size_of::<f64>()],
                count,
                None,
            );
        }

        for (i, value) in values.iter().enumerate() {
            assert_eq!(
                *value,
                i as f64 + 100.0,
                "batch f64 add failed at index {}",
                i
            );
        }
    }

    #[test]
    fn test_execute_batch_f64_mul_const() {
        // field *= const over a contiguous f64 run
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(2.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::Mul);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let count = 10;
        let mut values: Vec<f64> = (1..=count).map(|i| i as f64).collect();
        let base_ptr = values.as_mut_ptr() as *mut u8;

        // SAFETY: The live typed allocation covers the batch count and stride; writes are exclusive.
        unsafe {
            vm.execute_batch_multi(
                &bytecode,
                &[base_ptr],
                &[std::mem::size_of::<f64>()],
                count,
                None,
            );
        }

        for (i, value) in values.iter().enumerate() {
            assert_eq!(
                *value,
                (i + 1) as f64 * 2.0,
                "batch f64 mul failed at index {}",
                i
            );
        }
    }

    #[test]
    fn test_execute_batch_f64_set_const() {
        // field = const over a contiguous f64 run
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(42.5);

        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let count = 10;
        let mut values: Vec<f64> = vec![0.0; count];
        let base_ptr = values.as_mut_ptr() as *mut u8;

        // SAFETY: The live typed allocation covers the batch count and stride; writes are exclusive.
        unsafe {
            vm.execute_batch_multi(
                &bytecode,
                &[base_ptr],
                &[std::mem::size_of::<f64>()],
                count,
                None,
            );
        }

        for value in &values {
            assert_eq!(*value, 42.5);
        }
    }

    #[test]
    fn test_execute_batch_f32_preserves_precision() {
        // f32 rows widen to f64 and narrow on store
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(1.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // Use > 4 entities to test the unrolled loop
        let count = 10;
        let mut values: Vec<f32> = (0..count).map(|i| i as f32).collect();
        let base_ptr = values.as_mut_ptr() as *mut u8;

        // SAFETY: The live typed allocation covers the batch count and stride; writes are exclusive.
        unsafe {
            vm.execute_batch_multi(
                &bytecode,
                &[base_ptr],
                &[std::mem::size_of::<f32>()],
                count,
                None,
            );
        }

        for (i, value) in values.iter().enumerate() {
            assert_eq!(
                *value,
                (i + 1) as f32,
                "batch f32 add failed at index {}",
                i
            );
        }
    }

    #[test]
    fn test_execute_and_reduce_f64() {
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(3.0);

        // Expression: field * 3.0
        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::Mul);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let mut field_value = 7.0_f64;
        let field_ptr = &mut field_value as *mut f64 as *mut u8;

        // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
        let result = unsafe { vm.execute_and_reduce(&bytecode, &[field_ptr], 0) };

        assert_eq!(result, 21.0);
        // Original value should be unchanged (reduce doesn't store)
        assert_eq!(field_value, 7.0);
    }

    /// Regression test: read/write f64 fields at 4-byte-aligned (but not
    /// 8-byte-aligned) addresses must not panic.
    #[test]
    fn test_unaligned_f64_field_access() {
        // Allocate a buffer with deliberate 4-byte misalignment for f64.
        // Layout: [4-byte padding] [f64 value] [f64 value]
        let mut buf = vec![0u8; 32];
        // Find a 4-byte-aligned-but-not-8-byte-aligned address within buf
        let base = buf.as_mut_ptr() as usize;
        let offset = if base.is_multiple_of(8) { 4 } else { 0 };
        // SAFETY: The offset is at most four bytes inside the 32-byte allocation.
        let misaligned_ptr = unsafe { buf.as_mut_ptr().add(offset) };
        assert_eq!(
            misaligned_ptr as usize % 8,
            4,
            "test setup: pointer should be 4-byte aligned but NOT 8-byte aligned"
        );

        // Write a known f64 value at the misaligned address
        // SAFETY: The live byte buffer covers every field; accesses use unaligned operations.
        unsafe {
            write_field_value(misaligned_ptr, 123.456, FieldType::F64);
        }

        // Read it back - must not panic
        // SAFETY: The live byte buffer covers every field; accesses use unaligned operations.
        let read_back = unsafe { read_field_value(misaligned_ptr as *const u8, FieldType::F64) };
        assert_eq!(read_back, 123.456);

        // Now test via the VM: compile `field = field + 1.0`
        let mut compiler = Compiler::new();
        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_1 = compiler.add_constant(1.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_1));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // SAFETY: The live byte buffer covers every field; accesses use unaligned operations.
        unsafe {
            vm.execute(&bytecode, &[misaligned_ptr], 0);
        }

        // SAFETY: The live byte buffer covers every field; accesses use unaligned operations.
        let result = unsafe { read_field_value(misaligned_ptr as *const u8, FieldType::F64) };
        assert_eq!(result, 124.456);
    }

    #[test]
    fn test_unaligned_i64_u64_field_access() {
        let mut buf = vec![0u8; 32];
        let base = buf.as_mut_ptr() as usize;
        let offset = if base.is_multiple_of(8) { 4 } else { 0 };
        // SAFETY: The offset is at most four bytes inside the 32-byte allocation.
        let misaligned_ptr = unsafe { buf.as_mut_ptr().add(offset) };
        assert_eq!(misaligned_ptr as usize % 8, 4);

        // i64
        // SAFETY: The live byte buffer covers every field; accesses use unaligned operations.
        unsafe {
            write_field_value(misaligned_ptr, 42.0, FieldType::I64);
        }
        // SAFETY: The live byte buffer covers every field; accesses use unaligned operations.
        let v = unsafe { read_field_value(misaligned_ptr as *const u8, FieldType::I64) };
        assert_eq!(v, 42.0);

        // u64
        // SAFETY: The live byte buffer covers every field; accesses use unaligned operations.
        unsafe {
            write_field_value(misaligned_ptr, 99.0, FieldType::U64);
        }
        // SAFETY: The live byte buffer covers every field; accesses use unaligned operations.
        let v = unsafe { read_field_value(misaligned_ptr as *const u8, FieldType::U64) };
        assert_eq!(v, 99.0);
    }

    /// Regression test: batch assignment must handle misaligned field
    /// addresses, since ECS column bytes have no alignment guarantee for
    /// embedded fields.
    #[test]
    fn test_unaligned_f32_batch_assignment() {
        let compile = |ops: &[Op], constant: f64| {
            let mut compiler = Compiler::new();
            let field_id = FieldId {
                component_id: ComponentId::new(0),
                offset: 0,
                field_type: FieldType::F32,
            };
            let field_idx = compiler.add_field(field_id);
            let const_val = compiler.add_constant(constant);
            for op in ops {
                compiler.emit(match op {
                    Op::PushField(_) => Op::PushField(field_idx),
                    Op::PushConst(_) => Op::PushConst(const_val),
                    Op::StoreField(_) => Op::StoreField(field_idx),
                    other => *other,
                });
            }
            compiler.finalize()
        };

        // Odd stride keeps every entity's f32 misaligned.
        let count = 10;
        let stride = 5;
        let mut buf = vec![0u8; count * stride + 1];
        // SAFETY: The allocation includes the leading byte plus every strided f32.
        let base_ptr = unsafe { buf.as_mut_ptr().add(1) };
        assert_ne!(base_ptr as usize % 4, 0, "base pointer must be misaligned");

        let read_all = |buf: &[u8]| -> Vec<f32> {
            (0..count)
                // SAFETY: The live byte buffer covers every field; accesses use unaligned operations.
                .map(|i| unsafe {
                    (buf.as_ptr().add(1 + i * stride) as *const f32).read_unaligned()
                })
                .collect()
        };

        let mut vm = VM::new();

        // field = const
        let assign = compile(&[Op::PushConst(0), Op::StoreField(0)], 2.0);
        // SAFETY: The live byte buffer covers every field; accesses use unaligned operations.
        unsafe {
            vm.execute_batch_multi(&assign, &[base_ptr], &[stride], count, None);
        }
        assert!(read_all(&buf).iter().all(|v| *v == 2.0));

        // field = field + const (unrolled loop + remainder)
        let add = compile(
            &[
                Op::PushField(0),
                Op::PushConst(0),
                Op::Add,
                Op::StoreField(0),
            ],
            1.5,
        );
        // SAFETY: The live byte buffer covers every field; accesses use unaligned operations.
        unsafe {
            vm.execute_batch_multi(&add, &[base_ptr], &[stride], count, None);
        }
        assert!(read_all(&buf).iter().all(|v| *v == 3.5));

        // field = field * const
        let mul = compile(
            &[
                Op::PushField(0),
                Op::PushConst(0),
                Op::Mul,
                Op::StoreField(0),
            ],
            2.0,
        );
        // SAFETY: The live byte buffer covers every field; accesses use unaligned operations.
        unsafe {
            vm.execute_batch_multi(&mul, &[base_ptr], &[stride], count, None);
        }
        assert!(read_all(&buf).iter().all(|v| *v == 7.0));
    }

    /// Eq uses exact comparison, not epsilon. Nearly-equal values must
    /// compare as not-equal (regression: old batch modes used epsilon).
    #[test]
    fn test_eq_exact_not_epsilon() {
        let mut compiler = Compiler::new();
        let a = compiler.add_constant(1.0);
        let b = compiler.add_constant(1.0 + f64::EPSILON);

        compiler.emit(Op::PushConst(a));
        compiler.emit(Op::PushConst(b));
        compiler.emit(Op::Eq);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };

        assert!(!vm.stack[0].as_bool(), "1.0 != 1.0+EPSILON (exact)");
    }

    #[test]
    fn test_ne_operator() {
        let mut compiler = Compiler::new();
        let a = compiler.add_constant(3.0);
        let b = compiler.add_constant(4.0);

        compiler.emit(Op::PushConst(a));
        compiler.emit(Op::PushConst(b));
        compiler.emit(Op::Ne);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };

        assert!(vm.stack[0].as_bool(), "3.0 != 4.0");

        // Same values should be not Ne
        let mut compiler = Compiler::new();
        let a = compiler.add_constant(5.0);
        let b = compiler.add_constant(5.0);

        compiler.emit(Op::PushConst(a));
        compiler.emit(Op::PushConst(b));
        compiler.emit(Op::Ne);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };

        assert!(!vm.stack[0].as_bool(), "5.0 == 5.0 so Ne is false");
    }

    /// Sign propagates NaN, matching Python array and expression libraries.
    #[test]
    fn test_sign_nan_propagates() {
        let mut compiler = Compiler::new();
        let nan = compiler.add_constant(f64::NAN);

        compiler.emit(Op::PushConst(nan));
        compiler.emit(Op::Sign);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };

        assert!(vm.stack[0].as_float().is_nan());
    }

    #[test]
    fn test_sign_positive_negative_and_signed_zero() {
        // Positive
        let mut compiler = Compiler::new();
        let c = compiler.add_constant(42.0);
        compiler.emit(Op::PushConst(c));
        compiler.emit(Op::Sign);
        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert_eq!(vm.stack[0].as_float(), 1.0);

        // Negative
        let mut compiler = Compiler::new();
        let c = compiler.add_constant(-7.5);
        compiler.emit(Op::PushConst(c));
        compiler.emit(Op::Sign);
        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert_eq!(vm.stack[0].as_float(), -1.0);

        // Zero
        let mut compiler = Compiler::new();
        let c = compiler.add_constant(0.0);
        compiler.emit(Op::PushConst(c));
        compiler.emit(Op::Sign);
        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert_eq!(vm.stack[0].as_float(), 0.0);

        // Preserve negative zero rather than converting it to -1.0.
        let mut compiler = Compiler::new();
        let c = compiler.add_constant(-0.0);
        compiler.emit(Op::PushConst(c));
        compiler.emit(Op::Sign);
        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        let result = vm.stack[0].as_float();
        assert_eq!(result, 0.0);
        assert!(result.is_sign_negative());
    }

    #[test]
    fn test_python_remainder_uses_divisor_sign() {
        assert_eq!(python_remainder(-3.0, 2.0), 1.0);
        assert_eq!(python_remainder(3.0, -2.0), -1.0);
        assert!(python_remainder(1.0, 0.0).is_nan());

        let zero = python_remainder(-4.0, 2.0);
        assert_eq!(zero, 0.0);
        assert!(!zero.is_sign_negative());
        let negative_zero = python_remainder(4.0, -2.0);
        assert_eq!(negative_zero, 0.0);
        assert!(negative_zero.is_sign_negative());
    }

    #[test]
    fn integer_floor_division_and_modulo_preserve_large_signed_values() {
        for (numerator, denominator) in [(-7, 2), (7, -2), (-7, -2), ((1 << 60) + 1, 3)] {
            let (StackValue::I64(quotient), StackValue::I64(remainder)) = (
                floor_divide(StackValue::I64(numerator), StackValue::I64(denominator)),
                remainder(StackValue::I64(numerator), StackValue::I64(denominator)),
            ) else {
                panic!("integer operands must retain the i64 lane");
            };
            assert_eq!(quotient * denominator + remainder, numerator);
            assert!(remainder == 0 || (remainder < 0) == (denominator < 0));
            assert!(remainder.abs() < denominator.abs());
        }
    }

    #[test]
    fn integer_zero_divisor_and_minimum_quotient_follow_numpy() {
        let pairs = [
            (StackValue::I64(5), StackValue::I64(0), 0, 0),
            (StackValue::I64(i64::MIN), StackValue::I64(-1), i64::MIN, 0),
            (
                StackValue::I32(i32::MIN),
                StackValue::I32(-1),
                i64::from(i32::MIN),
                0,
            ),
            (StackValue::U32(5), StackValue::U32(0), 0, 0),
        ];
        for (a, b, quotient, modulus) in pairs {
            assert_eq!(floor_divide(a, b).as_float(), quotient as f64);
            assert_eq!(remainder(a, b).as_float(), modulus as f64);
        }
    }

    #[test]
    fn float_floor_division_matches_numpy() {
        let cases = [
            (7.0, -2.0, -4.0),
            (-7.0, 2.0, -4.0),
            (7.5, -2.0, -4.0),
            (1.0, 0.0, f64::INFINITY),
            (-1.0, 0.0, f64::NEG_INFINITY),
            (1.0, -0.0, f64::NEG_INFINITY),
        ];
        for (a, b, expected) in cases {
            assert_eq!(numpy_floor_divide(a, b), expected);
            assert_eq!(numpy_floor_divide_f32(a as f32, b as f32), expected as f32);
        }
        assert!(numpy_floor_divide(0.0, 0.0).is_nan());
        assert!(numpy_floor_divide(f64::INFINITY, 2.0).is_nan());
        assert!(numpy_floor_divide(0.0, -2.0).is_sign_negative());
        assert!(python_remainder(1.0, 0.0).is_nan());
    }

    #[test]
    fn test_python_round_uses_ties_to_even() {
        assert_eq!(python_round(2.5), 2.0);
        assert_eq!(python_round(3.5), 4.0);
        assert_eq!(python_round(-2.5), -2.0);
        assert_eq!(python_round(-3.5), -4.0);
    }

    #[test]
    fn test_python_minimum_and_maximum_propagate_nan() {
        assert!(python_minimum(f64::NAN, 1.0).is_nan());
        assert!(python_minimum(1.0, f64::NAN).is_nan());
        assert!(python_maximum(f64::NAN, 1.0).is_nan());
        assert!(python_maximum(1.0, f64::NAN).is_nan());
    }

    #[test]
    fn test_python_clip_propagates_nan_and_accepts_reversed_bounds() {
        assert_eq!(python_clip(-1.0, 0.0, 1.0), 0.0);
        assert_eq!(python_clip(0.5, 0.0, 1.0), 0.5);
        assert_eq!(python_clip(2.0, 0.0, 1.0), 1.0);
        assert_eq!(python_clip(-1.0, 2.0, 1.0), 1.0);
        assert_eq!(python_clip(0.5, 2.0, 1.0), 1.0);
        assert_eq!(python_clip(2.0, 2.0, 1.0), 1.0);
        assert!(python_clip(f64::NAN, 0.0, 1.0).is_nan());
        assert!(python_clip(0.5, f64::NAN, 1.0).is_nan());
        assert!(python_clip(0.5, 0.0, f64::NAN).is_nan());
    }

    #[test]
    fn integer_identity_does_not_round_through_f64() {
        let mut compiler = Compiler::new();
        let field = compiler.add_field(FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::I64,
        });
        compiler.emit(Op::PushField(field));
        compiler.emit(Op::StoreField(field));
        let bytecode = compiler.finalize();

        let mut value = (1_i64 << 53) + 1;
        let pointers = [(&mut value as *mut i64).cast::<u8>()];
        let mut vm = VM::new();
        // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
        unsafe { vm.execute(&bytecode, &pointers, 0) };

        assert_eq!(value, (1_i64 << 53) + 1);
    }

    #[test]
    fn u8_arithmetic_uses_wrapping_native_lane_semantics() {
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

        let mut value = 250_u8;
        let pointers = [(&mut value as *mut u8).cast::<u8>()];
        let mut vm = VM::new();
        // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
        unsafe { vm.execute(&bytecode, &pointers, 0) };

        assert_eq!(value, 4);
    }

    #[test]
    fn integer_power_wraps_like_numpy() {
        let cases = [
            (
                StackValue::I64(3),
                StackValue::Constant(41.0),
                -420_491_770_248_316_829,
            ),
            (StackValue::Constant(2.0), StackValue::I64(63), i64::MIN),
            (StackValue::Constant(2.0), StackValue::I64(64), 0),
            (StackValue::I64(-3), StackValue::Constant(3.0), -27),
            (StackValue::I64(0), StackValue::Constant(0.0), 1),
        ];
        for (base, exponent, expected) in cases {
            assert_eq!(power(base, exponent), (StackValue::I64(expected), false));
        }
        assert_eq!(
            power(StackValue::I32(70_000), StackValue::Constant(2.0)).0,
            StackValue::I32(605_032_704)
        );
        assert_eq!(
            power(StackValue::U8(3), StackValue::U8(7)).0,
            StackValue::U8(139)
        );
        assert_eq!(
            power(StackValue::U32(70_000), StackValue::Constant(2.0)).0,
            StackValue::U32(605_032_704)
        );
        assert_eq!(
            power(StackValue::U64(u64::MAX), StackValue::Constant(3.0)).0,
            StackValue::U64(u64::MAX)
        );
    }

    #[test]
    fn int_literals_beyond_f64_stay_exact_in_integer_lanes() {
        let i64_max = StackValue::int_literal(i64::MAX.into());
        let u64_max = StackValue::int_literal(u64::MAX.into());
        let below_i64 = StackValue::int_literal(-(1_i128 << 63) - 1);
        assert_eq!(i64_max, StackValue::ExactInt(i64::MAX));
        assert_eq!(u64_max, StackValue::ExactUInt(u64::MAX));
        assert_eq!(below_i64, StackValue::Constant(-(2_f64.powi(63))));
        assert_eq!(
            StackValue::int_literal(1 << 60),
            StackValue::Constant(2_f64.powi(60))
        );

        let mut vm = VM::new();
        let bytecode = CompiledBytecode {
            bytecode: Vec::new(),
            constants: Vec::new(),
            integers: vec![i64::MAX.into(), u64::MAX.into(), (1 << 53) + 1],
            field_map: Vec::new(),
        };
        let run = |vm: &mut VM, field: StackValue, integer: u16, op: Op| {
            vm.stack.clear();
            vm.stack.push(field);
            vm.dispatch_stack_op(&Op::PushInt(integer), &bytecode);
            vm.dispatch_stack_op(&op, &bytecode);
            vm.stack.pop().unwrap()
        };
        assert_eq!(
            run(&mut vm, StackValue::I64(-5), 0, Op::Add),
            StackValue::I64(i64::MAX - 5)
        );
        assert_eq!(
            run(&mut vm, StackValue::I64(2), 0, Op::Add),
            StackValue::I64(i64::MIN + 1)
        );
        assert_eq!(
            run(&mut vm, StackValue::U64(1), 1, Op::Sub),
            StackValue::U64(2)
        );
        assert_eq!(
            run(&mut vm, StackValue::U64(3), 1, Op::Mul),
            StackValue::U64(u64::MAX - 2)
        );
        assert_eq!(
            run(&mut vm, StackValue::I64((1 << 53) + 1), 2, Op::Eq),
            StackValue::Bool(true)
        );
        assert_eq!(
            run(&mut vm, StackValue::I64(1 << 53), 2, Op::Lt),
            StackValue::Bool(true)
        );
        assert_eq!(
            run(&mut vm, StackValue::Float(1.0), 2, Op::Add),
            StackValue::Float(((1_i64 << 53) + 1) as f64 + 1.0)
        );
        vm.stack.clear();
        vm.dispatch_stack_op(&Op::PushInt(0), &bytecode);
        vm.dispatch_stack_op(&Op::Neg, &bytecode);
        assert_eq!(vm.stack, [StackValue::ExactInt(-i64::MAX)]);
    }

    #[test]
    fn int_literal_stored_into_an_integer_field_wraps_like_numpy() {
        let mut value = 7_u32;
        let mut signed = 7_i32;
        // SAFETY: both pointers address live, exclusively borrowed locals of
        // the written field types.
        unsafe {
            write_field_stack_value(
                (&mut value as *mut u32).cast(),
                StackValue::Constant(2_f64.powi(32) + 3.0),
                FieldType::U32,
            );
            write_field_stack_value(
                (&mut signed as *mut i32).cast(),
                StackValue::Constant(-5.0),
                FieldType::I32,
            );
        }
        assert_eq!((value, signed), (3, -5));
        let mut wide = 0_u64;
        let mut narrow = 0_u32;
        // SAFETY: both pointers address live, exclusively borrowed locals of
        // the written field types.
        unsafe {
            write_field_stack_value(
                (&mut wide as *mut u64).cast(),
                StackValue::ExactUInt(u64::MAX),
                FieldType::U64,
            );
            write_field_stack_value(
                (&mut narrow as *mut u32).cast(),
                StackValue::ExactInt(i64::MAX),
                FieldType::U32,
            );
        }
        assert_eq!((wide, narrow), (u64::MAX, u32::MAX));
    }

    #[test]
    fn integer_stores_between_widths_wrap_exactly_like_numpy() {
        let mut narrow = [0_i32; 2];
        let mut signed = [0_i64; 2];
        let mut byte = 0_u8;
        // SAFETY: every pointer addresses a live, exclusively borrowed local of
        // the written field type.
        unsafe {
            for (slot, value) in narrow
                .iter_mut()
                .zip([(1_i64 << 31) + 5, -(1_i64 << 40) - 3])
            {
                write_field_stack_value(
                    (slot as *mut i32).cast(),
                    StackValue::I64(value),
                    FieldType::I32,
                );
            }
            for (slot, value) in signed.iter_mut().zip([u64::MAX, (1_u64 << 53) + 1]) {
                write_field_stack_value(
                    (slot as *mut i64).cast(),
                    StackValue::U64(value),
                    FieldType::I64,
                );
            }
            write_field_stack_value(&mut byte, StackValue::U32(300), FieldType::U8);
        }
        assert_eq!(narrow, [-2_147_483_643, -3]);
        assert_eq!(signed, [-1, (1_i64 << 53) + 1]);
        assert_eq!(byte, 44);
    }

    #[test]
    fn negative_runtime_integer_exponent_is_flagged_and_truncated() {
        for (base, exponent, expected) in [
            (2, -1, 0),
            (1, -5, 1),
            (-1, -3, -1),
            (-1, -2, 1),
            (0, -1, 0),
        ] {
            assert_eq!(
                power(StackValue::I64(base), StackValue::I64(exponent)),
                (StackValue::I64(expected), true)
            );
        }
        assert!(!power(StackValue::I64(2), StackValue::I64(0)).1);
        assert!(!power(StackValue::U8(2), StackValue::U8(200)).1);

        let bytecode = CompiledBytecode {
            bytecode: Vec::new(),
            constants: Vec::new(),
            integers: Vec::new(),
            field_map: Vec::new(),
        };
        let mut vm = VM::new();
        vm.stack.extend([StackValue::I32(2), StackValue::I32(-1)]);
        vm.dispatch_stack_op(&Op::Pow, &bytecode);
        vm.stack.extend([StackValue::I32(2), StackValue::I32(3)]);
        vm.dispatch_stack_op(&Op::Pow, &bytecode);
        assert_eq!(vm.stack, [StackValue::I32(0), StackValue::I32(8)]);
        assert_eq!(vm.take_negative_power(), Err(NegativeIntegerPower));
        assert_eq!(vm.take_negative_power(), Ok(()));
    }

    #[test]
    fn float_literal_promotes_integer_lane_and_int_literal_adopts_it() {
        let seven = StackValue::I64(7);
        assert_eq!(
            floor_divide(seven, StackValue::Constant(2.0)),
            StackValue::I64(3)
        );
        assert_eq!(
            floor_divide(seven, StackValue::FloatConstant(2.0)),
            StackValue::Float(3.0)
        );
        assert_eq!(
            power(seven, StackValue::FloatConstant(2.0)).0,
            StackValue::Float(49.0)
        );
        assert_eq!(
            power(StackValue::F32(1.5), StackValue::FloatConstant(2.0)).0,
            StackValue::F32(2.25)
        );

        let mut vm = VM::new();
        let bytecode = CompiledBytecode {
            bytecode: Vec::new(),
            constants: vec![2.0],
            integers: Vec::new(),
            field_map: Vec::new(),
        };
        for (op, expected) in [
            (Op::Add, StackValue::Float(9.0)),
            (Op::Mul, StackValue::Float(14.0)),
            (Op::Min, StackValue::Float(2.0)),
            (Op::Mod, StackValue::Float(1.0)),
        ] {
            vm.stack.clear();
            vm.stack.push(seven);
            vm.dispatch_stack_op(&Op::PushFloatConst(0), &bytecode);
            vm.dispatch_stack_op(&op, &bytecode);
            assert_eq!(vm.stack, [expected], "{op:?}");
        }
    }

    #[test]
    fn literal_only_results_keep_their_kind() {
        assert_eq!(
            power(StackValue::Constant(2.0), StackValue::Constant(3.0)).0,
            StackValue::Constant(8.0)
        );
        assert_eq!(
            power(StackValue::Constant(2.0), StackValue::FloatConstant(3.0)).0,
            StackValue::FloatConstant(8.0)
        );
        assert_eq!(
            floor_divide(StackValue::Constant(7.0), StackValue::FloatConstant(2.0)),
            StackValue::FloatConstant(3.0)
        );

        let mut vm = VM::new();
        let bytecode = CompiledBytecode {
            bytecode: Vec::new(),
            constants: vec![4.0],
            integers: Vec::new(),
            field_map: Vec::new(),
        };
        vm.dispatch_stack_op(&Op::PushConst(0), &bytecode);
        vm.dispatch_stack_op(&Op::Sqrt, &bytecode);
        assert_eq!(vm.stack, [StackValue::FloatConstant(2.0)]);
    }

    #[test]
    fn optimizer_folds_literals_into_the_resulting_kind() {
        let fold = |a: Op, b: Op, op: Op, constants: &[f64]| {
            let mut compiler = Compiler::new();
            for value in constants {
                compiler.add_constant(*value);
            }
            compiler.emit(a);
            compiler.emit(b);
            compiler.emit(op);
            compiler.optimize();
            let bytecode = compiler.finalize();
            match bytecode.bytecode[..] {
                [Op::PushConst(index)] => Some((bytecode.constants[usize::from(index)], false)),
                [Op::PushFloatConst(index)] => Some((bytecode.constants[usize::from(index)], true)),
                _ => None,
            }
        };
        let int_2 = Op::PushConst(0);
        let int_3 = Op::PushConst(1);
        let float_3 = Op::PushFloatConst(1);
        let int_negative = Op::PushConst(2);
        let constants = [2.0, 3.0, -1.0];
        assert_eq!(fold(int_2, int_3, Op::Add, &constants), Some((5.0, false)));
        assert_eq!(fold(int_2, float_3, Op::Mul, &constants), Some((6.0, true)));
        assert_eq!(
            fold(int_2, int_3, Op::Div, &constants),
            Some((2.0 / 3.0, true))
        );
        assert_eq!(fold(int_2, int_3, Op::Pow, &constants), Some((8.0, false)));
        assert_eq!(fold(int_2, float_3, Op::Pow, &constants), Some((8.0, true)));
        assert_eq!(fold(int_2, int_negative, Op::Pow, &constants), None);
    }

    fn folded(ops: &[Op], constants: &[f64], integers: &[i128]) -> CompiledBytecode {
        let mut compiler = Compiler::new();
        for value in constants {
            compiler.add_constant(*value);
        }
        for value in integers {
            compiler.integers.push(*value);
        }
        for op in ops {
            compiler.emit(*op);
        }
        compiler.optimize();
        compiler.finalize()
    }

    fn single_literal(bytecode: &CompiledBytecode) -> Option<(f64, bool)> {
        match bytecode.bytecode[..] {
            [Op::PushConst(index)] => Some((bytecode.constants[usize::from(index)], false)),
            [Op::PushFloatConst(index)] => Some((bytecode.constants[usize::from(index)], true)),
            _ => None,
        }
    }

    #[test]
    fn optimizer_folds_nested_and_unary_literal_operations() {
        let (one, two, three) = (Op::PushConst(0), Op::PushConst(1), Op::PushConst(2));
        let constants = [1.0, 2.0, 3.0, -7.0, 0.5];
        let cases: [(&[Op], (f64, bool)); 8] = [
            (&[one, two, three, Op::Add, Op::Add], (6.0, false)),
            (&[one, two, Op::Add, three, Op::Mul], (9.0, false)),
            (&[three, Op::Neg, Op::Abs], (3.0, false)),
            (&[three, two, Op::Min, one, Op::Max], (2.0, false)),
            (&[Op::PushConst(3), two, Op::FloorDiv], (-4.0, false)),
            (&[Op::PushConst(3), two, Op::Mod], (1.0, false)),
            (&[Op::PushConst(3), Op::Sign], (-1.0, false)),
            (
                &[Op::PushFloatConst(4), Op::Neg, three, Op::Max],
                (3.0, true),
            ),
        ];
        for (ops, expected) in cases {
            assert_eq!(
                single_literal(&folded(ops, &constants, &[])),
                Some(expected),
                "{ops:?}"
            );
        }
    }

    #[test]
    fn optimizer_folds_exact_integer_literals_beyond_f64() {
        let bytecode = folded(
            &[
                Op::PushConst(0),
                Op::PushInt(0),
                Op::PushConst(0),
                Op::Add,
                Op::Add,
            ],
            &[1.0],
            &[(1 << 62) + 1],
        );
        assert!(matches!(bytecode.bytecode[..], [Op::PushInt(_)]));
        assert!(bytecode.integers.contains(&((1 << 62) + 3)));
        let min = folded(
            &[Op::PushInt(0), Op::PushInt(1), Op::Min],
            &[],
            &[(1 << 64) + 1, (1 << 65) + 1],
        );
        assert!(
            matches!(min.bytecode[..], [Op::PushInt(index)] if min.integers[usize::from(index)] == (1 << 64) + 1)
        );
    }

    #[test]
    fn optimizer_leaves_zero_divisors_and_overflow_to_the_vm() {
        for op in [Op::Div, Op::FloorDiv, Op::Mod] {
            let bytecode = folded(&[Op::PushConst(0), Op::PushConst(1), op], &[7.0, 0.0], &[]);
            assert_eq!(bytecode.bytecode.len(), 3, "{op:?}");
        }
        let bytecode = folded(
            &[Op::PushInt(0), Op::PushInt(1), Op::FloorDiv],
            &[],
            &[i128::MIN, -1],
        );
        assert_eq!(bytecode.bytecode.len(), 3);
    }

    #[test]
    fn pools_past_u16_indices_are_rejected_instead_of_aliasing() {
        let full = |count: usize| {
            let mut compiler = Compiler::new();
            for value in 0..count {
                let index = compiler.add_constant(value as f64);
                compiler.emit(Op::PushConst(index));
            }
            compiler
        };
        assert!(full(EXPRESSION_MAX_POOL_ENTRIES).try_finalize().is_ok());
        assert_eq!(
            full(EXPRESSION_MAX_POOL_ENTRIES + 1).try_finalize().err(),
            Some(ProgramTooLarge)
        );

        let mut fields = Compiler::new();
        for offset in 0..=EXPRESSION_MAX_POOL_ENTRIES {
            fields.add_field(FieldId {
                component_id: ComponentId::new(0),
                offset,
                field_type: FieldType::U8,
            });
        }
        assert_eq!(fields.try_finalize().err(), Some(ProgramTooLarge));

        let mut integers = Compiler::new();
        for value in 0..=EXPRESSION_MAX_POOL_ENTRIES as i128 {
            integers.int_literal((1 << 60) + 2 * value + 1);
        }
        assert_eq!(integers.try_finalize().err(), Some(ProgramTooLarge));
    }

    #[test]
    fn i64_comparison_is_exact_above_f64_integer_precision() {
        let mut compiler = Compiler::new();
        let left = compiler.add_field(FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::I64,
        });
        let right = compiler.add_field(FieldId {
            component_id: ComponentId::new(1),
            offset: 0,
            field_type: FieldType::I64,
        });
        compiler.emit(Op::PushField(left));
        compiler.emit(Op::PushField(right));
        compiler.emit(Op::Gt);
        let bytecode = compiler.finalize();

        let mut left_value = (1_i64 << 53) + 1;
        let mut right_value = 1_i64 << 53;
        let pointers = [
            (&mut left_value as *mut i64).cast::<u8>(),
            (&mut right_value as *mut i64).cast::<u8>(),
        ];
        let mut vm = VM::new();
        // SAFETY: Field pointers match live local values and declared types; accesses are sequential.
        unsafe { vm.execute(&bytecode, &pointers, 0) };

        assert!(vm.stack[0].as_bool());
    }

    #[test]
    fn integer_float_comparison_is_exact_at_domain_boundaries() {
        assert_eq!(
            numeric_cmp(
                StackValue::U64(u64::MAX),
                StackValue::Constant(2_f64.powi(64))
            ),
            Some(Ordering::Less)
        );
        assert_eq!(
            numeric_cmp(
                StackValue::I64(i64::MAX),
                StackValue::Constant(2_f64.powi(63))
            ),
            Some(Ordering::Less)
        );
        assert_eq!(
            numeric_cmp(
                StackValue::I64(i64::MIN),
                StackValue::Constant(-2_f64.powi(63))
            ),
            Some(Ordering::Equal)
        );
        assert_eq!(
            numeric_cmp(StackValue::I64(-1), StackValue::Float(-1.5)),
            Some(Ordering::Greater)
        );
        assert_eq!(
            numeric_cmp(StackValue::U64(1), StackValue::Float(1.5)),
            Some(Ordering::Less)
        );
        assert_eq!(
            numeric_cmp(StackValue::I64(-1), StackValue::U64(0)),
            Some(Ordering::Less)
        );
        assert_eq!(
            numeric_cmp(StackValue::U64(u64::MAX), StackValue::I64(i64::MAX)),
            Some(Ordering::Greater)
        );
        assert_eq!(
            numeric_cmp(StackValue::U64(u64::MAX), StackValue::Float(f64::NAN)),
            None
        );
    }
}
