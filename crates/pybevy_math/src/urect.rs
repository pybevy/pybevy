use bevy::math::{URect, UVec2};
use pybevy_core::{
    FromBorrowedStorage, ValueStorage,
    public_error::{UNSUPPORTED_COMPARISON, unsigned_rect_origin},
};
use pybevy_macros::pyvalue;
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use super::{integer::checked, uvec2::PyUVec2};
use crate::richcmp::comparison_result;

#[pyvalue]
#[pyclass(name = "URect", module = "pybevy.math", from_py_object)]
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
        Ok(URect::from_corners(p0.try_into()?, p1.try_into()?).into())
    }

    #[staticmethod]
    pub fn from_center_size(origin: PyUVec2, size: PyUVec2) -> PyResult<PyURect> {
        let origin: UVec2 = origin.try_into()?;
        let size: UVec2 = size.try_into()?;
        if !origin.cmpge(size / 2).all() {
            return Err(PyValueError::new_err(unsigned_rect_origin(
                origin.to_array(),
                size.to_array(),
                false,
            )));
        }
        Self::from_center_half_size(origin.into(), (size / 2).into())
    }

    #[staticmethod]
    pub fn from_center_half_size(origin: PyUVec2, half_size: PyUVec2) -> PyResult<PyURect> {
        let origin: UVec2 = origin.try_into()?;
        let half_size: UVec2 = half_size.try_into()?;
        if !origin.cmpge(half_size).all() {
            return Err(PyValueError::new_err(unsigned_rect_origin(
                origin.to_array(),
                half_size.to_array(),
                true,
            )));
        }
        Ok(URect {
            min: UVec2::new(
                checked(origin.x.checked_sub(half_size.x))?,
                checked(origin.y.checked_sub(half_size.y))?,
            ),
            max: UVec2::new(
                checked(origin.x.checked_add(half_size.x))?,
                checked(origin.y.checked_add(half_size.y))?,
            ),
        }
        .into())
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

    /// Returns zero when the rectangle is inverted.
    pub fn width(&self) -> PyResult<u32> {
        let rect = self.to_bevy()?;
        Ok(rect.max.x.saturating_sub(rect.min.x))
    }

    pub fn height(&self) -> PyResult<u32> {
        let rect = self.to_bevy()?;
        Ok(rect.max.y.saturating_sub(rect.min.y))
    }

    pub fn inflate(&self, expansion: i32) -> PyResult<PyURect> {
        let rect = self.to_bevy()?;
        let inverse = checked(expansion.checked_neg())?;
        let min = UVec2::new(
            rect.min.x.saturating_add_signed(inverse),
            rect.min.y.saturating_add_signed(inverse),
        );
        let max = UVec2::new(
            rect.max.x.saturating_add_signed(expansion),
            rect.max.y.saturating_add_signed(expansion),
        );
        Ok(URect {
            min: min.min(max),
            max,
        }
        .into())
    }

    pub fn size(&self) -> PyResult<PyUVec2> {
        Ok(UVec2::new(self.width()?, self.height()?).into())
    }

    pub fn half_size(&self) -> PyResult<PyUVec2> {
        Ok((UVec2::new(self.width()?, self.height()?) / 2).into())
    }

    pub fn center(&self) -> PyResult<PyUVec2> {
        let rect = self.to_bevy()?;
        Ok(UVec2::new(
            checked(rect.min.x.checked_add(rect.max.x))? / 2,
            checked(rect.min.y.checked_add(rect.max.y))? / 2,
        )
        .into())
    }

    pub fn contains(&self, point: PyUVec2) -> PyResult<bool> {
        Ok(self.to_bevy()?.contains(point.try_into()?))
    }

    pub fn union(&self, other: &PyURect) -> PyResult<PyURect> {
        Ok(self.to_bevy()?.union(other.to_bevy()?).into())
    }

    pub fn union_point(&self, point: PyUVec2) -> PyResult<PyURect> {
        Ok(self.to_bevy()?.union_point(point.try_into()?).into())
    }

    pub fn intersect(&self, other: &PyURect) -> PyResult<PyURect> {
        Ok(self.to_bevy()?.intersect(other.to_bevy()?).into())
    }

    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(PyURect {
            storage: ValueStorage::owned(*self.as_ref()?),
        })
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let rect = self.to_bevy()?;
        Ok(format!(
            "URect(min=UVec2({}, {}), max=UVec2({}, {}))",
            rect.min.x, rect.min.y, rect.max.x, rect.max.y
        ))
    }

    pub fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_value) = other.extract::<PyURect>() else {
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
