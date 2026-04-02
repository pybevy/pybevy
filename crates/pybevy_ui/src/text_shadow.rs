use bevy::{color::Color, math::Vec2, prelude::TextShadow};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[component_storage(TextShadow, bridge)]
#[pyclass(name = "TextShadow", extends = PyComponent, eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyTextShadow {
    pub(crate) storage: ComponentStorage<TextShadow>,
}

#[pymethods]
impl PyTextShadow {
    #[new]
    #[pyo3(signature = (offset = None, color = None))]
    pub fn new(offset: Option<PyVec2>, color: Option<PyColor>) -> (Self, PyComponent) {
        let bevy_offset: Vec2 = offset.map(|o| o.into()).unwrap_or(Vec2::ZERO);
        let bevy_color: Color = color.map(|c| c.into()).unwrap_or(Color::BLACK);
        Self::from_owned(TextShadow {
            offset: bevy_offset,
            color: bevy_color,
        })
    }

    #[getter]
    pub fn offset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.offset)?)
    }

    #[setter]
    pub fn set_offset(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.offset = value.into();
        Ok(())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let bevy_color: Color = color.into();
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
