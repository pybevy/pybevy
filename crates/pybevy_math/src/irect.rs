use bevy::math::{IRect, IVec2};
use pybevy_core::{
    FromBorrowedStorage, ValueStorage,
    public_error::{INTEGER_RECT_HALF_SIZE_NEGATIVE, INTEGER_RECT_SIZE_NEGATIVE},
};
use pybevy_macros::pyvalue;
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use super::{integer::checked, ivec2::PyIVec2};
use crate::richcmp::comparison_result;

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
        Ok(IRect::from_corners(p0.try_into()?, p1.try_into()?).into())
    }

    #[staticmethod]
    pub fn from_center_size(origin: PyIVec2, size: PyIVec2) -> PyResult<PyIRect> {
        let size: IVec2 = size.try_into()?;
        if size.x < 0 || size.y < 0 {
            return Err(PyValueError::new_err(INTEGER_RECT_SIZE_NEGATIVE));
        }
        Self::from_center_half_size(origin, (size / 2).into())
    }

    #[staticmethod]
    pub fn from_center_half_size(origin: PyIVec2, half_size: PyIVec2) -> PyResult<PyIRect> {
        let origin: IVec2 = origin.try_into()?;
        let half_size: IVec2 = half_size.try_into()?;
        if half_size.x < 0 || half_size.y < 0 {
            return Err(PyValueError::new_err(INTEGER_RECT_HALF_SIZE_NEGATIVE));
        }
        Ok(IRect {
            min: IVec2::new(
                checked(origin.x.checked_sub(half_size.x))?,
                checked(origin.y.checked_sub(half_size.y))?,
            ),
            max: IVec2::new(
                checked(origin.x.checked_add(half_size.x))?,
                checked(origin.y.checked_add(half_size.y))?,
            ),
        }
        .into())
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
        let rect = self.to_bevy()?;
        checked(rect.max.x.checked_sub(rect.min.x))
    }

    pub fn height(&self) -> PyResult<i32> {
        let rect = self.to_bevy()?;
        checked(rect.max.y.checked_sub(rect.min.y))
    }

    pub fn inflate(&self, expansion: i32) -> PyResult<PyIRect> {
        let rect = self.to_bevy()?;
        let min = IVec2::new(
            checked(rect.min.x.checked_sub(expansion))?,
            checked(rect.min.y.checked_sub(expansion))?,
        );
        let max = IVec2::new(
            checked(rect.max.x.checked_add(expansion))?,
            checked(rect.max.y.checked_add(expansion))?,
        );
        Ok(IRect {
            min: min.min(max),
            max,
        }
        .into())
    }

    pub fn size(&self) -> PyResult<PyIVec2> {
        Ok(IVec2::new(self.width()?, self.height()?).into())
    }

    pub fn half_size(&self) -> PyResult<PyIVec2> {
        Ok((IVec2::new(self.width()?, self.height()?) / 2).into())
    }

    pub fn center(&self) -> PyResult<PyIVec2> {
        let rect = self.to_bevy()?;
        Ok(IVec2::new(
            checked(rect.min.x.checked_add(rect.max.x))? / 2,
            checked(rect.min.y.checked_add(rect.max.y))? / 2,
        )
        .into())
    }

    pub fn contains(&self, point: PyIVec2) -> PyResult<bool> {
        Ok(self.to_bevy()?.contains(point.try_into()?))
    }

    pub fn union(&self, other: &PyIRect) -> PyResult<PyIRect> {
        Ok(self.to_bevy()?.union(other.to_bevy()?).into())
    }

    pub fn union_point(&self, point: PyIVec2) -> PyResult<PyIRect> {
        Ok(self.to_bevy()?.union_point(point.try_into()?).into())
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

    pub fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_value) = other.extract::<PyIRect>() else {
            return Ok(py.NotImplemented());
        };
        let a = self.to_bevy()?;
        let b = other_value.to_bevy()?;
        let result = match op {
            CompareOp::Eq => a == b,
            CompareOp::Ne => a != b,
            _ => return Err(PyTypeError::new_err("Unsupported comparison operation")),
        };
        Ok(comparison_result(py, result))
    }
}
