use bevy::math::URect;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use super::uvec2::PyUVec2;

#[pyvalue]
#[pyclass(name = "URect", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyURect {
    pub(crate) storage: ValueStorage<URect>,
}

impl TryFrom<PyURect> for URect {
    type Error = PyErr;

    fn try_from(py_rect: PyURect) -> PyResult<Self> {
        Ok(py_rect.storage.get()?)
    }
}

impl TryFrom<&PyURect> for URect {
    type Error = PyErr;

    fn try_from(py_rect: &PyURect) -> PyResult<Self> {
        Ok(py_rect.storage.get()?)
    }
}

impl From<URect> for PyURect {
    fn from(rect: URect) -> Self {
        PyURect::from_owned(rect)
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
    pub fn from_corners(p0: PyUVec2, p1: PyUVec2) -> PyResult<PyURect> {
        Ok(URect::from_corners(p0.try_into()?, p1.try_into()?).try_into()?)
    }

    #[staticmethod]
    pub fn from_center_size(origin: PyUVec2, size: PyUVec2) -> PyResult<PyURect> {
        Ok(URect::from_center_size(origin.try_into()?, size.try_into()?).try_into()?)
    }

    #[staticmethod]
    pub fn from_center_half_size(origin: PyUVec2, half_size: PyUVec2) -> PyResult<PyURect> {
        Ok(URect::from_center_half_size(origin.try_into()?, half_size.try_into()?).try_into()?)
    }

    #[getter]
    pub fn min(&self) -> PyResult<PyUVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|rect| &rect.min, |rect| &mut rect.min)?)
    }

    #[setter]
    pub fn set_min(&mut self, value: PyUVec2) -> PyResult<()> {
        self.storage.as_mut()?.min = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn max(&self) -> PyResult<PyUVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|rect| &rect.max, |rect| &mut rect.max)?)
    }

    #[setter]
    pub fn set_max(&mut self, value: PyUVec2) -> PyResult<()> {
        self.storage.as_mut()?.max = value.try_into()?;
        Ok(())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.to_bevy()?.is_empty())
    }

    pub fn width(&self) -> PyResult<u32> {
        Ok(self.to_bevy()?.width())
    }

    pub fn height(&self) -> PyResult<u32> {
        Ok(self.to_bevy()?.height())
    }

    pub fn size(&self) -> PyResult<PyUVec2> {
        Ok(self.to_bevy()?.size().into())
    }

    pub fn half_size(&self) -> PyResult<PyUVec2> {
        Ok(self.to_bevy()?.half_size().into())
    }

    pub fn center(&self) -> PyResult<PyUVec2> {
        Ok(self.to_bevy()?.center().into())
    }

    pub fn contains(&self, point: PyUVec2) -> PyResult<bool> {
        Ok(self.to_bevy()?.contains(point.try_into()?))
    }

    pub fn union(&self, other: &PyURect) -> PyResult<PyURect> {
        Ok(self.to_bevy()?.union(other.to_bevy()?).into())
    }

    pub fn union_point(&self, point: PyUVec2) -> PyResult<PyURect> {
        Ok(self.to_bevy()?.union_point(point.try_into()?).try_into()?)
    }

    pub fn intersect(&self, other: &PyURect) -> PyResult<PyURect> {
        Ok(self.to_bevy()?.intersect(other.to_bevy()?).into())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let rect = self.to_bevy()?;
        Ok(format!(
            "URect(min=UVec2({}, {}), max=UVec2({}, {}))",
            rect.min.x, rect.min.y, rect.max.x, rect.max.y
        ))
    }

    pub fn __richcmp__(&self, other: &PyURect, op: CompareOp) -> PyResult<bool> {
        let a = self.to_bevy()?;
        let b = other.to_bevy()?;
        match op {
            CompareOp::Eq => Ok(a == b),
            CompareOp::Ne => Ok(a != b),
            _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
        }
    }
}
