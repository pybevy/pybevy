use bevy::ui::{BackgroundGradient, Gradient};
use pybevy_core::{ComponentStorage, PyComponent, repr::object_repr};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::gradient::{PyGradient, PyGradients};

#[pycomponent(BackgroundGradient, bridge)]
#[pyclass(name = "BackgroundGradient", module = "pybevy.ui", extends = PyComponent, eq)]
#[derive(Debug, PartialEq)]
pub struct PyBackgroundGradient {
    pub(crate) storage: ComponentStorage<BackgroundGradient>,
}

#[pymethods]
impl PyBackgroundGradient {
    #[new]
    #[pyo3(signature = (gradients = vec![]))]
    pub fn new(gradients: Vec<PyGradient>) -> PyClassInitializer<Self> {
        Self::from_owned(BackgroundGradient(
            gradients.into_iter().map(Gradient::from).collect(),
        ))
        .into()
    }

    #[getter]
    pub fn gradients(&self) -> PyResult<PyGradients> {
        Ok(self.storage.borrow_field_as(|component| &component.0)?)
    }

    #[setter]
    pub fn set_gradients(&mut self, gradients: Vec<PyGradient>) -> PyResult<()> {
        self.as_mut()?.0 = gradients.into_iter().map(Gradient::from).collect();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let inner = self.as_ref()?;
        Ok(format!(
            "BackgroundGradient(gradients={})",
            object_repr(
                inner
                    .0
                    .iter()
                    .cloned()
                    .map(PyGradient::from)
                    .collect::<Vec<_>>()
            )
        ))
    }
}
