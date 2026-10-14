use std::ops::Range;

use pybevy_core::{
    FieldStorage, FromBorrowedStorage, StorageRef, public_error::UNSUPPORTED_COMPARISON,
    repr::float_repr,
};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use crate::richcmp::comparison_result;

#[pyclass(name = "Range", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyRange {
    storage: FieldStorage<Range<f32>>,
}

impl Default for PyRange {
    fn default() -> Self {
        Self {
            storage: FieldStorage::owned(0.0..1.0),
        }
    }
}

impl FromBorrowedStorage<FieldStorage<Range<f32>>> for PyRange {
    fn from_borrowed(storage: FieldStorage<Range<f32>>) -> Self {
        PyRange { storage }
    }
}

impl PyRange {
    #[inline(always)]
    pub fn from_range(start: f32, end: f32) -> Self {
        PyRange {
            storage: FieldStorage::owned(start..end),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Range<f32>>> {
        Ok(self.storage.as_ref()?)
    }
}

#[pymethods]
impl PyRange {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Range::<f32>::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    #[pyo3(signature = (start, end))]
    pub fn new(start: f32, end: f32) -> Self {
        PyRange::from_range(start, end)
    }

    #[getter]
    pub fn start(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.start)
    }

    #[setter]
    pub fn set_start(&mut self, value: f32) -> PyResult<()> {
        self.storage.update(|range| range.start = value)?;
        Ok(())
    }

    #[getter]
    pub fn end(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.end)
    }

    #[setter]
    pub fn set_end(&mut self, value: f32) -> PyResult<()> {
        self.storage.update(|range| range.end = value)?;
        Ok(())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        let r = self.as_ref()?;
        Ok(r.is_empty())
    }

    pub fn contains(&self, value: f32) -> PyResult<bool> {
        let r = self.as_ref()?;
        Ok(value >= r.start && value < r.end)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let r = self.as_ref()?;
        Ok(format!(
            "Range({}, {})",
            float_repr(r.start),
            float_repr(r.end)
        ))
    }

    pub fn __str__(&self) -> PyResult<String> {
        let r = self.as_ref()?;
        Ok(format!("{}..{}", r.start, r.end))
    }

    pub fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_range) = other.cast::<PyRange>() else {
            return Ok(py.NotImplemented());
        };
        let other_range = other_range.borrow();
        let a = self.as_ref()?.clone();
        let b = other_range.as_ref()?.clone();
        let result = match op {
            CompareOp::Eq => a == b,
            CompareOp::Ne => a != b,
            _ => return Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        };
        Ok(comparison_result(py, result))
    }
}

impl From<Range<f32>> for PyRange {
    fn from(range: Range<f32>) -> Self {
        PyRange::from_range(range.start, range.end)
    }
}

impl TryFrom<PyRange> for Range<f32> {
    type Error = PyErr;

    fn try_from(range: PyRange) -> Result<Self, Self::Error> {
        Self::try_from(&range)
    }
}

impl TryFrom<&PyRange> for Range<f32> {
    type Error = PyErr;

    fn try_from(range: &PyRange) -> Result<Self, Self::Error> {
        let r = range.storage.get()?;
        Ok(r.start..r.end)
    }
}

impl From<(f32, f32)> for PyRange {
    fn from(tuple: (f32, f32)) -> Self {
        PyRange::from_range(tuple.0, tuple.1)
    }
}

impl TryFrom<PyRange> for (f32, f32) {
    type Error = PyErr;

    fn try_from(range: PyRange) -> Result<Self, Self::Error> {
        let r = range.storage.get()?;
        Ok((r.start, r.end))
    }
}
