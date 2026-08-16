//! PyO3 glue over the interpreter-neutral kernels in [`crate::kernels`].
//! Operand extraction, scalar boxing, and `PyErr` mapping live here; every
//! numeric routine is a thin wrapper that calls `crate::kernels` and wraps the
//! `DenseArrayCore`/`Reduced`/`bool` result back into `PyArray`/`Py<PyAny>`.
//! Signatures are unchanged from before the split, so `array.rs`/`funcs.rs`
//! call sites are untouched.

use pybevy_bytecodevm::{bytecode::Op, dense::DenseError};
use pyo3::{
    IntoPyObjectExt,
    exceptions::{PyMemoryError, PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyBool, PyFloat, PyInt},
};

use super::{array::PyArray, dtype::cast_scalar};
pub use crate::kernels::{CmpKind, KernelError, OperandRef, Reduce, Reduced};
use crate::{ArrayDType, ArrayError, DenseArrayCore, Scalar, kernels as core_kernels};

pub fn map_array_err(err: ArrayError) -> PyErr {
    match err {
        ArrayError::IndexOutOfBounds { .. }
        | ArrayError::TooManyIndices { .. }
        | ArrayError::MaskShapeMismatch { .. } => {
            pyo3::exceptions::PyIndexError::new_err(err.to_string())
        }
        ArrayError::NotWritable => PyValueError::new_err(err.to_string()),
        ArrayError::UnsupportedDType { .. } => PyTypeError::new_err(err.to_string()),
        ArrayError::BorrowExpired(_) | ArrayError::AccessConflict => {
            PyRuntimeError::new_err(err.to_string())
        }
        ArrayError::AllocationFailed { .. } => PyMemoryError::new_err(err.to_string()),
        _ => PyValueError::new_err(err.to_string()),
    }
}

pub fn map_dense_err(err: DenseError) -> PyErr {
    PyRuntimeError::new_err(format!("dense execution error: {err}"))
}

/// Map a neutral [`KernelError`] to a `PyErr`, delegating to the array/dense
/// mappers so exception categories stay identical to the pre-split behavior.
pub fn map_kernel_err(err: KernelError) -> PyErr {
    match err {
        KernelError::Array(e) => map_array_err(e),
        KernelError::Dense(e) => map_dense_err(e),
        KernelError::RequiresArrayOperand { op } => {
            PyTypeError::new_err(format!("{op} requires at least one array operand"))
        }
        KernelError::MixedFloatDTypes => {
            PyTypeError::new_err("fused evaluation requires one shared float dtype")
        }
    }
}

/// Keeps a Python array borrow alive for a synchronous neutral-kernel call.
pub enum ExtractedOperand<'py> {
    Array(PyRef<'py, PyArray>),
    Scalar(Scalar),
}

impl ExtractedOperand<'_> {
    pub fn as_kernel(&self) -> OperandRef<'_> {
        match self {
            ExtractedOperand::Array(array) => OperandRef::Array(&array.core),
            ExtractedOperand::Scalar(value) => OperandRef::Scalar(*value),
        }
    }
}

/// Extract a Python object while retaining any bounded-array borrow.
pub fn extract_operand<'py>(obj: &Bound<'py, PyAny>) -> PyResult<ExtractedOperand<'py>> {
    if let Ok(nd) = obj.extract::<PyRef<'_, PyArray>>() {
        return Ok(ExtractedOperand::Array(nd));
    }
    Ok(ExtractedOperand::Scalar(extract_scalar(obj)?))
}

/// Extract a Python number as a neutral scalar (bool before int before float).
pub fn extract_scalar(obj: &Bound<'_, PyAny>) -> PyResult<Scalar> {
    if let Ok(b) = obj.cast::<PyBool>() {
        return Ok(Scalar::Bool(b.is_true()));
    }
    if obj.is_instance_of::<PyInt>() {
        return Ok(Scalar::I64(obj.extract::<i64>()?));
    }
    if obj.is_instance_of::<PyFloat>() {
        return Ok(Scalar::F64(obj.extract::<f64>()?));
    }
    Err(PyTypeError::new_err(
        "expected a number (int, float, or bool)",
    ))
}

