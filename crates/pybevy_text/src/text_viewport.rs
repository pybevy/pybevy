use bevy::{math::Vec2, text::TextViewport};
use pybevy_core::{FromBorrowedStorage, ValueStorage, repr::ReprLiteral};
use pybevy_macros::pyvalue;
use pybevy_math::{rect::PyRect, vec2::PyVec2};
use pyo3::prelude::*;

#[pyvalue(domains = [offset => [finite], size => [finite]])]
#[pyclass(name = "TextViewport", module = "pybevy.text", eq, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyTextViewport {
    pub(crate) storage: ValueStorage<TextViewport>,
}

impl PartialEq for PyTextViewport {
    fn eq(&self, other: &Self) -> bool {
        match (self.to_bevy(), other.to_bevy()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl From<TextViewport> for PyTextViewport {
    fn from(viewport: TextViewport) -> Self {
        Self::from_owned(viewport)
    }
}

impl TryFrom<PyTextViewport> for TextViewport {
    type Error = PyErr;

    fn try_from(viewport: PyTextViewport) -> PyResult<Self> {
        viewport.to_bevy()
    }
}

impl TryFrom<&PyTextViewport> for TextViewport {
    type Error = PyErr;

    fn try_from(viewport: &PyTextViewport) -> PyResult<Self> {
        viewport.to_bevy()
    }
}

#[pymethods]
impl PyTextViewport {
    #[new]
    #[pyo3(signature = (*, offset = PyVec2::ZERO, size = PyVec2::ZERO))]
    pub fn new(offset: PyVec2, size: PyVec2) -> PyResult<Self> {
        Ok(Self::from_owned(TextViewport {
            offset: Self::check_offset(Vec2::try_from(offset)?)?,
            size: Self::check_size(Vec2::try_from(size)?)?,
        }))
    }

    #[getter]
    pub fn offset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |v| &v.offset,
            |v| &mut v.offset,
            Self::validate_offset,
        )?)
    }

    #[setter]
    pub fn set_offset(&mut self, value: PyVec2) -> PyResult<()> {
        let value = Self::check_offset(Vec2::try_from(value)?)?;
        self.storage.as_mut()?.offset = value;
        Ok(())
    }

    #[getter]
    pub fn size(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |v| &v.size,
            |v| &mut v.size,
            Self::validate_size,
        )?)
    }

    #[setter]
    pub fn set_size(&mut self, value: PyVec2) -> PyResult<()> {
        let value = Self::check_size(Vec2::try_from(value)?)?;
        self.storage.as_mut()?.size = value;
        Ok(())
    }

    pub fn rect(&self) -> PyResult<PyRect> {
        Ok(self.to_bevy()?.rect().into())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let viewport = self.to_bevy()?;
        Ok(format!(
            "TextViewport(offset={}, size={})",
            viewport.offset.repr_literal(),
            viewport.size.repr_literal()
        ))
    }
}
