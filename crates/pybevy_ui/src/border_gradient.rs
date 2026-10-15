use bevy::ui::{BorderGradient, Gradient};
use pybevy_core::{ComponentStorage, PyComponent, repr::object_repr};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::gradient::PyGradient;

#[pycomponent(BorderGradient, bridge)]
#[pyclass(name = "BorderGradient", module = "pybevy.ui", extends = PyComponent, eq)]
#[derive(Debug, PartialEq)]
pub struct PyBorderGradient {
    pub(crate) storage: ComponentStorage<BorderGradient>,
}

#[pymethods]
impl PyBorderGradient {
    #[new]
    #[pyo3(signature = (gradients = vec![]))]
    pub fn new(gradients: Vec<PyGradient>) -> PyClassInitializer<Self> {
        Self::from_owned(BorderGradient(
            gradients.into_iter().map(Gradient::from).collect(),
        ))
        .into()
    }

    #[getter]
    pub fn gradients(&self) -> PyResult<Vec<PyGradient>> {
        Ok(self.as_ref()?.0.iter().cloned().map(|g| g.into()).collect())
    }

    #[setter]
    pub fn set_gradients(&mut self, gradients: Vec<PyGradient>) -> PyResult<()> {
        self.as_mut()?.0 = gradients.into_iter().map(Gradient::from).collect();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let inner = self.as_ref()?;
        Ok(format!(
            "BorderGradient(gradients={})",
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
