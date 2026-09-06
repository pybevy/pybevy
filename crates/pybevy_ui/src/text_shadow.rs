use bevy::{color::Color, math::Vec2, prelude::TextShadow};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
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
    #[pyo3(signature = (offset = None, color = None))]
    pub fn new(
        offset: Option<PyVec2>,
        color: Option<PyColor>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let bevy_offset: Vec2 = offset
            .map(TryInto::try_into)
            .transpose()?
            .unwrap_or(Vec2::splat(4.0));
        let bevy_color = color
            .map(Color::try_from)
            .transpose()?
            .unwrap_or(Color::linear_rgba(0.0, 0.0, 0.0, 0.75));
        Ok(Self::from_owned(TextShadow {
            offset: bevy_offset,
            color: bevy_color,
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
            "TextShadow(offset={:?}, color={:?})",
            shadow.offset, shadow.color
        ))
    }
}
