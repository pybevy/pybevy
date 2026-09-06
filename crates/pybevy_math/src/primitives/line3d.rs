use bevy::math::primitives::Line3d;
use pyo3::prelude::*;

use crate::dir3::PyDir3;

#[pyclass(name = "Line3d", module = "pybevy.math", eq, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub struct PyLine3d {
    pub(crate) inner: Line3d,
}

#[pymethods]
impl PyLine3d {
    #[new]
    pub fn new(direction: PyDir3) -> PyResult<Self> {
        Ok(Self {
            inner: Line3d {
                direction: direction.into_dir3()?,
            },
        })
    }

    #[getter]
    pub fn direction(&self) -> PyDir3 {
        PyDir3::from_dir3(self.inner.direction)
    }

    #[setter]
    pub fn set_direction(&mut self, direction: PyDir3) -> PyResult<()> {
        self.inner.direction = direction.into_dir3()?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!("Line3d(direction={})", self.inner.direction)
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
