use bevy::math::{URect, UVec2};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use super::uvec2::PyUVec2;

#[pyclass(name = "URect")]
#[derive(Debug, Clone)]
pub struct PyURect {
    pub min: UVec2,
    pub max: UVec2,
}

impl From<PyURect> for URect {
    fn from(py_rect: PyURect) -> Self {
        URect {
            min: py_rect.min,
            max: py_rect.max,
        }
    }
}

impl From<&PyURect> for URect {
    fn from(py_rect: &PyURect) -> Self {
        URect {
            min: py_rect.min,
            max: py_rect.max,
        }
    }
}

impl From<URect> for PyURect {
    fn from(rect: URect) -> Self {
        PyURect {
            min: rect.min,
            max: rect.max,
        }
    }
}

#[pymethods]
impl PyURect {
    #[new]
    #[pyo3(signature = (x0, y0, x1, y1))]
    pub fn new(x0: u32, y0: u32, x1: u32, y1: u32) -> Self {
        URect::new(x0, y0, x1, y1).into()
    }

    #[staticmethod]
    #[pyo3(name = "EMPTY")]
    pub fn empty() -> PyURect {
        URect::EMPTY.into()
    }

    #[staticmethod]
    pub fn from_corners(p0: PyUVec2, p1: PyUVec2) -> PyURect {
        URect::from_corners(p0.into(), p1.into()).into()
    }

    #[staticmethod]
    pub fn from_center_size(origin: PyUVec2, size: PyUVec2) -> PyURect {
        URect::from_center_size(origin.into(), size.into()).into()
    }

    #[staticmethod]
    pub fn from_center_half_size(origin: PyUVec2, half_size: PyUVec2) -> PyURect {
        URect::from_center_half_size(origin.into(), half_size.into()).into()
    }

    #[getter]
    pub fn min(&self) -> PyUVec2 {
        self.min.into()
    }

    #[setter]
    pub fn set_min(&mut self, value: PyUVec2) {
        self.min = value.into();
    }

    #[getter]
    pub fn max(&self) -> PyUVec2 {
        self.max.into()
    }

    #[setter]
    pub fn set_max(&mut self, value: PyUVec2) {
        self.max = value.into();
    }

    pub fn is_empty(&self) -> bool {
        URect::from(self).is_empty()
    }

    pub fn width(&self) -> u32 {
        URect::from(self).width()
    }

    pub fn height(&self) -> u32 {
        URect::from(self).height()
    }

    pub fn size(&self) -> PyUVec2 {
        URect::from(self).size().into()
    }

    pub fn half_size(&self) -> PyUVec2 {
        URect::from(self).half_size().into()
    }

    pub fn center(&self) -> PyUVec2 {
        URect::from(self).center().into()
    }

    pub fn contains(&self, point: PyUVec2) -> bool {
        URect::from(self).contains(point.into())
    }

    pub fn union(&self, other: &PyURect) -> PyURect {
        URect::from(self).union(other.into()).into()
    }

    pub fn union_point(&self, point: PyUVec2) -> PyURect {
        URect::from(self).union_point(point.into()).into()
    }

    pub fn intersect(&self, other: &PyURect) -> PyURect {
        URect::from(self).intersect(other.into()).into()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "URect(min=UVec2({}, {}), max=UVec2({}, {}))",
            self.min.x, self.min.y, self.max.x, self.max.y
        )
    }

    pub fn __richcmp__(&self, other: &PyURect, op: CompareOp) -> PyResult<bool> {
        let a: URect = self.into();
        let b: URect = other.into();
        match op {
            CompareOp::Eq => Ok(a == b),
            CompareOp::Ne => Ok(a != b),
            _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
        }
    }
}
