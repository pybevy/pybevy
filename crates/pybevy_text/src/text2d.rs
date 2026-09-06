use bevy::sprite::Text2d;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(Text2d, bridge)]
#[pyclass(name = "Text2d", module = "pybevy.text", extends = PyComponent)]
#[derive(Debug)]
pub struct PyText2d {
    pub(crate) storage: ComponentStorage<Text2d>,
}

#[pymethods]
impl PyText2d {
    #[new]
    pub fn new(text: String) -> PyClassInitializer<Self> {
        (Text2d(text).into(), PyComponent).into()
    }

    #[getter]
    pub fn text(&self) -> PyResult<String> {
        Ok(self.as_ref()?.0.clone())
    }

    #[setter]
    pub fn set_text(&mut self, text: String) -> PyResult<()> {
        self.as_mut()?.0 = text;
        Ok(())
    }

    /// Alias for `text`, matching UI `Text.content`.
    #[getter]
    pub fn content(&self) -> PyResult<String> {
        self.text()
    }

    #[setter]
    pub fn set_content(&mut self, value: String) -> PyResult<()> {
        self.set_text(value)
    }
}
