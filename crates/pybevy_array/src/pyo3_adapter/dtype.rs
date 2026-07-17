//! PyO3-facing dtype object and callable scalar cast.

use pyo3::{exceptions::PyTypeError, prelude::*, types::PyString};

use super::kernels;
use crate::{ArrayDType, Scalar};

/// A bounded-array dtype. Exposed both as module constants (`xp.float32`) and
/// as `Array.dtype`. Callable as a scalar cast: `xp.float32(2.5)`.
#[pyclass(name = "dtype", module = "pybevy.array", frozen, from_py_object)]
#[derive(Clone, Copy)]
pub struct PyDType {
    pub inner: ArrayDType,
}

impl PyDType {
    pub fn new(inner: ArrayDType) -> Self {
        PyDType { inner }
    }
}

#[pymethods]
impl PyDType {
    #[new]
    fn py_new(spec: &Bound<'_, PyAny>) -> PyResult<Self> {
        // `dtype(None)` is float64, matching NumPy.
        let inner = parse_dtype(Some(spec))?.unwrap_or(ArrayDType::Float64);
        Ok(PyDType { inner })
    }

    /// `str(dtype)` matches NumPy: `"float32"`, ..., `"bool"`.
    fn __str__(&self) -> &'static str {
        self.inner.name()
    }

    fn __repr__(&self) -> String {
        format!("dtype('{}')", self.inner.name())
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other) = other.extract::<PyDType>() {
            return self.inner == other.inner;
        }
        if let Ok(name) = other.extract::<String>() {
            return ArrayDType::from_name(&name) == Some(self.inner);
        }
        false
    }

    fn __hash__(&self) -> u64 {
        self.inner as u64
    }

    #[getter]
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    /// Cast a Python scalar to this dtype, returning a plain Python scalar.
    fn __call__(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let scalar = kernels::extract_scalar(value)?;
        Ok(kernels::cast_scalar_to_py(py, scalar, self.inner))
    }
}

/// Resolve a `dtype=` argument: a [`PyDType`], a name string, or `None`.
pub fn parse_dtype(obj: Option<&Bound<'_, PyAny>>) -> PyResult<Option<ArrayDType>> {
    let Some(obj) = obj else {
        return Ok(None);
    };
    if obj.is_none() {
        return Ok(None);
    }
    if let Ok(dt) = obj.extract::<PyDType>() {
        return Ok(Some(dt.inner));
    }
    if let Ok(s) = obj.cast::<PyString>() {
        let name = s.to_str()?;
        return ArrayDType::from_name(name)
            .map(Some)
            .ok_or_else(|| PyTypeError::new_err(format!("unknown dtype {name:?}")));
    }
    Err(PyTypeError::new_err(
        "dtype must be a pybevy.array dtype or a dtype name string",
    ))
}

/// Cast a scalar to a dtype and return it as `Scalar` (used by constructors).
pub fn cast_scalar(value: Scalar, dtype: ArrayDType) -> Scalar {
    match dtype {
        ArrayDType::Float32 => Scalar::F64(f64::from(value.to_f64() as f32)),
        ArrayDType::Float64 => Scalar::F64(value.to_f64()),
        ArrayDType::Bool => Scalar::Bool(value.to_bool()),
        _ => Scalar::I64(narrow_int(value.to_i64_trunc(), dtype)),
    }
}

fn narrow_int(v: i64, dtype: ArrayDType) -> i64 {
    match dtype {
        ArrayDType::Int64 => v,
        ArrayDType::Int32 => v as i32 as i64,
        ArrayDType::Uint32 => v as u32 as i64,
        ArrayDType::Uint16 => v as u16 as i64,
        ArrayDType::Uint8 => v as u8 as i64,
        _ => v,
    }
}
