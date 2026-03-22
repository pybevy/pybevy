use bevy::ui::widget::Text;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

#[component_storage(Text)]
#[pyclass(name = "Text", extends = PyComponent)]
#[derive(Clone, Debug)]
pub struct PyText {
    pub(crate) storage: ComponentStorage<Text>,
}

#[pymethods]
impl PyText {
    #[new]
    #[pyo3(signature = (content = String::new()))]
    pub fn new(content: String) -> (Self, PyComponent) {
        Self::from_owned(Text::new(content))
    }

    #[getter]
    pub fn content(&self) -> PyResult<String> {
        Ok(self.as_ref()?.0.clone())
    }

    #[setter]
    pub fn set_content(&mut self, value: String) -> PyResult<()> {
        self.as_mut()?.0 = value;
        Ok(())
    }
}
