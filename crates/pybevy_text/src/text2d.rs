use bevy::sprite::Text2d;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(Text2d, bridge)]
#[pyclass(name = "Text2d", extends = PyComponent)]
#[derive(Debug)]
pub struct PyText2d {
    pub(crate) storage: ComponentStorage<Text2d>,
}

#[pymethods]
impl PyText2d {
    #[new]
    pub fn new(text: String) -> (Self, PyComponent) {
        (Text2d(text).into(), PyComponent)
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
}
