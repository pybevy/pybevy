//! Expression AST and compiler for lazy batch operations.
//!
//! This module provides an Abstract Syntax Tree (AST) for mathematical expressions
//! that can be compiled to bytecode for efficient execution.

use std::{
    hash::{Hash, Hasher},
    mem::discriminant,
};

use bevy_ecs::component::ComponentId;
use pybevy_storage::storage_error::{EXPRESSION_MAX_DEPTH, expression_too_deep};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyBool, PyFloat, PyInt},
};

use super::bytecode::{Compiler, FieldId, FieldType, Op};

/// Extract exactly `N` expression arguments, rejecting malformed `args` lists
/// with a `TypeError`.
fn expect_args<'py, const N: usize>(
    op_name: &str,
    args: &Bound<'py, PyAny>,
) -> PyResult<[Bound<'py, PyAny>; N]> {
    let args_list = args.extract::<Vec<Bound<'py, PyAny>>>()?;
    let got = args_list.len();
    <[Bound<'py, PyAny>; N]>::try_from(args_list).map_err(|_| {
        PyTypeError::new_err(format!(
            "{op_name} operation requires {N} argument{}, got {got}",
            if N == 1 { "" } else { "s" }
        ))
    })
}

fn parse_random_stream(value: &Bound<'_, PyAny>) -> PyResult<u32> {
    if value.is_instance_of::<PyBool>() || !value.is_instance_of::<PyInt>() {
        return Err(PyTypeError::new_err("random stream must be an integer"));
    }
    value
        .extract::<u32>()
        .map_err(|_| PyValueError::new_err("random stream must be between 0 and 4294967295"))
}

/// Abstract Syntax Tree node for lazy expressions
#[derive(Debug, Clone)]
pub enum RustExpr {
    /// Binary addition: left + right
    Add(Box<RustExpr>, Box<RustExpr>),

    /// Binary subtraction: left - right
    Sub(Box<RustExpr>, Box<RustExpr>),

    /// Binary multiplication: left * right
    Mul(Box<RustExpr>, Box<RustExpr>),

    /// Binary division: left / right
    Div(Box<RustExpr>, Box<RustExpr>),

    /// Binary floor division: left // right
    FloorDiv(Box<RustExpr>, Box<RustExpr>),

    /// Binary power: left ** right
    Pow(Box<RustExpr>, Box<RustExpr>),

    /// Unary negation: -expr
    Neg(Box<RustExpr>),

    /// Sine function: sin(expr)
    Sin(Box<RustExpr>),

    /// Cosine function: cos(expr)
    Cos(Box<RustExpr>),

    /// Tangent function: tan(expr)
    Tan(Box<RustExpr>),

    /// Arcsine function: asin(expr)
    Asin(Box<RustExpr>),

    /// Arccosine function: acos(expr)
    Acos(Box<RustExpr>),

    /// Arctangent function: atan(expr)
    Atan(Box<RustExpr>),

    /// Square root: sqrt(expr)
    Sqrt(Box<RustExpr>),

    /// Absolute value: abs(expr)
    Abs(Box<RustExpr>),

    /// Floor function: floor(expr)
    Floor(Box<RustExpr>),

    /// Ceiling function: ceil(expr)
    Ceil(Box<RustExpr>),

    /// Round function: round(expr)
    Round(Box<RustExpr>),

    /// Minimum: min(left, right)
    Min(Box<RustExpr>, Box<RustExpr>),

    /// Maximum: max(left, right)
    Max(Box<RustExpr>, Box<RustExpr>),

    /// Clamp: clamp(value, min, max)
    Clamp(Box<RustExpr>, Box<RustExpr>, Box<RustExpr>),

    /// Equality: left == right
    Eq(Box<RustExpr>, Box<RustExpr>),

    /// Inequality: left != right
    Ne(Box<RustExpr>, Box<RustExpr>),

    /// Less than: left < right
    Lt(Box<RustExpr>, Box<RustExpr>),

    /// Less than or equal: left <= right
    Le(Box<RustExpr>, Box<RustExpr>),

    /// Greater than: left > right
    Gt(Box<RustExpr>, Box<RustExpr>),

    /// Greater than or equal: left >= right
    Ge(Box<RustExpr>, Box<RustExpr>),

    /// Conditional selection: where(condition, true_value, false_value)
    Where(Box<RustExpr>, Box<RustExpr>, Box<RustExpr>),

    /// Logical AND: left && right
    And(Box<RustExpr>, Box<RustExpr>),

    /// Logical OR: left || right
    Or(Box<RustExpr>, Box<RustExpr>),

    /// Logical NOT: !expr
    Not(Box<RustExpr>),

    /// Exponential: e^expr
    Exp(Box<RustExpr>),

    /// Natural logarithm: ln(expr)
    Ln(Box<RustExpr>),

    /// Base-10 logarithm: log10(expr)
    Log10(Box<RustExpr>),

    /// Base-2 logarithm: log2(expr)
    Log2(Box<RustExpr>),

    /// Sign function: sign(expr) returns -1.0, 0.0, or 1.0
    Sign(Box<RustExpr>),

    /// Fractional part: fract(expr)
    Fract(Box<RustExpr>),

    /// Modulo: left % right
    Mod(Box<RustExpr>, Box<RustExpr>),

    /// Linear interpolation: lerp(a, b, t)
    Lerp(Box<RustExpr>, Box<RustExpr>, Box<RustExpr>),

    /// Random float in [0.0, 1.0): random(stream=0)
    Random(u32),

    /// Random float in [min, max): random_range(min, max, stream=0)
    RandomRange(Box<RustExpr>, Box<RustExpr>, u32),

    /// Component field access (e.g., pos.x, vel.y)
    Field {
        component_id: ComponentId,
        offset: usize,
        field_type: FieldType,
    },

