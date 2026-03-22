use std::ops::Range;

use pybevy_core::{FieldStorage, FromBorrowedStorage};
use pyo3::prelude::*;

#[pyclass(name = "Range")]
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
    fn as_ref(&self) -> PyResult<&Range<f32>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Range<f32>> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyRange {
    #[new]
    pub fn new(start: f32, end: f32) -> Self {
        PyRange::from_range(start, end)
    }

    #[getter]
    pub fn start(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.start)
    }

    #[setter]
    pub fn set_start(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.start = value;
        Ok(())
    }

    #[getter]
    pub fn end(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.end)
    }

    #[setter]
    pub fn set_end(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.end = value;
        Ok(())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        let r = self.as_ref()?;
        Ok(r.start >= r.end)
    }

    pub fn contains(&self, value: f32) -> PyResult<bool> {
        let r = self.as_ref()?;
        Ok(value >= r.start && value < r.end)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let r = self.as_ref()?;
        Ok(format!("Range({}, {})", r.start, r.end))
    }

    pub fn __str__(&self) -> PyResult<String> {
        let r = self.as_ref()?;
        Ok(format!("{}..{}", r.start, r.end))
    }
}

impl From<Range<f32>> for PyRange {
    fn from(range: Range<f32>) -> Self {
        PyRange::from_range(range.start, range.end)
    }
}

impl From<PyRange> for Range<f32> {
    fn from(range: PyRange) -> Self {
        let r = range.storage.get().unwrap();
        r.start..r.end
    }
}

impl From<&PyRange> for Range<f32> {
    fn from(range: &PyRange) -> Self {
        let r = range.storage.get().unwrap();
        r.start..r.end
    }
}

impl From<(f32, f32)> for PyRange {
    fn from(tuple: (f32, f32)) -> Self {
        PyRange::from_range(tuple.0, tuple.1)
    }
}

impl From<PyRange> for (f32, f32) {
    fn from(range: PyRange) -> Self {
        let r = range.storage.get().unwrap();
        (r.start, r.end)
    }
}
