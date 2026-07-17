//! Safe dense array execution.
//!
//! This is the array-side entry point into the VM. It reuses the existing
//! stack machine ([`VM::dispatch_stack_op`]) and every `python_*` semantic
//! helper unchanged, but replaces the ECS-coupled field boundary with plain
//! typed slices: `Op::PushInput(i)` reads dense input column `i`, and the
//! program's final stack value is written to the output for that row. No
//! `FieldId`, `ComponentId`, component stride, raw World pointer, or forged ECS
//! identity appears here.
//!
//! A program is validated once ([`DenseProgram::new`]) so execution never hits
//! the stack machine's type-mismatch panics: input/const indices are in range,
//! stack kinds line up, and exactly one value remains. Output/input aliasing is
//! forbidden by construction, since inputs are shared slices and the output is
//! a mutable slice.
//!
//! Dense numeric execution is float32/float64. Inputs retain their native
//! floating-point stack domain, including f32 rounding after every operation.
//! Integer arrays are stored, indexed, compared, and cast elsewhere.

use std::fmt;

use crate::bytecode::{CompiledBytecode, Op, StackValue, VM};

/// The kind a value has on the stack: numeric or boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackKind {
    Float,
    Bool,
}

impl fmt::Display for StackKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StackKind::Float => write!(f, "float"),
            StackKind::Bool => write!(f, "bool"),
        }
    }
}

/// Why a dense program failed validation or execution.
#[derive(Debug, Clone, PartialEq)]
pub enum DenseError {
    /// The program left no result on the stack.
    EmptyResult,
    /// The program left more than one value on the stack.
    UnbalancedStack { final_depth: usize },
    /// An op popped from an empty stack.
    StackUnderflow { op_index: usize },
    /// An op received the wrong operand kind.
    TypeMismatch {
        op_index: usize,
        expected: StackKind,
        found: StackKind,
    },
    /// `PushInput` referenced an input column that does not exist.
    InputIndexOutOfRange { index: usize, num_inputs: usize },
    /// `PushConst` referenced a constant that does not exist.
    ConstIndexOutOfRange { index: usize, num_constants: usize },
    /// An op is not permitted in a dense program (`StoreField`, `Random`).
    UnsupportedOp { op_index: usize, op: &'static str },
    /// The output slice kind does not match the program's result kind.
    OutputKindMismatch {
        expected: StackKind,
        found: StackKind,
    },
    /// Fewer input columns were supplied than the program references.
    NotEnoughInputs { supplied: usize, required: usize },
    /// An input column is shorter than the iteration domain.
    InputTooShort {
        index: usize,
        len: usize,
        domain: usize,
    },
}

impl fmt::Display for DenseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DenseError::EmptyResult => write!(f, "dense program produced no result"),
            DenseError::UnbalancedStack { final_depth } => {
                write!(
                    f,
                    "dense program left {final_depth} values on the stack, expected 1"
                )
            }
            DenseError::StackUnderflow { op_index } => {
                write!(f, "stack underflow at op {op_index}")
            }
            DenseError::TypeMismatch {
                op_index,
                expected,
                found,
            } => {
                write!(
                    f,
                    "op {op_index} expected {expected} operand, found {found}"
                )
            }
            DenseError::InputIndexOutOfRange { index, num_inputs } => {
                write!(f, "input column {index} out of range ({num_inputs} inputs)")
            }
            DenseError::ConstIndexOutOfRange {
                index,
                num_constants,
            } => {
                write!(
                    f,
                    "constant {index} out of range ({num_constants} constants)"
                )
            }
            DenseError::UnsupportedOp { op_index, op } => {
                write!(f, "op {op_index} ({op}) is not allowed in a dense program")
            }
            DenseError::OutputKindMismatch { expected, found } => {
                write!(f, "output is {found} but program result is {expected}")
            }
            DenseError::NotEnoughInputs { supplied, required } => {
                write!(
                    f,
                    "program references {required} inputs but {supplied} supplied"
                )
            }
            DenseError::InputTooShort { index, len, domain } => {
                write!(f, "input {index} has length {len}, need {domain}")
            }
        }
    }
}

