use bevy::ui::widget::Text;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(Text, bridge)]
#[pyclass(name = "Text", extends = PyComponent)]
#[derive(Debug)]
pub struct PyText {
    pub(crate) storage: ComponentStorage<Text>,
}

#[pymethods]
impl PyText {
    #[new]
    #[pyo3(signature = (content = String::new()))]
    pub fn new(content: String) -> PyClassInitializer<Self> {
        Self::from_owned(Text::new(content)).into()
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
