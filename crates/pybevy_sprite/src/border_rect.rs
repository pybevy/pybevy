use bevy::{math::Vec2, sprite::BorderRect};
use pybevy_core::{FromBorrowedStorage, ValueStorage, repr::ReprLiteral};
use pybevy_macros::pyvalue;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pyvalue(domains = [min_inset => [finite], max_inset => [finite]])]
#[pyclass(name = "BorderRect", module = "pybevy.sprite", eq, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyBorderRect {
    pub(crate) storage: ValueStorage<BorderRect>,
}

impl PartialEq for PyBorderRect {
    fn eq(&self, other: &Self) -> bool {
        match (self.to_bevy(), other.to_bevy()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

#[pymethods]
impl PyBorderRect {
    #[new]
    #[pyo3(signature = (*, min_inset = PyVec2::ZERO, max_inset = PyVec2::ZERO))]
    pub fn new(min_inset: PyVec2, max_inset: PyVec2) -> PyResult<Self> {
        Ok(Self::from_owned(BorderRect {
            min_inset: Self::check_min_inset(Vec2::try_from(min_inset)?)?,
            max_inset: Self::check_max_inset(Vec2::try_from(max_inset)?)?,
        }))
    }

    #[staticmethod]
    pub fn all(inset: f32) -> PyResult<Self> {
        let rect = BorderRect::all(inset);
        Self::validate_domains(&rect)?;
        Ok(Self::from_owned(rect))
    }

    #[staticmethod]
    pub fn axes(horizontal: f32, vertical: f32) -> PyResult<Self> {
        let inset = Vec2::new(horizontal, vertical);
        let rect = BorderRect {
            min_inset: inset,
            max_inset: inset,
        };
        Self::validate_domains(&rect)?;
        Ok(Self::from_owned(rect))
    }

    #[getter]
    pub fn min_inset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |r| &r.min_inset,
            |r| &mut r.min_inset,
            Self::validate_min_inset,
        )?)
    }

    #[setter]
    pub fn set_min_inset(&mut self, value: PyVec2) -> PyResult<()> {
        let value = Self::check_min_inset(Vec2::try_from(value)?)?;
        self.storage.as_mut()?.min_inset = value;
        Ok(())
    }

    #[getter]
    pub fn max_inset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |r| &r.max_inset,
            |r| &mut r.max_inset,
            Self::validate_max_inset,
        )?)
    }

    #[setter]
    pub fn set_max_inset(&mut self, value: PyVec2) -> PyResult<()> {
        let value = Self::check_max_inset(Vec2::try_from(value)?)?;
        self.storage.as_mut()?.max_inset = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let rect = self.to_bevy()?;
        Ok(format!(
            "BorderRect(min_inset={}, max_inset={})",
            rect.min_inset.repr_literal(),
            rect.max_inset.repr_literal()
        ))
    }
}

impl TryFrom<PyBorderRect> for BorderRect {
    type Error = PyErr;

    fn try_from(rect: PyBorderRect) -> PyResult<Self> {
        Ok(rect.storage.get()?)
    }
}

impl TryFrom<&PyBorderRect> for BorderRect {
    type Error = PyErr;

    fn try_from(rect: &PyBorderRect) -> PyResult<Self> {
        Ok(rect.storage.get()?)
    }
}

impl From<BorderRect> for PyBorderRect {
    fn from(rect: BorderRect) -> Self {
        PyBorderRect::from_owned(rect)
    }
}
