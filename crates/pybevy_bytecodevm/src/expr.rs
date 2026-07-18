//! Expression AST and compiler for lazy batch operations.
//!
//! This module provides an Abstract Syntax Tree (AST) for mathematical expressions
//! that can be compiled to bytecode for efficient execution.

use std::fmt;

use bevy_ecs::component::ComponentId;
use pyo3::{exceptions::PyTypeError, prelude::*};

use super::bytecode::{CompiledBytecode, Compiler, FieldId, FieldType, Op};

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

    /// Random float in [0.0, 1.0): random()
    Random,

    /// Random float in [min, max): random_range(min, max)
    RandomRange(Box<RustExpr>, Box<RustExpr>),

    /// Component field access (e.g., pos.x, vel.y)
    Field {
        component_id: ComponentId,
        offset: usize,
        field_type: FieldType,
    },

    /// Interpreter-neutral dense input column.
    Input { index: u16, field_type: FieldType },

    /// Constant value
    Const(f64),
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
            | (Mod(a1, a2), Mod(b1, b2))
            | (RandomRange(a1, a2), RandomRange(b1, b2)) => {
                a1.canonical_eq(b1) && a2.canonical_eq(b2)
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
            (Random, Random) => true,
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
            (
                Input {
                    index: ai,
                    field_type: at,
                },
                Input {
                    index: bi,
                    field_type: bt,
                },
            ) => ai == bi && at == bt,
            (Const(a), Const(b)) => a.to_bits() == b.to_bits(),
            _ => false,
        }
    }

    /// Parse a Python expression object into a Rust AST
    ///
    /// The Python object should have:
    /// - `op`: string describing the operation ("add", "mul", "field", "const", etc.)
    /// - `args`: list of arguments (child expressions or values)
    pub fn from_py_object(py: Python, obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut resolver = |_: &Bound<'_, PyAny>| Ok(None);
        Self::from_py_object_with(py, obj, &mut resolver)
    }

    /// Parse a Python expression while allowing an adapter to resolve its own
    /// leaf objects into neutral fields before the generic expression protocol
    /// is inspected.
    #[allow(clippy::only_used_in_recursion)]
    pub fn from_py_object_with<F>(
        py: Python,
        obj: &Bound<'_, PyAny>,
        resolver: &mut F,
    ) -> PyResult<Self>
    where
        F: FnMut(&Bound<'_, PyAny>) -> PyResult<Option<RustExpr>>,
    {
        if let Some(resolved) = resolver(obj)? {
            return Ok(resolved);
        }

        // Check if it's a simple numeric type (int or float)
        if let Ok(val) = obj.extract::<f64>() {
            return Ok(RustExpr::Const(val));
        }

        if let Ok(val) = obj.extract::<i64>() {
            return Ok(RustExpr::Const(val as f64));
        }

        // Otherwise, expect an Expr object with 'op' and 'args'
        let op: String = obj
            .getattr("op")
            .map_err(|_| PyTypeError::new_err("Expression object must have 'op' attribute (str)"))?
            .extract()?;

        let args = obj.getattr("args").map_err(|_| {
            PyTypeError::new_err("Expression object must have 'args' attribute (list)")
        })?;

        match op.as_str() {
            "add" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 2 {
                    return Err(PyTypeError::new_err(format!(
                        "Add operation requires 2 arguments, got {}",
                        args_list.len()
                    )));
                }

                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Add(Box::new(left), Box::new(right)))
            }

            "sub" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 2 {
                    return Err(PyTypeError::new_err(format!(
                        "Sub operation requires 2 arguments, got {}",
                        args_list.len()
                    )));
                }

                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Sub(Box::new(left), Box::new(right)))
            }

            "mul" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 2 {
                    return Err(PyTypeError::new_err(format!(
                        "Mul operation requires 2 arguments, got {}",
                        args_list.len()
                    )));
                }

                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Mul(Box::new(left), Box::new(right)))
            }

            "div" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 2 {
                    return Err(PyTypeError::new_err(format!(
                        "Div operation requires 2 arguments, got {}",
                        args_list.len()
                    )));
                }

                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Div(Box::new(left), Box::new(right)))
            }

            "pow" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 2 {
                    return Err(PyTypeError::new_err(format!(
                        "Pow operation requires 2 arguments, got {}",
                        args_list.len()
                    )));
                }

                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Pow(Box::new(left), Box::new(right)))
            }

            "neg" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Neg operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }

                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Neg(Box::new(expr)))
            }

            "sin" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Sin operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Sin(Box::new(expr)))
            }

            "cos" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Cos operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Cos(Box::new(expr)))
            }

            "tan" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Tan operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Tan(Box::new(expr)))
            }

            "asin" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Asin operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Asin(Box::new(expr)))
            }

            "acos" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Acos operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Acos(Box::new(expr)))
            }

            "atan" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Atan operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Atan(Box::new(expr)))
            }

            "sqrt" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Sqrt operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Sqrt(Box::new(expr)))
            }

            "abs" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Abs operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Abs(Box::new(expr)))
            }

            "floor" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Floor operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Floor(Box::new(expr)))
            }

            "ceil" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Ceil operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Ceil(Box::new(expr)))
            }

            "round" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Round operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Round(Box::new(expr)))
            }

            "min" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 2 {
                    return Err(PyTypeError::new_err(format!(
                        "Min operation requires 2 arguments, got {}",
                        args_list.len()
                    )));
                }
                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Min(Box::new(left), Box::new(right)))
            }

            "max" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 2 {
                    return Err(PyTypeError::new_err(format!(
                        "Max operation requires 2 arguments, got {}",
                        args_list.len()
                    )));
                }
                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Max(Box::new(left), Box::new(right)))
            }

            "clamp" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 3 {
                    return Err(PyTypeError::new_err(format!(
                        "Clamp operation requires 3 arguments (value, min, max), got {}",
                        args_list.len()
                    )));
                }
                let value = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let min = Self::from_py_object_with(py, &args_list[1], resolver)?;
                let max = Self::from_py_object_with(py, &args_list[2], resolver)?;
                Ok(RustExpr::Clamp(
                    Box::new(value),
                    Box::new(min),
                    Box::new(max),
                ))
            }

            "eq" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Eq(Box::new(left), Box::new(right)))
            }

            "ne" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Ne(Box::new(left), Box::new(right)))
            }

            "lt" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Lt(Box::new(left), Box::new(right)))
            }

            "le" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Le(Box::new(left), Box::new(right)))
            }

            "gt" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Gt(Box::new(left), Box::new(right)))
            }

            "ge" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Ge(Box::new(left), Box::new(right)))
            }

            "where" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 3 {
                    return Err(PyTypeError::new_err(format!(
                        "Where operation requires 3 arguments (condition, true_value, false_value), got {}",
                        args_list.len()
                    )));
                }
                let condition = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let true_value = Self::from_py_object_with(py, &args_list[1], resolver)?;
                let false_value = Self::from_py_object_with(py, &args_list[2], resolver)?;
                Ok(RustExpr::Where(
                    Box::new(condition),
                    Box::new(true_value),
                    Box::new(false_value),
                ))
            }

            "and" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::And(Box::new(left), Box::new(right)))
            }

            "or" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Or(Box::new(left), Box::new(right)))
            }

            "not" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Not(Box::new(expr)))
            }

            "exp" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Exp(Box::new(expr)))
            }

            "ln" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Ln(Box::new(expr)))
            }

            "log10" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Log10(Box::new(expr)))
            }

            "log2" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Log2(Box::new(expr)))
            }

            "sign" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Sign(Box::new(expr)))
            }

            "fract" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object_with(py, &args_list[0], resolver)?;
                Ok(RustExpr::Fract(Box::new(expr)))
            }

            "mod" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let right = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::Mod(Box::new(left), Box::new(right)))
            }

            "lerp" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 3 {
                    return Err(PyTypeError::new_err(format!(
                        "Lerp operation requires 3 arguments (a, b, t), got {}",
                        args_list.len()
                    )));
                }
                let a = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let b = Self::from_py_object_with(py, &args_list[1], resolver)?;
                let t = Self::from_py_object_with(py, &args_list[2], resolver)?;
                Ok(RustExpr::Lerp(Box::new(a), Box::new(b), Box::new(t)))
            }

            "random" => Ok(RustExpr::Random),

            "random_range" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 2 {
                    return Err(PyTypeError::new_err(format!(
                        "RandomRange operation requires 2 arguments (min, max), got {}",
                        args_list.len()
                    )));
                }

                let min = Self::from_py_object_with(py, &args_list[0], resolver)?;
                let max = Self::from_py_object_with(py, &args_list[1], resolver)?;
                Ok(RustExpr::RandomRange(Box::new(min), Box::new(max)))
            }

            "field" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 4 {
                    return Err(PyTypeError::new_err(format!(
                        "Field operation requires 4 arguments (component_id, field_name, offset, field_type), got {}",
                        args_list.len()
                    )));
                }

                // Extract ComponentId from Python int
                let component_id_value: usize = args_list[0].extract()?;
                let component_id = ComponentId::new(component_id_value);

                let offset: usize = args_list[2].extract()?;

                // Extract field type from string
                let field_type_str: String = args_list[3].extract()?;
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
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                if args_list.len() != 1 {
                    return Err(PyTypeError::new_err(format!(
                        "Const operation requires 1 argument, got {}",
                        args_list.len()
                    )));
                }

                let value: f64 = args_list[0].extract()?;
                Ok(RustExpr::Const(value))
            }

            _ => Err(PyTypeError::new_err(format!("Unknown operation: {}", op))),
        }
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

            RustExpr::Random => {
                compiler.emit(Op::Random);
            }

            RustExpr::RandomRange(min, max) => {
                min.compile(compiler);
                max.compile(compiler);
                compiler.emit(Op::RandomRange);
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

            RustExpr::Input { index, .. } => {
                compiler.emit(Op::PushInput(*index));
            }

            RustExpr::Const(value) => {
                let const_idx = compiler.add_constant(*value);
                compiler.emit(Op::PushConst(const_idx));
            }
        }
    }

    /// Compile a component-independent expression that produces one value per row.
    pub fn compile_map(expr: &RustExpr) -> Result<CompiledMap, MapCompileError> {
        let mut compiler = Compiler::new();
        expr.compile(&mut compiler);
        let compiled = compiler.finalize();
        if !compiled.field_map.is_empty() {
            return Err(MapCompileError::ComponentField);
        }
        let input_count = compiled
            .bytecode
            .iter()
            .filter_map(|operation| match operation {
                Op::PushInput(index) => Some(usize::from(*index) + 1),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        Ok(CompiledMap {
            bytecode: compiled.bytecode,
            constants: compiled.constants,
            input_count,
        })
    }

    /// Compile an assignment expression: dest = expr
    ///
    /// Returns compiled bytecode that evaluates `expr` and stores to `dest_field`
    pub fn compile_assignment(dest_field: FieldId, expr: &RustExpr) -> PyResult<CompiledBytecode> {
        let mut compiler = Compiler::new();

        // Compile the right-hand side expression
        expr.compile(&mut compiler);

        // Add the store operation
        let field_idx = compiler.add_field(dest_field);
        compiler.emit(Op::StoreField(field_idx));

        // Optimize before finalizing
        compiler.optimize();

        let compiled = compiler.finalize();
        if compiled
            .bytecode
            .iter()
            .any(|operation| matches!(operation, Op::PushInput(_)))
        {
            return Err(PyTypeError::new_err(
                "dense input cannot be used in an ECS assignment",
            ));
        }

        Ok(compiled)
    }
}

/// Store-free bytecode over numbered dense input columns.
#[derive(Debug, Clone)]
pub struct CompiledMap {
    pub bytecode: Vec<Op>,
    pub constants: Vec<f64>,
    pub input_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapCompileError {
    ComponentField,
}

impl fmt::Display for MapCompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapCompileError::ComponentField => {
                write!(f, "dense expressions cannot reference ECS component fields")
            }
        }
    }
}

