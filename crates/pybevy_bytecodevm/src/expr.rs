//! Expression AST and compiler for lazy batch operations.
//!
//! This module provides an Abstract Syntax Tree (AST) for mathematical expressions
//! that can be compiled to bytecode for efficient execution.

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

    /// Constant value
    Const(f64),
}

impl RustExpr {
    /// Parse a Python expression object into a Rust AST
    ///
    /// The Python object should have:
    /// - `op`: string describing the operation ("add", "mul", "field", "const", etc.)
    /// - `args`: list of arguments (child expressions or values)
    pub fn from_py_object(py: Python, obj: &Bound<'_, PyAny>) -> PyResult<Self> {
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

                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
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

                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
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

                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
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

                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
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

                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
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

                let expr = Self::from_py_object(py, &args_list[0])?;
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
                let expr = Self::from_py_object(py, &args_list[0])?;
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
                let expr = Self::from_py_object(py, &args_list[0])?;
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
                let expr = Self::from_py_object(py, &args_list[0])?;
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
                let expr = Self::from_py_object(py, &args_list[0])?;
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
                let expr = Self::from_py_object(py, &args_list[0])?;
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
                let expr = Self::from_py_object(py, &args_list[0])?;
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
                let expr = Self::from_py_object(py, &args_list[0])?;
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
                let expr = Self::from_py_object(py, &args_list[0])?;
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
                let expr = Self::from_py_object(py, &args_list[0])?;
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
                let expr = Self::from_py_object(py, &args_list[0])?;
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
                let expr = Self::from_py_object(py, &args_list[0])?;
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
                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
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
                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
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
                let value = Self::from_py_object(py, &args_list[0])?;
                let min = Self::from_py_object(py, &args_list[1])?;
                let max = Self::from_py_object(py, &args_list[2])?;
                Ok(RustExpr::Clamp(
                    Box::new(value),
                    Box::new(min),
                    Box::new(max),
                ))
            }

            "eq" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
                Ok(RustExpr::Eq(Box::new(left), Box::new(right)))
            }

            "ne" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
                Ok(RustExpr::Ne(Box::new(left), Box::new(right)))
            }

            "lt" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
                Ok(RustExpr::Lt(Box::new(left), Box::new(right)))
            }

            "le" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
                Ok(RustExpr::Le(Box::new(left), Box::new(right)))
            }

            "gt" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
                Ok(RustExpr::Gt(Box::new(left), Box::new(right)))
            }

            "ge" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
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
                let condition = Self::from_py_object(py, &args_list[0])?;
                let true_value = Self::from_py_object(py, &args_list[1])?;
                let false_value = Self::from_py_object(py, &args_list[2])?;
                Ok(RustExpr::Where(
                    Box::new(condition),
                    Box::new(true_value),
                    Box::new(false_value),
                ))
            }

            "and" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
                Ok(RustExpr::And(Box::new(left), Box::new(right)))
            }

            "or" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
                Ok(RustExpr::Or(Box::new(left), Box::new(right)))
            }

            "not" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object(py, &args_list[0])?;
                Ok(RustExpr::Not(Box::new(expr)))
            }

            "exp" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object(py, &args_list[0])?;
                Ok(RustExpr::Exp(Box::new(expr)))
            }

            "ln" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object(py, &args_list[0])?;
                Ok(RustExpr::Ln(Box::new(expr)))
            }

            "log10" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object(py, &args_list[0])?;
                Ok(RustExpr::Log10(Box::new(expr)))
            }

            "log2" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object(py, &args_list[0])?;
                Ok(RustExpr::Log2(Box::new(expr)))
            }

            "sign" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object(py, &args_list[0])?;
                Ok(RustExpr::Sign(Box::new(expr)))
            }

            "fract" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let expr = Self::from_py_object(py, &args_list[0])?;
                Ok(RustExpr::Fract(Box::new(expr)))
            }

            "mod" => {
                let args_list = args.extract::<Vec<Bound<'_, PyAny>>>()?;
                let left = Self::from_py_object(py, &args_list[0])?;
                let right = Self::from_py_object(py, &args_list[1])?;
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
                let a = Self::from_py_object(py, &args_list[0])?;
                let b = Self::from_py_object(py, &args_list[1])?;
                let t = Self::from_py_object(py, &args_list[2])?;
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

                let min = Self::from_py_object(py, &args_list[0])?;
                let max = Self::from_py_object(py, &args_list[1])?;
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

            RustExpr::Const(value) => {
                let const_idx = compiler.add_constant(*value);
                compiler.emit(Op::PushConst(const_idx));
            }
        }
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

        Ok(compiler.finalize())
    }
}
