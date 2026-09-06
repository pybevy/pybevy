use bevy::{math::Vec2, sprite::BorderRect};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pyvalue]
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
    #[pyo3(signature = (min_inset = PyVec2::ZERO, max_inset = PyVec2::ZERO))]
    pub fn new(min_inset: PyVec2, max_inset: PyVec2) -> PyResult<Self> {
        Ok(Self::from_owned(BorderRect {
            min_inset: min_inset.try_into()?,
            max_inset: max_inset.try_into()?,
        }))
    }

    #[staticmethod]
    pub fn all(inset: f32) -> Self {
        Self::from_owned(BorderRect::all(inset))
    }

    #[staticmethod]
    pub fn axes(horizontal: f32, vertical: f32) -> Self {
        let inset = Vec2::new(horizontal, vertical);
        Self::from_owned(BorderRect {
            min_inset: inset,
            max_inset: inset,
        })
    }

    #[getter]
    pub fn min_inset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|r| &r.min_inset)?)
    }

    #[setter]
    pub fn set_min_inset(&mut self, value: PyVec2) -> PyResult<()> {
        self.storage.as_mut()?.min_inset = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn max_inset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|r| &r.max_inset)?)
    }

    #[setter]
    pub fn set_max_inset(&mut self, value: PyVec2) -> PyResult<()> {
        self.storage.as_mut()?.max_inset = value.try_into()?;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let rect = self.to_bevy()?;
        Ok(format!(
            "BorderRect(min_inset=Vec2({}, {}), max_inset=Vec2({}, {}))",
            rect.min_inset.x, rect.min_inset.y, rect.max_inset.x, rect.max_inset.y
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