    /// Python `int` (or `bool`) literal; it adopts the other operand's lane.
    Const(f64),

    /// Python `float` literal; it promotes an integer operand to f64.
    FloatConst(f64),

    /// Python `int` literal held exactly; it adopts the other operand's lane.
    Int(i128),
}

impl RustExpr {
    /// Exact structural equality for collision-safe program caching.
    ///
    /// Floating-point constants compare by bits so `0.0` and `-0.0`, and
    /// distinct NaN payloads, can never alias one cached program.
    pub(crate) fn canonical_eq(&self, other: &Self) -> bool {
        use RustExpr::*;

        match (self, other) {
            (Add(a1, a2), Add(b1, b2))
            | (Sub(a1, a2), Sub(b1, b2))
            | (Mul(a1, a2), Mul(b1, b2))
            | (Div(a1, a2), Div(b1, b2))
            | (FloorDiv(a1, a2), FloorDiv(b1, b2))
            | (Pow(a1, a2), Pow(b1, b2))
            | (Min(a1, a2), Min(b1, b2))
            | (Max(a1, a2), Max(b1, b2))
            | (Eq(a1, a2), Eq(b1, b2))
            | (Ne(a1, a2), Ne(b1, b2))
            | (Lt(a1, a2), Lt(b1, b2))
            | (Le(a1, a2), Le(b1, b2))
            | (Gt(a1, a2), Gt(b1, b2))
            | (Ge(a1, a2), Ge(b1, b2))
            | (And(a1, a2), And(b1, b2))
            | (Or(a1, a2), Or(b1, b2))
            | (Mod(a1, a2), Mod(b1, b2)) => a1.canonical_eq(b1) && a2.canonical_eq(b2),
            (RandomRange(a1, a2, as_), RandomRange(b1, b2, bs)) => {
                as_ == bs && a1.canonical_eq(b1) && a2.canonical_eq(b2)
            }
            (Neg(a), Neg(b))
            | (Sin(a), Sin(b))
            | (Cos(a), Cos(b))
            | (Tan(a), Tan(b))
            | (Asin(a), Asin(b))
            | (Acos(a), Acos(b))
            | (Atan(a), Atan(b))
            | (Sqrt(a), Sqrt(b))
            | (Abs(a), Abs(b))
            | (Floor(a), Floor(b))
            | (Ceil(a), Ceil(b))
            | (Round(a), Round(b))
            | (Not(a), Not(b))
            | (Exp(a), Exp(b))
            | (Ln(a), Ln(b))
            | (Log10(a), Log10(b))
            | (Log2(a), Log2(b))
            | (Sign(a), Sign(b))
            | (Fract(a), Fract(b)) => a.canonical_eq(b),
            (Clamp(a1, a2, a3), Clamp(b1, b2, b3))
            | (Where(a1, a2, a3), Where(b1, b2, b3))
            | (Lerp(a1, a2, a3), Lerp(b1, b2, b3)) => {
                a1.canonical_eq(b1) && a2.canonical_eq(b2) && a3.canonical_eq(b3)
            }
            (Random(a), Random(b)) => a == b,
            (
                Field {
                    component_id: ac,
                    offset: ao,
                    field_type: at,
                },
                Field {
                    component_id: bc,
                    offset: bo,
                    field_type: bt,
                },
            ) => ac == bc && ao == bo && at == bt,
            (Const(a), Const(b)) | (FloatConst(a), FloatConst(b)) => a.to_bits() == b.to_bits(),
            (Int(a), Int(b)) => a == b,
            _ => false,
        }
    }

    /// Structural hash consistent with [`RustExpr::canonical_eq`].
    pub(crate) fn canonical_hash<H: Hasher>(&self, state: &mut H) {
        use RustExpr::*;

        discriminant(self).hash(state);
        match self {
            Add(a, b)
            | Sub(a, b)
            | Mul(a, b)
            | Div(a, b)
            | FloorDiv(a, b)
            | Pow(a, b)
            | Min(a, b)
            | Max(a, b)
            | Eq(a, b)
            | Ne(a, b)
            | Lt(a, b)
            | Le(a, b)
            | Gt(a, b)
            | Ge(a, b)
            | And(a, b)
            | Or(a, b)
            | Mod(a, b) => {
                a.canonical_hash(state);
                b.canonical_hash(state);
            }
            RandomRange(a, b, stream) => {
                stream.hash(state);
                a.canonical_hash(state);
                b.canonical_hash(state);
            }
            Neg(a) | Sin(a) | Cos(a) | Tan(a) | Asin(a) | Acos(a) | Atan(a) | Sqrt(a) | Abs(a)
            | Floor(a) | Ceil(a) | Round(a) | Not(a) | Exp(a) | Ln(a) | Log10(a) | Log2(a)
            | Sign(a) | Fract(a) => a.canonical_hash(state),
            Clamp(a, b, c) | Where(a, b, c) | Lerp(a, b, c) => {
                a.canonical_hash(state);
                b.canonical_hash(state);
                c.canonical_hash(state);
            }
            Random(stream) => stream.hash(state),
            Field {
                component_id,
                offset,
                field_type,
            } => {
                component_id.hash(state);
                offset.hash(state);
                field_type.hash(state);
            }
            Const(value) | FloatConst(value) => value.to_bits().hash(state),
            Int(value) => value.hash(state),
        }
    }

    /// Parse a Python expression object into a Rust AST
    ///
    /// The Python object should have:
    /// - `op`: string describing the operation ("add", "mul", "field", "const", etc.)
    /// - `args`: list of arguments (child expressions or values)
    pub fn from_py_object(py: Python, obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        Self::from_py_object_at(py, obj, 0)
    }

