use bevy::math::IRect;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use super::ivec2::PyIVec2;

#[pyvalue]
#[pyclass(name = "IRect", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyIRect {
    pub(crate) storage: ValueStorage<IRect>,
}

impl TryFrom<PyIRect> for IRect {
    type Error = PyErr;

    fn try_from(py_rect: PyIRect) -> PyResult<Self> {
        Ok(py_rect.storage.get()?)
    }
}

impl TryFrom<&PyIRect> for IRect {
    type Error = PyErr;

    fn try_from(py_rect: &PyIRect) -> PyResult<Self> {
        Ok(py_rect.storage.get()?)
    }
}

impl From<IRect> for PyIRect {
    fn from(rect: IRect) -> Self {
        PyIRect::from_owned(rect)
    }
}

#[pymethods]
impl PyIRect {
    #[new]
    #[pyo3(signature = (x0, y0, x1, y1))]
    pub fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        IRect::new(x0, y0, x1, y1).into()
    }

    #[staticmethod]
    #[pyo3(name = "EMPTY")]
    pub fn empty() -> PyIRect {
        IRect::EMPTY.into()
    }

    #[staticmethod]
    pub fn from_corners(p0: PyIVec2, p1: PyIVec2) -> PyResult<PyIRect> {
        Ok(IRect::from_corners(p0.try_into()?, p1.try_into()?).try_into()?)
    }

    #[staticmethod]
    pub fn from_center_size(origin: PyIVec2, size: PyIVec2) -> PyResult<PyIRect> {
        Ok(IRect::from_center_size(origin.try_into()?, size.try_into()?).try_into()?)
    }

    #[staticmethod]
    pub fn from_center_half_size(origin: PyIVec2, half_size: PyIVec2) -> PyResult<PyIRect> {
        Ok(IRect::from_center_half_size(origin.try_into()?, half_size.try_into()?).try_into()?)
    }

    #[getter]
    pub fn min(&self) -> PyResult<PyIVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|rect| &rect.min, |rect| &mut rect.min)?)
    }

    #[setter]
    pub fn set_min(&mut self, value: PyIVec2) -> PyResult<()> {
        self.storage.as_mut()?.min = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn max(&self) -> PyResult<PyIVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|rect| &rect.max, |rect| &mut rect.max)?)
    }

    #[setter]
    pub fn set_max(&mut self, value: PyIVec2) -> PyResult<()> {
        self.storage.as_mut()?.max = value.try_into()?;
        Ok(())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.to_bevy()?.is_empty())
    }

    pub fn width(&self) -> PyResult<i32> {
        Ok(self.to_bevy()?.width())
    }

    pub fn height(&self) -> PyResult<i32> {
        Ok(self.to_bevy()?.height())
    }

    pub fn inflate(&self, expansion: i32) -> PyResult<PyIRect> {
        Ok(self.to_bevy()?.inflate(expansion).into())
    }

    pub fn size(&self) -> PyResult<PyIVec2> {
        Ok(self.to_bevy()?.size().into())
    }

    pub fn half_size(&self) -> PyResult<PyIVec2> {
        Ok(self.to_bevy()?.half_size().into())
    }

    pub fn center(&self) -> PyResult<PyIVec2> {
        Ok(self.to_bevy()?.center().into())
    }

    pub fn contains(&self, point: PyIVec2) -> PyResult<bool> {
        Ok(self.to_bevy()?.contains(point.try_into()?))
    }

    pub fn union(&self, other: &PyIRect) -> PyResult<PyIRect> {
        Ok(self.to_bevy()?.union(other.to_bevy()?).into())
    }

    pub fn union_point(&self, point: PyIVec2) -> PyResult<PyIRect> {
        Ok(self.to_bevy()?.union_point(point.try_into()?).try_into()?)
    }

    pub fn intersect(&self, other: &PyIRect) -> PyResult<PyIRect> {
        Ok(self.to_bevy()?.intersect(other.to_bevy()?).into())
    }

    pub fn as_rect(&self) -> PyResult<super::rect::PyRect> {
        Ok(self.to_bevy()?.as_rect().into())
    }

    pub fn as_urect(&self) -> PyResult<super::urect::PyURect> {
        Ok(self.to_bevy()?.as_urect().into())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let rect = self.to_bevy()?;
        Ok(format!(
            "IRect(min=IVec2({}, {}), max=IVec2({}, {}))",
            rect.min.x, rect.min.y, rect.max.x, rect.max.y
        ))
    }

    pub fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(other_rect) = other.extract::<PyIRect>() {
            let a = self.to_bevy()?;
            let b = other_rect.to_bevy()?;
            match op {
                CompareOp::Eq => Ok(a == b),
                CompareOp::Ne => Ok(a != b),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            }
        } else {
            Err(PyTypeError::new_err(
                "Can only compare IRect with another IRect",
            ))
        }
    }
}
