use bevy::math::Rect;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

use crate::vec2::PyVec2;

#[pyvalue]
#[pyclass(name = "Rect", from_py_object)]
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

    pub fn __repr__(&self) -> PyResult<String> {
        let rect = self.to_bevy()?;
        Ok(format!(
            "Rect(min=Vec2({}, {}), max=Vec2({}, {}))",
            rect.min.x, rect.min.y, rect.max.x, rect.max.y
        ))
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