    /// Convert one node at `depth`, rejecting a tree that would drive this walk,
    /// the compiler and the drop glue past the stack every backend can afford.
    #[allow(clippy::only_used_in_recursion)]
    fn from_py_object_at(py: Python, obj: &Bound<'_, PyAny>, depth: usize) -> PyResult<Self> {
        if depth >= EXPRESSION_MAX_DEPTH {
            return Err(PyValueError::new_err(expression_too_deep()));
        }

        if let Some(literal) = Self::literal(obj)? {
            return Ok(literal);
        }

        let op: String = obj
            .getattr("op")
            .map_err(|_| PyTypeError::new_err("Expression object must have 'op' attribute (str)"))?
            .extract()?;

        let args = obj.getattr("args").map_err(|_| {
            PyTypeError::new_err("Expression object must have 'args' attribute (list)")
        })?;

        let child = |arg: &Bound<'_, PyAny>| -> PyResult<Box<RustExpr>> {
            Self::from_py_object_at(py, arg, depth + 1).map(Box::new)
        };
        let unary = |name: &str, build: fn(Box<RustExpr>) -> RustExpr| -> PyResult<RustExpr> {
            let [value] = expect_args(name, &args)?;
            Ok(build(child(&value)?))
        };
        let binary = |name: &str,
                      build: fn(Box<RustExpr>, Box<RustExpr>) -> RustExpr|
         -> PyResult<RustExpr> {
            let [left, right] = expect_args(name, &args)?;
            Ok(build(child(&left)?, child(&right)?))
        };
        let ternary = |name: &str,
                       build: fn(Box<RustExpr>, Box<RustExpr>, Box<RustExpr>) -> RustExpr|
         -> PyResult<RustExpr> {
            let [first, second, third] = expect_args(name, &args)?;
            Ok(build(child(&first)?, child(&second)?, child(&third)?))
        };

        match op.as_str() {
            "add" => binary("Add", RustExpr::Add),
            "sub" => binary("Sub", RustExpr::Sub),
            "mul" => binary("Mul", RustExpr::Mul),
            "div" => binary("Div", RustExpr::Div),
            "floordiv" => binary("FloorDiv", RustExpr::FloorDiv),
            "pow" => binary("Pow", RustExpr::Pow),
            "min" => binary("Min", RustExpr::Min),
            "max" => binary("Max", RustExpr::Max),
            "eq" => binary("Eq", RustExpr::Eq),
            "ne" => binary("Ne", RustExpr::Ne),
            "lt" => binary("Lt", RustExpr::Lt),
            "le" => binary("Le", RustExpr::Le),
            "gt" => binary("Gt", RustExpr::Gt),
            "ge" => binary("Ge", RustExpr::Ge),
            "and" => binary("And", RustExpr::And),
            "or" => binary("Or", RustExpr::Or),
            "mod" => binary("Mod", RustExpr::Mod),
            "neg" => unary("Neg", RustExpr::Neg),
            "sin" => unary("Sin", RustExpr::Sin),
            "cos" => unary("Cos", RustExpr::Cos),
            "tan" => unary("Tan", RustExpr::Tan),
            "asin" => unary("Asin", RustExpr::Asin),
            "acos" => unary("Acos", RustExpr::Acos),
            "atan" => unary("Atan", RustExpr::Atan),
            "sqrt" => unary("Sqrt", RustExpr::Sqrt),
            "abs" => unary("Abs", RustExpr::Abs),
            "floor" => unary("Floor", RustExpr::Floor),
            "ceil" => unary("Ceil", RustExpr::Ceil),
            "round" => unary("Round", RustExpr::Round),
            "not" => unary("Not", RustExpr::Not),
            "exp" => unary("Exp", RustExpr::Exp),
            "ln" => unary("Ln", RustExpr::Ln),
            "log10" => unary("Log10", RustExpr::Log10),
            "log2" => unary("Log2", RustExpr::Log2),
            "sign" => unary("Sign", RustExpr::Sign),
            "fract" => unary("Fract", RustExpr::Fract),
            "clamp" => ternary("Clamp", RustExpr::Clamp),
            "where" => ternary("Where", RustExpr::Where),
            "lerp" => ternary("Lerp", RustExpr::Lerp),

            "random" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let stream = match args_list.as_slice() {
                    [] => 0,
                    [stream] => parse_random_stream(stream)?,
                    _ => {
                        return Err(PyTypeError::new_err(
                            "Random operation requires 0 or 1 arguments",
                        ));
                    }
                };
                Ok(RustExpr::Random(stream))
            }

