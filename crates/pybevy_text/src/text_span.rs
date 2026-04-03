use bevy::text::TextSpan;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(TextSpan, bridge)]
#[pyclass(name = "TextSpan", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyTextSpan {
    pub(crate) storage: ComponentStorage<TextSpan>,
}

#[pymethods]
impl PyTextSpan {
    #[new]
    #[pyo3(signature = (text = String::new()))]
    pub fn new(text: String) -> (Self, PyComponent) {
        (TextSpan::new(text).into(), PyComponent)
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

    fn __repr__(&self) -> PyResult<String> {
        let text = self.as_ref()?.0.clone();
        Ok(format!("TextSpan(\"{}\")", text))
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(self.as_ref()?.0.clone())
    }
}
