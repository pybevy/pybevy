use bevy::shape::Line3d;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::prelude::*;

use crate::dir3::PyDir3;

#[pyclass(name = "Line3d", module = "pybevy.shape", eq, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub struct PyLine3d {
    pub(crate) inner: Line3d,
}

#[pymethods]
impl PyLine3d {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(self.clone())
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    #[pyo3(signature = (*, direction))]
    pub fn new(direction: PyDir3) -> PyResult<Self> {
        Ok(Self {
            inner: Line3d {
                direction: direction.into_dir3()?,
            },
        })
    }

    #[getter]
    pub fn direction(&self) -> PyDir3 {
        PyDir3::from_borrowed(ValueStorage::read_only_snapshot(self.inner.direction))
    }

    #[setter]
    pub fn set_direction(&mut self, direction: PyDir3) -> PyResult<()> {
        self.inner.direction = direction.into_dir3()?;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Line3d(direction={})",
            PyDir3::from(self.inner.direction).__repr__()?
        ))
    }
}

impl From<Line3d> for PyLine3d {
    fn from(line: Line3d) -> Self {
        Self { inner: line }
    }
}

impl From<PyLine3d> for Line3d {
    fn from(line: PyLine3d) -> Self {
        line.inner
    }
}