/// Convert a neutral scalar to a plain Python scalar (int/float/bool).
pub fn scalar_to_py(py: Python<'_>, scalar: Scalar) -> Py<PyAny> {
    match scalar {
        Scalar::F64(v) => v.into_py_any(py).unwrap(),
        Scalar::I64(v) => v.into_py_any(py).unwrap(),
        Scalar::Bool(v) => v.into_py_any(py).unwrap(),
    }
}

/// Cast a scalar to `dtype`, then return it as a Python scalar (for dtype
/// constants called as casts, e.g. `float32(2.5)`).
pub fn cast_scalar_to_py(py: Python<'_>, scalar: Scalar, dtype: ArrayDType) -> Py<PyAny> {
    scalar_to_py(py, cast_scalar(scalar, dtype))
}

/// Broadcast an operand to `target_shape` as neutral scalars (exact for ints).
pub fn gather_scalars(op: OperandRef<'_>, target_shape: &[usize]) -> PyResult<Vec<Scalar>> {
    core_kernels::gather_scalars_borrowed(op, target_shape).map_err(map_kernel_err)
}

/// Run a float-producing dense program over `operands`, returning a new array.
pub fn float_elementwise(
    op_name: &'static str,
    ops: &[Op],
    constants: &[f64],
    operands: &[OperandRef<'_>],
) -> PyResult<PyArray> {
    let core = core_kernels::float_elementwise_borrowed(op_name, ops, constants, operands)
        .map_err(map_kernel_err)?;
    Ok(PyArray::wrap(core))
}

/// Element-wise comparison producing a bool array.
pub fn compare(a: OperandRef<'_>, b: OperandRef<'_>, kind: CmpKind) -> PyResult<PyArray> {
    let core = core_kernels::compare_borrowed(a, b, kind).map_err(map_kernel_err)?;
    Ok(PyArray::wrap(core))
}

/// `where(condition, a, b)`.
pub fn where_select(
    cond: OperandRef<'_>,
    a: OperandRef<'_>,
    b: OperandRef<'_>,
) -> PyResult<PyArray> {
    let core = core_kernels::where_select_borrowed(cond, a, b).map_err(map_kernel_err)?;
    Ok(PyArray::wrap(core))
}

pub fn isfinite(a: &DenseArrayCore) -> PyResult<PyArray> {
    let core = core_kernels::isfinite(a).map_err(map_kernel_err)?;
    Ok(PyArray::wrap(core))
}

pub fn isclose(a: OperandRef<'_>, b: OperandRef<'_>) -> PyResult<PyArray> {
    let core = core_kernels::isclose_borrowed(a, b).map_err(map_kernel_err)?;
    Ok(PyArray::wrap(core))
}

pub fn allclose(a: OperandRef<'_>, b: OperandRef<'_>) -> PyResult<bool> {
    core_kernels::allclose_borrowed(a, b).map_err(map_kernel_err)
}

pub fn array_equal(a: &DenseArrayCore, b: &DenseArrayCore) -> PyResult<bool> {
    core_kernels::array_equal(a, b).map_err(map_kernel_err)
}

/// Resolve a possibly negative `axis` NumPy-style against `ndim`.
fn normalize_axis(axis: isize, ndim: usize) -> PyResult<usize> {
    let resolved = if axis < 0 {
        axis.checked_add(ndim as isize)
    } else {
        Some(axis)
    };
    resolved
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            PyValueError::new_err(format!(
                "axis {axis} is out of bounds for array of dimension {ndim}"
            ))
        })
}

/// Reduce over the whole array (returning a scalar) or along `axis` (returning
/// a new array).
pub fn reduce(
    py: Python<'_>,
    core: &DenseArrayCore,
    kind: Reduce,
    axis: Option<isize>,
) -> PyResult<Py<PyAny>> {
    let axis = axis
        .map(|axis| normalize_axis(axis, core.ndim()))
        .transpose()?;
    match core_kernels::reduce(core, kind, axis).map_err(map_kernel_err)? {
        Reduced::Scalar(s) => Ok(scalar_to_py(py, s)),
        Reduced::Array(core) => Ok(Py::new(py, PyArray::wrap(core))?.into_any()),
    }
}