impl std::error::Error for MapCompileError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::VM;

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

        assert!(RustExpr::Random.canonical_eq(&RustExpr::Random));

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

        let input = RustExpr::Input {
            index: 1,
            field_type: FieldType::F32,
        };
        assert!(input.canonical_eq(&input));
        assert!(!input.canonical_eq(&RustExpr::Input {
            index: 2,
            field_type: FieldType::F32,
        }));
        assert!(!input.canonical_eq(&RustExpr::Input {
            index: 1,
            field_type: FieldType::F64,
        }));
        assert!(!RustExpr::Const(0.0).canonical_eq(&RustExpr::Const(-0.0)));

        let first_nan = f64::from_bits(0x7ff8_0000_0000_0001);
        let second_nan = f64::from_bits(0x7ff8_0000_0000_0002);
        assert!(RustExpr::Const(first_nan).canonical_eq(&RustExpr::Const(first_nan)));
        assert!(!RustExpr::Const(first_nan).canonical_eq(&RustExpr::Const(second_nan)));
    }

    #[test]
    fn test_compile_simple_add() {
        let expr = RustExpr::Add(c(5.0), c(3.0));
        let bytecode = compile_expr(&expr);
        assert_eq!(bytecode.bytecode.len(), 3);
        assert_eq!(bytecode.constants.len(), 2);
    }

    #[test]
    fn test_compile_assignment() {
        let component_id = ComponentId::new(0);
        let field_id = FieldId {
            component_id,
            offset: 0,
            field_type: FieldType::F32,
        };

        let expr = RustExpr::Add(
            Box::new(RustExpr::Field {
                component_id,
                offset: 0,
                field_type: FieldType::F32,
            }),
            c(10.0),
        );

        let bytecode = RustExpr::compile_assignment(field_id, &expr).unwrap();
        // PushField(0), PushConst(10.0), Add, StoreField(0)
        assert_eq!(bytecode.bytecode.len(), 4);
    }

    #[test]
    fn compile_map_preserves_expression_order_without_a_store() {
        let left = RustExpr::Input {
            index: 0,
            field_type: FieldType::F32,
        };
        let right = RustExpr::Input {
            index: 1,
            field_type: FieldType::F32,
        };
        let expression = RustExpr::Add(
            Box::new(left),
            Box::new(RustExpr::Mul(Box::new(right), c(2.0))),
        );

        let compiled = RustExpr::compile_map(&expression).unwrap();

        assert_eq!(compiled.bytecode.len(), 5);
        assert!(matches!(compiled.bytecode[0], Op::PushInput(0)));
        assert!(matches!(compiled.bytecode[1], Op::PushInput(1)));
        assert!(matches!(compiled.bytecode[2], Op::PushConst(0)));
        assert!(matches!(compiled.bytecode[3], Op::Mul));
        assert!(matches!(compiled.bytecode[4], Op::Add));
        assert_eq!(compiled.constants, vec![2.0]);
        assert_eq!(compiled.input_count, 2);
        assert!(
            compiled
                .bytecode
                .iter()
                .all(|operation| !matches!(operation, Op::StoreField(_)))
        );
    }

    #[test]
    fn compile_map_rejects_ecs_component_fields() {
        let expression = RustExpr::Field {
            component_id: ComponentId::new(3),
            offset: 0,
            field_type: FieldType::F32,
        };

        assert_eq!(
            RustExpr::compile_map(&expression).unwrap_err(),
            MapCompileError::ComponentField
        );
    }

    #[test]
    fn compile_assignment_rejects_dense_inputs() {
        Python::initialize();
        let destination = FieldId {
            component_id: ComponentId::new(3),
            offset: 0,
            field_type: FieldType::F32,
        };
        let expression = RustExpr::Input {
            index: 0,
            field_type: FieldType::F32,
        };

        let error = RustExpr::compile_assignment(destination, &expression).unwrap_err();
        assert!(error.to_string().contains("dense input"));
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
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(vm.stack[0].as_bool());

        let bytecode = compile_expr(&RustExpr::Ne(c(5.0), c(3.0)));
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(vm.stack[0].as_bool());

        let bytecode = compile_expr(&RustExpr::Lt(c(3.0), c(5.0)));
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(vm.stack[0].as_bool());

        let bytecode = compile_expr(&RustExpr::Le(c(5.0), c(5.0)));
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(vm.stack[0].as_bool());

        let bytecode = compile_expr(&RustExpr::Gt(c(5.0), c(3.0)));
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(vm.stack[0].as_bool());

        let bytecode = compile_expr(&RustExpr::Ge(c(3.0), c(5.0)));
        let mut vm = VM::new();
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
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(!vm.stack[0].as_bool());

        // true OR false = true
        let expr = RustExpr::Or(
            Box::new(RustExpr::Gt(c(5.0), c(3.0))),
            Box::new(RustExpr::Lt(c(5.0), c(3.0))),
        );
        let bytecode = compile_expr(&expr);
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(vm.stack[0].as_bool());

        // NOT true = false
        let expr = RustExpr::Not(Box::new(RustExpr::Gt(c(5.0), c(3.0))));
        let bytecode = compile_expr(&expr);
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert!(!vm.stack[0].as_bool());
    }

    #[test]
    fn test_compile_random() {
        let expr = RustExpr::Random;
        let bytecode = compile_expr(&expr);
        assert_eq!(bytecode.bytecode.len(), 1);
        assert!(matches!(bytecode.bytecode[0], Op::Random));
    }

    #[test]
    fn test_compile_random_range() {
        let expr = RustExpr::RandomRange(c(0.0), c(100.0));
        let bytecode = compile_expr(&expr);
        // PushConst(0), PushConst(100), RandomRange
        assert_eq!(bytecode.bytecode.len(), 3);
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
    fn test_assignment_constant_folding() {
        // field = 5.0 + 3.0 → should optimize to field = 8.0
        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        };
        let expr = RustExpr::Add(c(5.0), c(3.0));
        let bytecode = RustExpr::compile_assignment(field_id, &expr).unwrap();
        // After folding: PushConst(8.0), StoreField(0)
        assert_eq!(bytecode.bytecode.len(), 2);
    }

    #[test]
    fn test_assignment_no_folding_with_field() {
        // field = field + 10.0 → can't fold (PushField breaks pattern)
        let component_id = ComponentId::new(0);
        let field_id = FieldId {
            component_id,
            offset: 0,
            field_type: FieldType::F32,
        };
        let expr = RustExpr::Add(
            Box::new(RustExpr::Field {
                component_id,
                offset: 0,
                field_type: FieldType::F32,
            }),
            c(10.0),
        );
        let bytecode = RustExpr::compile_assignment(field_id, &expr).unwrap();
        // No folding possible: PushField(0), PushConst(10), Add, StoreField(0)
        assert_eq!(bytecode.bytecode.len(), 4);
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
