use bevy::ui::auto_directional_navigation::AutoDirectionalNavigation;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(AutoDirectionalNavigation, bridge)]
#[pyclass(name = "AutoDirectionalNavigation", module = "pybevy.ui", extends = PyComponent)]
#[derive(Debug)]
pub struct PyAutoDirectionalNavigation {
    pub(crate) storage: ComponentStorage<AutoDirectionalNavigation>,
}

impl PyAutoDirectionalNavigation {
    fn default_respect_tab_order() -> bool {
        AutoDirectionalNavigation::default().respect_tab_order
    }
}

#[pymethods]
impl PyAutoDirectionalNavigation {
    #[new]
    #[pyo3(signature = (respect_tab_order = Self::default_respect_tab_order()))]
    pub fn new(respect_tab_order: bool) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(AutoDirectionalNavigation { respect_tab_order }).into())
    }

    #[getter]
    pub fn respect_tab_order(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.respect_tab_order)
    }

    #[setter]
    pub fn set_respect_tab_order(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.respect_tab_order = value;
        Ok(())
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
