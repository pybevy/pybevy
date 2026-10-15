use bevy::{color::Color, prelude::TextShadow};
use pybevy_color::color::PyColor;
use pybevy_core::{
    ComponentStorage, PyComponent,
    repr::{ReprLiteral, object_repr},
};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(TextShadow, bridge)]
#[pyclass(name = "TextShadow", module = "pybevy.ui", extends = PyComponent, eq)]
#[derive(Debug, PartialEq)]
pub struct PyTextShadow {
    pub(crate) storage: ComponentStorage<TextShadow>,
}

#[pymethods]
impl PyTextShadow {
    #[new]
    #[pyo3(signature = (
        *,
        offset = PyVec2::from(TextShadow::default().offset),
        color = PyColor::from(TextShadow::default().color)
    ))]
    pub fn new(offset: PyVec2, color: PyColor) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(TextShadow {
            offset: offset.try_into()?,
            color: color.try_into()?,
        })
        .into())
    }

    #[getter]
    pub fn offset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.offset)?)
    }

    #[setter]
    pub fn set_offset(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.offset = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |shadow| &shadow.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let bevy_color = Color::try_from(color)?;
        self.as_mut()?.color = bevy_color;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let shadow = self.as_ref()?;
        Ok(format!(
            "TextShadow(offset={}, color={})",
            shadow.offset.repr_literal(),
            object_repr(PyColor::from(shadow.color))
        ))
    }
}
