use bevy::math::primitives::Line2d;
use pyo3::prelude::*;

use crate::dir2::PyDir2;

#[pyclass(name = "Line2d", frozen, eq, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub struct PyLine2d {
    pub(crate) inner: Line2d,
}

#[pymethods]
impl PyLine2d {
    #[new]
    pub fn new(direction: PyDir2) -> Self {
        Self {
            inner: Line2d {
                direction: direction.into_dir2(),
            },
        }
    }

    #[getter]
    pub fn direction(&self) -> PyDir2 {
        PyDir2::from_dir2(self.inner.direction)
    }

    fn __repr__(&self) -> String {
        format!("Line2d(direction={})", self.inner.direction)
    }
}

impl From<Line2d> for PyLine2d {
    fn from(line: Line2d) -> Self {
        Self { inner: line }
    }
}

impl From<PyLine2d> for Line2d {
    fn from(line: PyLine2d) -> Self {
        line.inner
    }
}