impl std::error::Error for DenseError {}

/// One input column feeding `PushInput`. Slices retain their native float
/// domain; `Scalar` is cast once to an all-f32 program's execution domain.
#[derive(Debug, Clone, Copy)]
pub enum DenseInput<'a> {
    F32(&'a [f32]),
    F64(&'a [f64]),
    Scalar(f64),
}

impl DenseInput<'_> {
    #[inline]
    fn read_stack_value(&self, row: usize, native_f32: bool) -> StackValue {
        match self {
            DenseInput::F32(s) => StackValue::F32(s[row]),
            DenseInput::F64(s) => StackValue::Float(s[row]),
            DenseInput::Scalar(v) if native_f32 => StackValue::F32(*v as f32),
            DenseInput::Scalar(v) => StackValue::Constant(*v),
        }
    }

    fn len(&self) -> Option<usize> {
        match self {
            DenseInput::F32(s) => Some(s.len()),
            DenseInput::F64(s) => Some(s.len()),
            DenseInput::Scalar(_) => None,
        }
    }

    fn is_f32_compatible(&self) -> bool {
        matches!(self, DenseInput::F32(_) | DenseInput::Scalar(_))
    }
}

/// The destination for a dense kernel. Float results narrow to `f32` on write.
#[derive(Debug)]
pub enum DenseOutput<'a> {
    F32(&'a mut [f32]),
    F64(&'a mut [f64]),
    Bool(&'a mut [bool]),
}

impl DenseOutput<'_> {
    fn kind(&self) -> StackKind {
        match self {
            DenseOutput::F32(_) | DenseOutput::F64(_) => StackKind::Float,
            DenseOutput::Bool(_) => StackKind::Bool,
        }
    }

    fn len(&self) -> usize {
        match self {
            DenseOutput::F32(s) => s.len(),
            DenseOutput::F64(s) => s.len(),
            DenseOutput::Bool(s) => s.len(),
        }
    }
}

/// A validated dense program: an expression over `num_inputs` columns that
/// produces one value per row.
#[derive(Debug, Clone)]
pub struct DenseProgram {
    ops: Vec<Op>,
    constants: Vec<f64>,
    num_inputs: usize,
    result_kind: StackKind,
}

impl DenseProgram {
    /// Validate `ops` (referencing `num_inputs` input columns and `constants`)
    /// and record the result kind. `Op::StoreField`, `Op::Random`, and
    /// `Op::RandomRange` are rejected.
    pub fn new(ops: Vec<Op>, constants: Vec<f64>, num_inputs: usize) -> Result<Self, DenseError> {
        let result_kind = validate(&ops, constants.len(), num_inputs)?;
        Ok(DenseProgram {
            ops,
            constants,
            num_inputs,
            result_kind,
        })
    }

    pub fn result_kind(&self) -> StackKind {
        self.result_kind
    }

    pub fn num_inputs(&self) -> usize {
        self.num_inputs
    }
}

/// Pop-order operand kinds and the result kind for an op (excluding
/// `PushInput`/`PushConst`, which are handled inline).
fn stack_effect(op: &Op) -> Result<(&'static [StackKind], StackKind), &'static str> {
    use StackKind::{Bool, Float};
    const F: StackKind = Float;
    const B: StackKind = Bool;
    Ok(match op {
        Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Pow | Op::Min | Op::Max | Op::Mod => {
            (&[F, F], Float)
        }
        Op::Neg
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
        | Op::Exp
        | Op::Ln
        | Op::Log10
        | Op::Log2
        | Op::Sign
        | Op::Fract => (&[F], Float),
        Op::Clamp | Op::Lerp => (&[F, F, F], Float),
        Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => (&[F, F], Bool),
        Op::And | Op::Or => (&[B, B], Bool),
        Op::Not => (&[B], Bool),
        // Kinds are in push (bottom-to-top) order. The Where dispatch pops
        // false_value (top), then true_value, then condition (bottom), so the
        // bottom-to-top order is [condition: bool, true: float, false: float].
        Op::Where => (&[B, F, F], Float),
        Op::StoreField(_) => return Err("StoreField"),
        Op::Random => return Err("Random"),
        Op::RandomRange => return Err("RandomRange"),
        Op::PushInput(_) | Op::PushConst(_) => unreachable!("handled before stack_effect"),
        Op::PushField(_) => return Err("PushField"),
    })
}

fn validate(ops: &[Op], num_constants: usize, num_inputs: usize) -> Result<StackKind, DenseError> {
    let mut kinds: Vec<StackKind> = Vec::new();
    for (op_index, op) in ops.iter().enumerate() {
        match op {
            Op::PushInput(i) => {
                let idx = *i as usize;
                if idx >= num_inputs {
                    return Err(DenseError::InputIndexOutOfRange {
                        index: idx,
                        num_inputs,
                    });
                }
                kinds.push(StackKind::Float);
            }
            Op::PushConst(i) => {
                let idx = *i as usize;
                if idx >= num_constants {
                    return Err(DenseError::ConstIndexOutOfRange {
                        index: idx,
                        num_constants,
                    });
                }
                kinds.push(StackKind::Float);
            }
            _ => {
                let (pops, push) =
                    stack_effect(op).map_err(|op| DenseError::UnsupportedOp { op_index, op })?;
                // Operands were pushed left-to-right; they pop in reverse.
                for expected in pops.iter().rev() {
                    let found = kinds.pop().ok_or(DenseError::StackUnderflow { op_index })?;
                    if found != *expected {
                        return Err(DenseError::TypeMismatch {
                            op_index,
                            expected: *expected,
                            found,
                        });
                    }
                }
                kinds.push(push);
            }
        }
    }
    match kinds.len() {
        0 => Err(DenseError::EmptyResult),
        1 => Ok(kinds[0]),
        n => Err(DenseError::UnbalancedStack { final_depth: n }),
    }
}

#[cfg(feature = "parallel")]
const PARALLEL_THRESHOLD: usize = 8192;

/// Execute a validated dense program element-wise into `output`.
///
/// `inputs[i]` feeds `Op::PushInput(i)`. The iteration domain is the output
/// length; every non-scalar input must be at least that long. Results are
/// identical on the serial and (feature-gated) parallel paths.
pub fn execute_dense(
    program: &DenseProgram,
    inputs: &[DenseInput<'_>],
    output: DenseOutput<'_>,
) -> Result<(), DenseError> {
    if output.kind() != program.result_kind {
        return Err(DenseError::OutputKindMismatch {
            expected: program.result_kind,
            found: output.kind(),
        });
    }
    if inputs.len() < program.num_inputs {
        return Err(DenseError::NotEnoughInputs {
            supplied: inputs.len(),
            required: program.num_inputs,
        });
    }
    let program_inputs = &inputs[..program.num_inputs];
    let domain = output.len();
    for (index, input) in program_inputs.iter().enumerate() {
        if let Some(len) = input.len()
            && len < domain
        {
            return Err(DenseError::InputTooShort { index, len, domain });
        }
    }

    // dispatch_stack_op only reads `constants` for PushConst; the other fields
    // are inert here.
    let compiled = CompiledBytecode {
        bytecode: Vec::new(),
        constants: program.constants.clone(),
        field_map: Vec::new(),
    };
    let has_f32_input = program_inputs
        .iter()
        .any(|input| matches!(input, DenseInput::F32(_)));
    let all_f32_compatible = program_inputs.iter().all(DenseInput::is_f32_compatible);

    match output {
        DenseOutput::F64(out) => run(
            &program.ops,
            &compiled,
            program_inputs,
            out,
            |sv, dst| *dst = sv.as_float(),
            has_f32_input && all_f32_compatible,
        ),
        DenseOutput::F32(out) => run(
            &program.ops,
            &compiled,
            program_inputs,
            out,
            |sv, dst| *dst = sv.as_float() as f32,
            all_f32_compatible,
        ),
        DenseOutput::Bool(out) => run(
            &program.ops,
            &compiled,
            program_inputs,
            out,
            |sv, dst| *dst = sv.as_bool(),
            has_f32_input && all_f32_compatible,
        ),
    }
    Ok(())
}

#[inline]
fn eval_row(
    vm: &mut VM,
    ops: &[Op],
    compiled: &CompiledBytecode,
    inputs: &[DenseInput<'_>],
    row: usize,
    native_f32: bool,
) -> StackValue {
    vm.stack.clear();
    vm.set_native_f32(native_f32);
    for op in ops {
        if let Op::PushInput(i) = op {
            vm.stack
                .push(inputs[*i as usize].read_stack_value(row, native_f32));
        } else {
            vm.dispatch_stack_op(op, compiled);
        }
    }
    vm.stack.pop().expect("validated: exactly one result")
}

fn run<T: Send, W>(
    ops: &[Op],
    compiled: &CompiledBytecode,
    inputs: &[DenseInput<'_>],
    out: &mut [T],
    write: W,
    native_f32: bool,
) where
    W: Fn(StackValue, &mut T) + Sync,
{
    #[cfg(feature = "parallel")]
    if out.len() >= PARALLEL_THRESHOLD {
        use rayon::prelude::*;
        let chunk = 4096;
        out.par_chunks_mut(chunk)
            .enumerate()
            .for_each(|(ci, slice)| {
                let mut vm = VM::new();
                let base = ci * chunk;
                for (local, dst) in slice.iter_mut().enumerate() {
                    let sv = eval_row(&mut vm, ops, compiled, inputs, base + local, native_f32);
                    write(sv, dst);
                }
            });
        return;
    }

    let mut vm = VM::new();
    for (row, dst) in out.iter_mut().enumerate() {
        let sv = eval_row(&mut vm, ops, compiled, inputs, row, native_f32);
        write(sv, dst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_f64(program: &DenseProgram, inputs: &[DenseInput<'_>], n: usize) -> Vec<f64> {
        let mut out = vec![0.0f64; n];
        execute_dense(program, inputs, DenseOutput::F64(&mut out)).unwrap();
        out
    }

    #[test]
    fn add_two_columns() {
        let a = [1.0, 2.0, 3.0];
        let b = [10.0, 20.0, 30.0];
        let program =
            DenseProgram::new(vec![Op::PushInput(0), Op::PushInput(1), Op::Add], vec![], 2)
                .unwrap();
        let out = run_f64(&program, &[DenseInput::F64(&a), DenseInput::F64(&b)], 3);
        assert_eq!(out, vec![11.0, 22.0, 33.0]);
    }

    #[test]
    fn sin_of_scaled_column_matches_std() {
        let a: Vec<f64> = (0..8).map(|i| i as f64).collect();
        // sin(a * 0.5)
        let program = DenseProgram::new(
            vec![Op::PushInput(0), Op::PushConst(0), Op::Mul, Op::Sin],
            vec![0.5],
            1,
        )
        .unwrap();
        let out = run_f64(&program, &[DenseInput::F64(&a)], a.len());
        for (i, v) in out.iter().enumerate() {
            assert!((v - (a[i] * 0.5).sin()).abs() < 1e-12);
        }
    }

    #[test]
    fn scalar_broadcast_reads_same_value() {
        let a = [1.0, 2.0, 3.0, 4.0];
        // a * 10.0 via a scalar input column
        let program =
            DenseProgram::new(vec![Op::PushInput(0), Op::PushInput(1), Op::Mul], vec![], 2)
                .unwrap();
        let out = run_f64(
            &program,
            &[DenseInput::F64(&a), DenseInput::Scalar(10.0)],
            4,
        );
        assert_eq!(out, vec![10.0, 20.0, 30.0, 40.0]);
    }

    #[test]
    fn comparison_writes_bool_output() {
        let a = [1.0, 5.0, 3.0];
        let b = [2.0, 2.0, 3.0];
        let program =
            DenseProgram::new(vec![Op::PushInput(0), Op::PushInput(1), Op::Lt], vec![], 2).unwrap();
        assert_eq!(program.result_kind(), StackKind::Bool);
        let mut out = vec![false; 3];
        execute_dense(
            &program,
            &[DenseInput::F64(&a), DenseInput::F64(&b)],
            DenseOutput::Bool(&mut out),
        )
        .unwrap();
        assert_eq!(out, vec![true, false, false]);
    }

    #[test]
    fn where_selects_by_condition() {
        // where(a < b, a, b) = elementwise minimum-ish selection
        let a = [1.0, 9.0, 3.0];
        let b = [5.0, 2.0, 3.0];
        // push order (bottom->top): condition=(a<b), true_value=a, false_value=b
        let program = DenseProgram::new(
            vec![
                Op::PushInput(0),
                Op::PushInput(1),
                Op::Lt,
                Op::PushInput(0),
                Op::PushInput(1),
                Op::Where,
            ],
            vec![],
            2,
        )
        .unwrap();
        let out = run_f64(&program, &[DenseInput::F64(&a), DenseInput::F64(&b)], 3);
        assert_eq!(out, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn python_modulo_uses_divisor_sign() {
        // a % b, mirroring NumPy/Python divisor-sign semantics.
        let a = [-3.0, 3.0, -3.0, 3.0];
        let b = [2.0, -2.0, -2.0, 2.0];
        let program =
            DenseProgram::new(vec![Op::PushInput(0), Op::PushInput(1), Op::Mod], vec![], 2)
                .unwrap();
        let out = run_f64(&program, &[DenseInput::F64(&a), DenseInput::F64(&b)], 4);
        assert_eq!(out, vec![1.0, -1.0, -1.0, 1.0]);
    }

    #[test]
    fn round_is_ties_to_even() {
        let a = [-2.5, -1.5, -0.5, 0.5, 1.5, 2.5];
        let program = DenseProgram::new(vec![Op::PushInput(0), Op::Round], vec![], 1).unwrap();
        let out = run_f64(&program, &[DenseInput::F64(&a)], 6);
        assert_eq!(out, vec![-2.0, -2.0, -0.0, 0.0, 2.0, 2.0]);
    }

    #[test]
    fn min_max_propagate_nan() {
        let a = [f64::NAN, 1.0];
        let b = [2.0, f64::NAN];
        let prog_min =
            DenseProgram::new(vec![Op::PushInput(0), Op::PushInput(1), Op::Min], vec![], 2)
                .unwrap();
        let out = run_f64(&prog_min, &[DenseInput::F64(&a), DenseInput::F64(&b)], 2);
        assert!(out[0].is_nan() && out[1].is_nan());
    }

    #[test]
    fn f32_output_narrows() {
        let a = [1.0f32, 2.0, 3.0];
        let program = DenseProgram::new(
            vec![Op::PushInput(0), Op::PushConst(0), Op::Mul],
            vec![2.0],
            1,
        )
        .unwrap();
        let mut out = vec![0.0f32; 3];
        execute_dense(&program, &[DenseInput::F32(&a)], DenseOutput::F32(&mut out)).unwrap();
        assert_eq!(out, vec![2.0f32, 4.0, 6.0]);
    }

    #[test]
    fn f32_dense_stack_rounds_after_each_operation() {
        let input = [16_777_216.0_f32];
        let program = DenseProgram::new(
            vec![
                Op::PushInput(0),
                Op::PushConst(0),
                Op::Add,
                Op::PushConst(0),
                Op::Add,
            ],
            vec![1.0],
            1,
        )
        .unwrap();
        let mut output = [0.0_f32];
        execute_dense(
            &program,
            &[DenseInput::F32(&input)],
            DenseOutput::F32(&mut output),
        )
        .unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn f64_input_keeps_f64_intermediates_when_output_is_f32() {
        let input = [16_777_216.0_f64];
        let program = DenseProgram::new(
            vec![
                Op::PushInput(0),
                Op::PushConst(0),
                Op::Add,
                Op::PushConst(0),
                Op::Add,
            ],
            vec![1.0],
            1,
        )
        .unwrap();
        let mut output = [0.0_f32];
        execute_dense(
            &program,
            &[DenseInput::F64(&input)],
            DenseOutput::F32(&mut output),
        )
        .unwrap();
        assert_eq!(output, [16_777_218.0_f32]);
    }

    #[test]
    fn f32_input_keeps_f32_intermediates_when_output_is_f64() {
        let input = [16_777_216.0_f32];
        let program = DenseProgram::new(
            vec![
                Op::PushInput(0),
                Op::PushConst(0),
                Op::Add,
                Op::PushConst(0),
                Op::Add,
            ],
            vec![1.0],
            1,
        )
        .unwrap();
        let mut output = [0.0_f64];
        execute_dense(
            &program,
            &[DenseInput::F32(&input)],
            DenseOutput::F64(&mut output),
        )
        .unwrap();
        assert_eq!(output, [16_777_216.0]);
    }

    #[test]
    fn large_domain_matches_reference() {
        // Exercises the parallel path when the feature is enabled; correctness
        // must be identical either way.
        let n = 20_000usize;
        let a: Vec<f64> = (0..n).map(|i| (i as f64) * 0.001).collect();
        let b: Vec<f64> = (0..n).map(|i| (i as f64) * -0.002).collect();
        // a * b + sin(a)
        let program = DenseProgram::new(
            vec![
                Op::PushInput(0),
                Op::PushInput(1),
                Op::Mul,
                Op::PushInput(0),
                Op::Sin,
                Op::Add,
            ],
            vec![],
            2,
        )
        .unwrap();
        let out = run_f64(&program, &[DenseInput::F64(&a), DenseInput::F64(&b)], n);
        for i in 0..n {
            let expected = a[i] * b[i] + a[i].sin();
            assert!((out[i] - expected).abs() < 1e-12, "row {i}");
        }
    }

    #[test]
    fn rejects_stack_underflow() {
        let err = DenseProgram::new(vec![Op::Add], vec![], 0).unwrap_err();
        assert!(matches!(err, DenseError::StackUnderflow { .. }));
    }

    #[test]
    fn rejects_type_mismatch() {
        // And expects two bools, gets two floats.
        let err = DenseProgram::new(vec![Op::PushInput(0), Op::PushInput(1), Op::And], vec![], 2)
            .unwrap_err();
        assert!(matches!(err, DenseError::TypeMismatch { .. }));
    }

    #[test]
    fn rejects_unbalanced_stack() {
        let err =
            DenseProgram::new(vec![Op::PushInput(0), Op::PushInput(1)], vec![], 2).unwrap_err();
        assert!(matches!(
            err,
            DenseError::UnbalancedStack { final_depth: 2 }
        ));
    }

    #[test]
    fn rejects_input_index_out_of_range() {
        let err = DenseProgram::new(vec![Op::PushInput(3)], vec![], 1).unwrap_err();
        assert!(matches!(err, DenseError::InputIndexOutOfRange { .. }));
    }

    #[test]
    fn rejects_store_and_random() {
        assert!(matches!(
            DenseProgram::new(vec![Op::PushInput(0), Op::StoreField(0)], vec![], 1),
            Err(DenseError::UnsupportedOp {
                op: "StoreField",
                ..
            })
        ));
        assert!(matches!(
            DenseProgram::new(vec![Op::Random], vec![], 0),
            Err(DenseError::UnsupportedOp { op: "Random", .. })
        ));
    }

    #[test]
    fn rejects_ecs_component_field_opcode() {
        assert!(matches!(
            DenseProgram::new(vec![Op::PushField(0)], vec![], 1),
            Err(DenseError::UnsupportedOp {
                op: "PushField",
                ..
            })
        ));
    }

    #[test]
    fn rejects_output_kind_mismatch() {
        // Float result into a bool output.
        let program = DenseProgram::new(vec![Op::PushInput(0)], vec![], 1).unwrap();
        let a = [1.0];
        let mut out = vec![false; 1];
        let err = execute_dense(
            &program,
            &[DenseInput::F64(&a)],
            DenseOutput::Bool(&mut out),
        )
        .unwrap_err();
        assert!(matches!(err, DenseError::OutputKindMismatch { .. }));
    }

    #[test]
    fn rejects_short_input() {
        let program = DenseProgram::new(vec![Op::PushInput(0)], vec![], 1).unwrap();
        let a = [1.0, 2.0];
        let mut out = vec![0.0f64; 4];
        let err = execute_dense(&program, &[DenseInput::F64(&a)], DenseOutput::F64(&mut out))
            .unwrap_err();
        assert!(matches!(err, DenseError::InputTooShort { .. }));
    }
}
