use bevy::math::Rect;
use pybevy_core::{FromBorrowedStorage, ValueStorage, public_error::UNSUPPORTED_COMPARISON};
use pybevy_macros::pyvalue;
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

use crate::{richcmp::comparison_result, vec2::PyVec2};

#[pyvalue]
#[pyclass(name = "Rect", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyRect {
    pub(crate) storage: ValueStorage<Rect>,
}

#[pymethods]
impl PyRect {
    #[new]
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self::from_owned(Rect::new(x0, y0, x1, y1))
    }

    #[staticmethod]
    pub fn from_corners(p0: PyVec2, p1: PyVec2) -> PyResult<Self> {
        Ok(Self::from_owned(Rect::from_corners(
            p0.try_into()?,
            p1.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_center_size(origin: PyVec2, size: PyVec2) -> PyResult<Self> {
        Ok(Self::from_owned(Rect::from_center_size(
            origin.try_into()?,
            size.try_into()?,
        )))
    }

    #[staticmethod]
    pub fn from_center_half_size(origin: PyVec2, half_size: PyVec2) -> PyResult<Self> {
        Ok(Self::from_owned(Rect::from_center_half_size(
            origin.try_into()?,
            half_size.try_into()?,
        )))
    }

    #[getter]
    pub fn min(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|r| &r.min)?)
    }

    #[setter]
    pub fn set_min(&mut self, value: PyVec2) -> PyResult<()> {
        self.storage.as_mut()?.min = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn max(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|r| &r.max)?)
    }

    #[setter]
    pub fn set_max(&mut self, value: PyVec2) -> PyResult<()> {
        self.storage.as_mut()?.max = value.try_into()?;
        Ok(())
    }

    pub fn center(&self) -> PyResult<PyVec2> {
        Ok(self.to_bevy()?.center().into())
    }

    pub fn size(&self) -> PyResult<PyVec2> {
        Ok(self.to_bevy()?.size().into())
    }

    pub fn half_size(&self) -> PyResult<PyVec2> {
        Ok(self.to_bevy()?.half_size().into())
    }

    pub fn width(&self) -> PyResult<f32> {
        Ok(self.to_bevy()?.width())
    }

    pub fn height(&self) -> PyResult<f32> {
        Ok(self.to_bevy()?.height())
    }

    pub fn contains(&self, point: PyVec2) -> PyResult<bool> {
        Ok(self.to_bevy()?.contains(point.try_into()?))
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.to_bevy()?.is_empty())
    }

    pub fn intersect(&self, other: &PyRect) -> PyResult<PyRect> {
        Ok(Self::from_owned(
            self.to_bevy()?.intersect(other.to_bevy()?),
        ))
    }

    pub fn union(&self, other: &PyRect) -> PyResult<PyRect> {
        Ok(Self::from_owned(self.to_bevy()?.union(other.to_bevy()?)))
    }

    pub fn union_point(&self, point: PyVec2) -> PyResult<PyRect> {
        Ok(Self::from_owned(
            self.to_bevy()?.union_point(point.try_into()?),
        ))
    }

    pub fn inflate(&self, expansion: f32) -> PyResult<PyRect> {
        Ok(Self::from_owned(self.to_bevy()?.inflate(expansion)))
    }

    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(PyRect {
            storage: ValueStorage::owned(*self.as_ref()?),
        })
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let rect = self.to_bevy()?;
        Ok(format!(
            "Rect(min=Vec2({}, {}), max=Vec2({}, {}))",
            rect.min.x, rect.min.y, rect.max.x, rect.max.y
        ))
    }

    pub fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_value) = other.extract::<PyRect>() else {
            return Ok(py.NotImplemented());
        };
        let a = self.to_bevy()?;
        let b = other_value.to_bevy()?;
        let result = match op {
            CompareOp::Eq => a == b,
            CompareOp::Ne => a != b,
            _ => return Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        };
        Ok(comparison_result(py, result))
    }
}

impl PartialEq for PyRect {
    fn eq(&self, other: &Self) -> bool {
        match (self.to_bevy(), other.to_bevy()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl From<Rect> for PyRect {
    fn from(rect: Rect) -> Self {
        Self::from_owned(rect)
    }
}

impl TryFrom<PyRect> for Rect {
    type Error = PyErr;

    fn try_from(rect: PyRect) -> PyResult<Self> {
        Ok(rect.storage.get()?)
    }
}

impl TryFrom<&PyRect> for Rect {
    type Error = PyErr;

    fn try_from(rect: &PyRect) -> PyResult<Self> {
        Ok(rect.storage.get()?)
    }
}
