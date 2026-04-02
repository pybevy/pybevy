use bevy::sprite::Text2dShadow;
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[component_storage(Text2dShadow, bridge)]
#[pyclass(name = "Text2dShadow", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyText2dShadow {
    pub(crate) storage: ComponentStorage<Text2dShadow>,
}

#[pymethods]
impl PyText2dShadow {
    #[new]
    #[pyo3(signature = (offset = None, color = None))]
    pub fn new(offset: Option<PyVec2>, color: Option<PyColor>) -> (Self, PyComponent) {
        let default = Text2dShadow::default();
        let shadow = Text2dShadow {
            offset: offset.map(Into::into).unwrap_or(default.offset),
            color: color.map(Into::into).unwrap_or(default.color),
        };

        Self::from_owned(shadow)
    }

    #[getter]
    pub fn offset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.offset)?)
    }

    #[setter]
    pub fn set_offset(&mut self, offset: PyVec2) -> PyResult<()> {
        self.as_mut()?.offset = offset.into();
        Ok(())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.color = color.into();
        Ok(())
    }
}