            "random_range" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let (min, max, stream) = match args_list.as_slice() {
                    [min, max] => (min, max, 0),
                    [min, max, stream] => (min, max, parse_random_stream(stream)?),
                    _ => {
                        return Err(PyTypeError::new_err(format!(
                            "RandomRange operation requires 2 or 3 arguments (min, max, stream), got {}",
                            args_list.len()
                        )));
                    }
                };
                Ok(RustExpr::RandomRange(child(min)?, child(max)?, stream))
            }

            "field" => {
                let [component_id, _name, offset, field_type] = expect_args("Field", &args)?;
                let component_id = ComponentId::new(component_id.extract()?);
                let offset: usize = offset.extract()?;
                let field_type_str: String = field_type.extract()?;
                let field_type = match field_type_str.as_str() {
                    "F32" => FieldType::F32,
                    "F64" => FieldType::F64,
                    "I32" => FieldType::I32,
                    "I64" => FieldType::I64,
                    "U8" => FieldType::U8,
                    "U32" => FieldType::U32,
                    "U64" => FieldType::U64,
                    "Bool" => FieldType::Bool,
                    _ => {
                        return Err(PyTypeError::new_err(format!(
                            "Unknown field type: {}",
                            field_type_str
                        )));
                    }
                };

                Ok(RustExpr::Field {
                    component_id,
                    offset,
                    field_type,
                })
            }

            "const" => {
                let [value] = expect_args("Const", &args)?;
                match Self::literal(&value)? {
                    Some(literal) => Ok(literal),
                    None => Ok(RustExpr::FloatConst(value.extract()?)),
                }
            }

            _ => Err(PyTypeError::new_err(format!("Unknown operation: {}", op))),
        }
    }

    /// Convert a numeric Python scalar, keeping its NumPy scalar kind: an
    /// integer (anything with `__index__`), held exactly, or a float.
    fn literal(obj: &Bound<'_, PyAny>) -> PyResult<Option<Self>> {
        if !obj.is_instance_of::<PyFloat>() {
            if let Ok(value) = obj.extract::<i128>() {
                return Ok(Some(RustExpr::Int(value)));
            }
            if obj.is_instance_of::<PyInt>() {
                return obj
                    .extract::<f64>()
                    .map(|value| Some(RustExpr::Const(value)));
            }
        }
        Ok(obj.extract::<f64>().ok().map(RustExpr::FloatConst))
    }

    /// Compile this expression AST to bytecode
    pub fn compile(&self, compiler: &mut Compiler) {
        match self {
            RustExpr::Add(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Add);
            }

            RustExpr::Sub(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Sub);
            }

            RustExpr::Mul(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Mul);
            }

            RustExpr::Div(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Div);
            }
            RustExpr::FloorDiv(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::FloorDiv);
            }

            RustExpr::Pow(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Pow);
            }

            RustExpr::Neg(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Neg);
            }

            RustExpr::Sin(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Sin);
            }

            RustExpr::Cos(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Cos);
            }

            RustExpr::Tan(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Tan);
            }

            RustExpr::Asin(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Asin);
            }

            RustExpr::Acos(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Acos);
            }

            RustExpr::Atan(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Atan);
            }

            RustExpr::Sqrt(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Sqrt);
            }

            RustExpr::Abs(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Abs);
            }

            RustExpr::Floor(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Floor);
            }

            RustExpr::Ceil(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Ceil);
            }

            RustExpr::Round(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Round);
            }

            RustExpr::Min(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Min);
            }

            RustExpr::Max(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Max);
            }

            RustExpr::Clamp(value, min, max) => {
                value.compile(compiler);
                min.compile(compiler);
                max.compile(compiler);
                compiler.emit(Op::Clamp);
            }

            RustExpr::Eq(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Eq);
            }

            RustExpr::Ne(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Ne);
            }

            RustExpr::Lt(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Lt);
            }

            RustExpr::Le(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Le);
            }

            RustExpr::Gt(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Gt);
            }

            RustExpr::Ge(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Ge);
            }

            RustExpr::Where(condition, true_value, false_value) => {
                // Compile in order: condition, true_value, false_value
                // Stack will be: [condition: bool, true_value: float, false_value: float]
                condition.compile(compiler);
                true_value.compile(compiler);
                false_value.compile(compiler);
                compiler.emit(Op::Where);
            }

            RustExpr::And(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::And);
            }

            RustExpr::Or(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Or);
            }

            RustExpr::Not(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Not);
            }

            RustExpr::Exp(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Exp);
            }

            RustExpr::Ln(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Ln);
            }

            RustExpr::Log10(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Log10);
            }

            RustExpr::Log2(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Log2);
            }

            RustExpr::Sign(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Sign);
            }

            RustExpr::Fract(expr) => {
                expr.compile(compiler);
                compiler.emit(Op::Fract);
            }

            RustExpr::Mod(left, right) => {
                left.compile(compiler);
                right.compile(compiler);
                compiler.emit(Op::Mod);
            }

            RustExpr::Lerp(a, b, t) => {
                a.compile(compiler);
                b.compile(compiler);
                t.compile(compiler);
                compiler.emit(Op::Lerp);
            }

            RustExpr::Random(stream) => {
                compiler.emit(Op::Random(*stream));
            }

            RustExpr::RandomRange(min, max, stream) => {
                min.compile(compiler);
                max.compile(compiler);
                compiler.emit(Op::RandomRange(*stream));
            }

            RustExpr::Field {
                component_id,
                offset,
                field_type,
            } => {
                let field_id = FieldId {
                    component_id: *component_id,
                    offset: *offset,
                    field_type: *field_type,
                };
                let field_idx = compiler.add_field(field_id);
                compiler.emit(Op::PushField(field_idx));
            }

            RustExpr::Const(value) => {
                let const_idx = compiler.add_constant(*value);
                compiler.emit(Op::PushConst(const_idx));
            }

            RustExpr::FloatConst(value) => {
                let const_idx = compiler.add_constant(*value);
                compiler.emit(Op::PushFloatConst(const_idx));
            }

            RustExpr::Int(value) => {
                let push = compiler.int_literal(*value);
                compiler.emit(push);
            }
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::ffi::CStr;

    use pyo3::{
        exceptions::PyOverflowError,
        types::{PyDict, PyFloat, PyInt, PyList, PyString},
    };

    use super::*;
    use crate::bytecode::{CompiledBytecode, VM};

    /// Build a Python expression object (`types.SimpleNamespace`) with `op`/`args`.
    fn named_expr<'py>(py: Python<'py>, op: &str, args: &Bound<'py, PyList>) -> Bound<'py, PyAny> {
        let namespace = py
            .import("types")
            .unwrap()
            .getattr("SimpleNamespace")
            .unwrap();
        let kwargs = PyDict::new(py);
        kwargs.set_item("op", op).unwrap();
        kwargs.set_item("args", args).unwrap();
        namespace.call((), Some(&kwargs)).unwrap()
    }

    // Helper: compile an expression and return bytecode
    fn compile_expr(expr: &RustExpr) -> CompiledBytecode {
        let mut compiler = Compiler::new();
        expr.compile(&mut compiler);
        compiler.finalize()
    }

    // Helper: compile and execute with no fields, return top of stack
    fn eval_const_expr(expr: &RustExpr) -> f64 {
        let bytecode = compile_expr(expr);
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        vm.stack[0].as_float()
    }

    fn c(v: f64) -> Box<RustExpr> {
        Box::new(RustExpr::Const(v))
    }

    #[test]
    fn canonical_equality_is_structural_and_float_bit_exact() {
        assert!(RustExpr::Add(c(1.0), c(2.0)).canonical_eq(&RustExpr::Add(c(1.0), c(2.0))));
        assert!(!RustExpr::Add(c(1.0), c(2.0)).canonical_eq(&RustExpr::Sub(c(1.0), c(2.0))));
        assert!(!RustExpr::Add(c(9.0), c(2.0)).canonical_eq(&RustExpr::Add(c(1.0), c(2.0))));
        assert!(!RustExpr::Add(c(1.0), c(9.0)).canonical_eq(&RustExpr::Add(c(1.0), c(2.0))));
        assert!(RustExpr::Neg(c(1.0)).canonical_eq(&RustExpr::Neg(c(1.0))));

        let ternary = RustExpr::Clamp(c(1.0), c(2.0), c(3.0));
        assert!(ternary.canonical_eq(&RustExpr::Clamp(c(1.0), c(2.0), c(3.0))));
        assert!(!ternary.canonical_eq(&RustExpr::Clamp(c(9.0), c(2.0), c(3.0))));
        assert!(!ternary.canonical_eq(&RustExpr::Clamp(c(1.0), c(9.0), c(3.0))));
        assert!(!ternary.canonical_eq(&RustExpr::Clamp(c(1.0), c(2.0), c(9.0))));

        assert!(RustExpr::Random(0).canonical_eq(&RustExpr::Random(0)));
        assert!(!RustExpr::Random(0).canonical_eq(&RustExpr::Random(1)));

        let field = RustExpr::Field {
            component_id: ComponentId::new(1),
            offset: 4,
            field_type: FieldType::F32,
        };
        assert!(field.canonical_eq(&field));
        assert!(!field.canonical_eq(&RustExpr::Field {
            component_id: ComponentId::new(2),
            offset: 4,
            field_type: FieldType::F32,
        }));
        assert!(!field.canonical_eq(&RustExpr::Field {
            component_id: ComponentId::new(1),
            offset: 8,
            field_type: FieldType::F32,
        }));
        assert!(!field.canonical_eq(&RustExpr::Field {
            component_id: ComponentId::new(1),
            offset: 4,
            field_type: FieldType::F64,
        }));

        assert!(!RustExpr::Const(0.0).canonical_eq(&RustExpr::Const(-0.0)));

        let first_nan = f64::from_bits(0x7ff8_0000_0000_0001);
        let second_nan = f64::from_bits(0x7ff8_0000_0000_0002);
        assert!(RustExpr::Const(first_nan).canonical_eq(&RustExpr::Const(first_nan)));
        assert!(!RustExpr::Const(first_nan).canonical_eq(&RustExpr::Const(second_nan)));
    }

    #[test]
    fn canonical_hash_agrees_with_canonical_equality() {
        let hash = |expression: &RustExpr| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            expression.canonical_hash(&mut hasher);
            hasher.finish()
        };
        let field = || RustExpr::Field {
            component_id: ComponentId::new(3),
            offset: 4,
            field_type: FieldType::F32,
        };
        let build = |constant: f64| {
            RustExpr::Clamp(
                Box::new(RustExpr::Add(Box::new(field()), c(constant))),
                Box::new(RustExpr::Int(1 << 70)),
                Box::new(RustExpr::RandomRange(c(0.0), c(1.0), 3)),
            )
        };
        assert!(build(2.0).canonical_eq(&build(2.0)));
        assert_eq!(hash(&build(2.0)), hash(&build(2.0)));
        assert_ne!(hash(&build(0.0)), hash(&build(-0.0)));
        assert_ne!(hash(&RustExpr::Int(2)), hash(&RustExpr::FloatConst(2.0)));
    }

    #[test]
    fn python_literals_keep_their_numpy_scalar_kind_and_exact_value() {
        Python::initialize();
        Python::attach(|py| {
            let convert =
                |source: &CStr| RustExpr::from_py_object(py, &py.eval(source, None, None).unwrap());
            let exact = [
                (c"2", 2),
                (c"True", 1),
                (c"2 ** 63 - 1", i128::from(i64::MAX)),
                (c"2 ** 64 - 1", i128::from(u64::MAX)),
                (c"-(2 ** 63) - 1", -(1_i128 << 63) - 1),
                (c"2 ** 100 + 1", (1_i128 << 100) + 1),
            ];
            for (source, expected) in exact {
                assert!(
                    matches!(convert(source), Ok(RustExpr::Int(value)) if value == expected),
                    "{source:?}"
                );
            }
            assert!(matches!(convert(c"2.0"), Ok(RustExpr::FloatConst(2.0))));
            assert!(matches!(convert(c"2 ** 200"), Ok(RustExpr::Const(v)) if v == 2_f64.powi(200)));
            let error = convert(c"2 ** 1100").unwrap_err();
            assert!(error.is_instance_of::<PyOverflowError>(py));

            let integer_const = named_expr(py, "const", &PyList::new(py, [3i64]).unwrap());
            assert!(matches!(
                RustExpr::from_py_object(py, &integer_const),
                Ok(RustExpr::Int(3))
            ));
            let float_const = named_expr(py, "const", &PyList::new(py, [3.0f64]).unwrap());
            assert!(matches!(
                RustExpr::from_py_object(py, &float_const),
                Ok(RustExpr::FloatConst(3.0))
            ));
        });
        assert!(!RustExpr::Int(2).canonical_eq(&RustExpr::FloatConst(2.0)));
        assert!(!RustExpr::Int(1 << 60).canonical_eq(&RustExpr::Int((1 << 60) + 1)));
        assert!(RustExpr::FloatConst(2.0).canonical_eq(&RustExpr::FloatConst(2.0)));
    }

    #[test]
    fn int_literals_beyond_f64_compile_to_exact_integer_pushes() {
        let bytecode = compile_expr(&RustExpr::Add(
            Box::new(RustExpr::Int((1 << 60) + 1)),
            Box::new(RustExpr::Int(1 << 60)),
        ));
        assert!(matches!(
            bytecode.bytecode[..],
            [Op::PushInt(0), Op::PushConst(0), Op::Add]
        ));
        assert_eq!(bytecode.integers, vec![(1 << 60) + 1]);
        assert_eq!(bytecode.constants, vec![2_f64.powi(60)]);
    }

    #[test]
    fn float_literal_compiles_to_float_constant_push() {
        let bytecode = compile_expr(&RustExpr::Add(c(2.0), Box::new(RustExpr::FloatConst(2.0))));
        assert!(matches!(
            bytecode.bytecode[..],
            [Op::PushConst(0), Op::PushFloatConst(0), Op::Add]
        ));
        assert_eq!(bytecode.constants, vec![2.0]);
    }

    #[test]
    fn test_compile_simple_add() {
        let expr = RustExpr::Add(c(5.0), c(3.0));
        let bytecode = compile_expr(&expr);
        assert_eq!(bytecode.bytecode.len(), 3);
        assert_eq!(bytecode.constants.len(), 2);
    }

    fn nested_adds<'py>(py: Python<'py>, additions: usize) -> Bound<'py, PyAny> {
        let mut expression = named_expr(py, "const", &PyList::new(py, [1.0f64]).unwrap());
        for _ in 0..additions {
            let args = PyList::new(py, [expression, PyFloat::new(py, 1.0).into_any()]).unwrap();
            expression = named_expr(py, "add", &args);
        }
        expression
    }

    #[test]
    fn from_py_object_accepts_the_deepest_allowed_tree() {
        Python::initialize();
        Python::attach(|py| {
            let deepest = nested_adds(py, EXPRESSION_MAX_DEPTH - 1);
            let expr = RustExpr::from_py_object(py, &deepest).expect("the limit must be inclusive");
            assert_eq!(eval_const_expr(&expr), EXPRESSION_MAX_DEPTH as f64);
        });
    }

    #[test]
    fn from_py_object_rejects_a_tree_past_the_depth_limit() {
        Python::initialize();
        Python::attach(|py| {
            let too_deep = nested_adds(py, EXPRESSION_MAX_DEPTH);
            let error = RustExpr::from_py_object(py, &too_deep)
                .expect_err("a tree past the limit must be rejected");
            assert!(error.is_instance_of::<PyValueError>(py));
            assert_eq!(error.value(py).to_string(), expression_too_deep());
        });
    }

    #[test]
    fn from_py_object_rejects_missing_args_without_panicking() {
        Python::initialize();
        Python::attach(|py| {
            for op in [
                "add",
                "sub",
                "mul",
                "div",
                "floordiv",
                "pow",
                "neg",
                "sin",
                "cos",
                "tan",
                "asin",
                "acos",
                "atan",
                "sqrt",
                "abs",
                "floor",
                "ceil",
                "round",
                "min",
                "max",
                "clamp",
                "eq",
                "ne",
                "lt",
                "le",
                "gt",
                "ge",
                "where",
                "and",
                "or",
                "not",
                "exp",
                "ln",
                "log10",
                "log2",
                "sign",
                "fract",
                "mod",
                "lerp",
                "random_range",
                "field",
                "const",
            ] {
                let empty = PyList::empty(py);
                let error = RustExpr::from_py_object(py, &named_expr(py, op, &empty))
                    .expect_err("empty args must be rejected");
                assert!(
                    error.to_string().contains("requires"),
                    "op {op}: unexpected error {error}"
                );
            }

            let empty = PyList::empty(py);
            let error = RustExpr::from_py_object(py, &named_expr(py, "not_an_operation", &empty))
                .expect_err("unknown operations must be rejected");
            assert!(
                error.to_string().contains("Unknown operation"),
                "unexpected error {error}"
            );

            let field_args = PyList::new(
                py,
                [
                    PyInt::new(py, 0).into_any(),
                    PyString::new(py, "value").into_any(),
                    PyInt::new(py, 0).into_any(),
                    PyString::new(py, "NotAType").into_any(),
                ],
            )
            .unwrap();
            let error = RustExpr::from_py_object(py, &named_expr(py, "field", &field_args))
                .expect_err("unknown field types must be rejected");
            assert!(
                error.to_string().contains("Unknown field type"),
                "unexpected error {error}"
            );
        });
    }

    #[test]
    fn test_compile_all_binary_ops() {
        assert_eq!(eval_const_expr(&RustExpr::Add(c(3.0), c(2.0))), 5.0);
        assert_eq!(eval_const_expr(&RustExpr::Sub(c(10.0), c(4.0))), 6.0);
        assert_eq!(eval_const_expr(&RustExpr::Mul(c(3.0), c(7.0))), 21.0);
        assert_eq!(eval_const_expr(&RustExpr::Div(c(15.0), c(3.0))), 5.0);
        assert_eq!(eval_const_expr(&RustExpr::Pow(c(2.0), c(10.0))), 1024.0);
        assert_eq!(eval_const_expr(&RustExpr::Mod(c(10.0), c(3.0))), 1.0);
        assert_eq!(eval_const_expr(&RustExpr::Min(c(3.0), c(7.0))), 3.0);
        assert_eq!(eval_const_expr(&RustExpr::Max(c(3.0), c(7.0))), 7.0);
    }

    #[test]
    fn test_compile_all_unary_ops() {
        assert_eq!(eval_const_expr(&RustExpr::Neg(c(5.0))), -5.0);
        assert_eq!(eval_const_expr(&RustExpr::Abs(c(-7.0))), 7.0);
        assert_eq!(eval_const_expr(&RustExpr::Floor(c(3.7))), 3.0);
        assert_eq!(eval_const_expr(&RustExpr::Ceil(c(3.2))), 4.0);
        assert_eq!(eval_const_expr(&RustExpr::Round(c(3.5))), 4.0);
        assert_eq!(eval_const_expr(&RustExpr::Sqrt(c(25.0))), 5.0);
        assert_eq!(eval_const_expr(&RustExpr::Sign(c(-42.0))), -1.0);
        assert_eq!(eval_const_expr(&RustExpr::Sign(c(0.0))), 0.0);
        assert_eq!(eval_const_expr(&RustExpr::Fract(c(3.75))), 0.75);
        assert!((eval_const_expr(&RustExpr::Exp(c(0.0))) - 1.0).abs() < 1e-10);
        assert!((eval_const_expr(&RustExpr::Ln(c(1.0)))).abs() < 1e-10);
        assert!((eval_const_expr(&RustExpr::Log10(c(100.0))) - 2.0).abs() < 1e-10);
        assert!((eval_const_expr(&RustExpr::Log2(c(8.0))) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_compile_trig_ops() {
        use std::f64::consts::PI;
        assert!((eval_const_expr(&RustExpr::Sin(c(PI / 2.0))) - 1.0).abs() < 1e-10);
        assert!((eval_const_expr(&RustExpr::Cos(c(0.0))) - 1.0).abs() < 1e-10);
        assert!((eval_const_expr(&RustExpr::Tan(c(0.0)))).abs() < 1e-10);
        assert!((eval_const_expr(&RustExpr::Asin(c(1.0))) - PI / 2.0).abs() < 1e-10);
        assert!((eval_const_expr(&RustExpr::Acos(c(1.0)))).abs() < 1e-10);
        assert!((eval_const_expr(&RustExpr::Atan(c(0.0)))).abs() < 1e-10);
    }

    #[test]
    fn test_compile_clamp() {
        // clamp(15.0, 0.0, 10.0) = 10.0
        let expr = RustExpr::Clamp(c(15.0), c(0.0), c(10.0));
        assert_eq!(eval_const_expr(&expr), 10.0);

        // clamp(-5.0, 0.0, 10.0) = 0.0
        let expr = RustExpr::Clamp(c(-5.0), c(0.0), c(10.0));
        assert_eq!(eval_const_expr(&expr), 0.0);

        // clamp(5.0, 0.0, 10.0) = 5.0
        let expr = RustExpr::Clamp(c(5.0), c(0.0), c(10.0));
        assert_eq!(eval_const_expr(&expr), 5.0);
    }

    #[test]
    fn test_compile_lerp() {
        // lerp(0.0, 10.0, 0.5) = 5.0
        let expr = RustExpr::Lerp(c(0.0), c(10.0), c(0.5));
        assert_eq!(eval_const_expr(&expr), 5.0);

        // lerp(0.0, 10.0, 0.0) = 0.0
        let expr = RustExpr::Lerp(c(0.0), c(10.0), c(0.0));
        assert_eq!(eval_const_expr(&expr), 0.0);

        // lerp(0.0, 10.0, 1.0) = 10.0
        let expr = RustExpr::Lerp(c(0.0), c(10.0), c(1.0));
        assert_eq!(eval_const_expr(&expr), 10.0);
    }

    #[test]
    fn test_compile_where() {
        // where(5 > 3, 100, 200) = 100
        let expr = RustExpr::Where(Box::new(RustExpr::Gt(c(5.0), c(3.0))), c(100.0), c(200.0));
        assert_eq!(eval_const_expr(&expr), 100.0);

        // where(1 > 5, 100, 200) = 200
        let expr = RustExpr::Where(Box::new(RustExpr::Gt(c(1.0), c(5.0))), c(100.0), c(200.0));
        assert_eq!(eval_const_expr(&expr), 200.0);
    }

    #[test]
    fn test_compile_comparisons() {
        let bytecode = compile_expr(&RustExpr::Eq(c(5.0), c(5.0)));
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(vm.stack[0].as_bool());

        let bytecode = compile_expr(&RustExpr::Ne(c(5.0), c(3.0)));
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(vm.stack[0].as_bool());

        let bytecode = compile_expr(&RustExpr::Lt(c(3.0), c(5.0)));
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(vm.stack[0].as_bool());

        let bytecode = compile_expr(&RustExpr::Le(c(5.0), c(5.0)));
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(vm.stack[0].as_bool());

        let bytecode = compile_expr(&RustExpr::Gt(c(5.0), c(3.0)));
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(vm.stack[0].as_bool());

        let bytecode = compile_expr(&RustExpr::Ge(c(3.0), c(5.0)));
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(!vm.stack[0].as_bool());
    }

    #[test]
    fn test_compile_logical() {
        // true AND false = false
        let expr = RustExpr::And(
            Box::new(RustExpr::Gt(c(5.0), c(3.0))),
            Box::new(RustExpr::Lt(c(5.0), c(3.0))),
        );
        let bytecode = compile_expr(&expr);
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(!vm.stack[0].as_bool());

        // true OR false = true
        let expr = RustExpr::Or(
            Box::new(RustExpr::Gt(c(5.0), c(3.0))),
            Box::new(RustExpr::Lt(c(5.0), c(3.0))),
        );
        let bytecode = compile_expr(&expr);
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(vm.stack[0].as_bool());

        // NOT true = false
        let expr = RustExpr::Not(Box::new(RustExpr::Gt(c(5.0), c(3.0))));
        let bytecode = compile_expr(&expr);
        let mut vm = VM::new();
        // SAFETY: This program has no field access; no raw storage is dereferenced.
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(!vm.stack[0].as_bool());
    }

    #[test]
    fn test_compile_random() {
        let expr = RustExpr::Random(0);
        let bytecode = compile_expr(&expr);
        assert_eq!(bytecode.bytecode.len(), 1);
        assert!(matches!(bytecode.bytecode[0], Op::Random(0)));
    }

    #[test]
    fn test_compile_random_range() {
        let expr = RustExpr::RandomRange(c(0.0), c(100.0), 0);
        let bytecode = compile_expr(&expr);
        // PushConst(0), PushConst(100), RandomRange
        assert_eq!(bytecode.bytecode.len(), 3);
    }

    #[test]
    fn random_stream_protocol_preserves_and_validates_stream() {
        Python::attach(|py| {
            let args = PyList::new(py, [17]).unwrap();
            let expr = RustExpr::from_py_object(py, &named_expr(py, "random", &args)).unwrap();
            assert!(matches!(expr, RustExpr::Random(17)));

            let bad_args = PyList::new(py, [-1]).unwrap();
            let error = RustExpr::from_py_object(py, &named_expr(py, "random", &bad_args))
                .expect_err("negative streams must be rejected");
            assert!(error.is_instance_of::<PyValueError>(py));
        });
    }

    #[test]
    fn test_compile_field_access() {
        let expr = RustExpr::Field {
            component_id: ComponentId::new(5),
            offset: 16,
            field_type: FieldType::F32,
        };
        let bytecode = compile_expr(&expr);
        assert_eq!(bytecode.bytecode.len(), 1);
        assert!(matches!(bytecode.bytecode[0], Op::PushField(0)));
        assert_eq!(bytecode.field_map[0].offset, 16);
        assert_eq!(bytecode.field_map[0].field_type, FieldType::F32);
    }

    #[test]
    fn test_field_deduplication() {
        // Same field referenced twice should get same index
        let field = RustExpr::Field {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        };
        let expr = RustExpr::Add(Box::new(field.clone()), Box::new(field));
        let bytecode = compile_expr(&expr);
        assert_eq!(bytecode.field_map.len(), 1); // deduplicated
        assert!(matches!(bytecode.bytecode[0], Op::PushField(0)));
        assert!(matches!(bytecode.bytecode[1], Op::PushField(0)));
    }

    #[test]
    fn test_different_fields_get_different_indices() {
        let expr = RustExpr::Add(
            Box::new(RustExpr::Field {
                component_id: ComponentId::new(0),
                offset: 0,
                field_type: FieldType::F32,
            }),
            Box::new(RustExpr::Field {
                component_id: ComponentId::new(0),
                offset: 4,
                field_type: FieldType::F32,
            }),
        );
        let bytecode = compile_expr(&expr);
        assert_eq!(bytecode.field_map.len(), 2);
    }

    #[test]
    fn test_constant_deduplication() {
        // Same value used twice → one constant pool entry
        let expr = RustExpr::Add(c(42.0), c(42.0));
        let bytecode = compile_expr(&expr);
        assert_eq!(bytecode.constants.len(), 1);
        assert_eq!(bytecode.constants[0], 42.0);
    }

    #[test]
    fn test_different_constants() {
        let expr = RustExpr::Add(c(1.0), c(2.0));
        let bytecode = compile_expr(&expr);
        assert_eq!(bytecode.constants.len(), 2);
    }

    #[test]
    fn test_deeply_nested() {
        // ((1 + 2) * 3) - 4 = 5
        let expr = RustExpr::Sub(
            Box::new(RustExpr::Mul(
                Box::new(RustExpr::Add(c(1.0), c(2.0))),
                c(3.0),
            )),
            c(4.0),
        );
        assert_eq!(eval_const_expr(&expr), 5.0);
    }

    #[test]
    fn test_complex_nested_with_unary() {
        // abs(neg(5)) + sqrt(16) = 5 + 4 = 9
        let expr = RustExpr::Add(
            Box::new(RustExpr::Abs(Box::new(RustExpr::Neg(c(5.0))))),
            Box::new(RustExpr::Sqrt(c(16.0))),
        );
        assert_eq!(eval_const_expr(&expr), 9.0);
    }

    #[test]
    fn test_optimizer_div_by_zero_not_folded() {
        let mut compiler = Compiler::new();
        let a = compiler.add_constant(10.0);
        let b = compiler.add_constant(0.0);
        compiler.emit(Op::PushConst(a));
        compiler.emit(Op::PushConst(b));
        compiler.emit(Op::Div);
        compiler.optimize();
        let bytecode = compiler.finalize();
        // Should NOT fold: 10.0 / 0.0 kept as 3 ops
        assert_eq!(bytecode.bytecode.len(), 3);
    }

    #[test]
    fn test_optimizer_chained_constants() {
        // 2.0 + 3.0 gets folded to 5.0
        let mut compiler = Compiler::new();
        let a = compiler.add_constant(2.0);
        let b = compiler.add_constant(3.0);
        compiler.emit(Op::PushConst(a));
        compiler.emit(Op::PushConst(b));
        compiler.emit(Op::Add);
        compiler.optimize();
        let bytecode = compiler.finalize();
        assert_eq!(bytecode.bytecode.len(), 1);
        assert_eq!(bytecode.constants[2], 5.0); // folded constant at index 2
    }

    #[test]
    fn test_optimizer_pow_folding() {
        let mut compiler = Compiler::new();
        let a = compiler.add_constant(2.0);
        let b = compiler.add_constant(8.0);
        compiler.emit(Op::PushConst(a));
        compiler.emit(Op::PushConst(b));
        compiler.emit(Op::Pow);
        compiler.optimize();
        let bytecode = compiler.finalize();
        assert_eq!(bytecode.bytecode.len(), 1);
        assert_eq!(bytecode.constants[2], 256.0);
    }
}
