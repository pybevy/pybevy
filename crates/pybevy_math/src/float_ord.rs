use bevy::math::FloatOrd;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

#[pyvalue]
#[pyclass(name = "FloatOrd", module = "pybevy.math", frozen, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyFloatOrd {
    pub(crate) storage: ValueStorage<FloatOrd>,
}

impl From<FloatOrd> for PyFloatOrd {
    fn from(value: FloatOrd) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyFloatOrd> for FloatOrd {
    type Error = PyErr;

    fn try_from(value: PyFloatOrd) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyFloatOrd> for FloatOrd {
    type Error = PyErr;

    fn try_from(value: &PyFloatOrd) -> PyResult<Self> {
        value.to_bevy()
    }
}

#[pymethods]
impl PyFloatOrd {
    #[new]
    pub fn new(value: f32) -> Self {
        Self::from_owned(FloatOrd(value))
    }

    #[getter]
    pub fn value(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.0)
    }

    pub fn __float__(&self) -> PyResult<f32> {
        self.value()
    }

    pub fn __neg__(&self) -> PyResult<Self> {
        Ok(Self::from_owned(-*self.as_ref()?))
    }

    pub fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(other) = other.extract::<Self>() {
            let left = self.as_ref()?;
            let right = other.as_ref()?;
            return Ok(match op {
                CompareOp::Eq => *left == *right,
                CompareOp::Ne => *left != *right,
                CompareOp::Lt => *left < *right,
                CompareOp::Le => *left <= *right,
                CompareOp::Gt => *left > *right,
                CompareOp::Ge => *left >= *right,
            });
        }
        Err(PyTypeError::new_err(
            "Can only compare FloatOrd with another FloatOrd",
        ))
    }

    pub fn __hash__(&self) -> PyResult<isize> {
        let value = self.value()?;
        let bits = if value.is_nan() {
            f32::NAN.to_bits()
        } else if value == 0.0 {
            0.0_f32.to_bits()
        } else {
            value.to_bits()
        };
        let hash = bits as isize;
        Ok(if hash == -1 { -2 } else { hash })
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("{:?}", *self.as_ref()?))
    }
}
