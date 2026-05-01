use bevy::ui::BackgroundGradient;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::gradient::PyGradient;

#[pycomponent(BackgroundGradient, bridge)]
#[pyclass(name = "BackgroundGradient", extends = PyComponent, eq)]
#[derive(Debug, PartialEq)]
pub struct PyBackgroundGradient {
    pub(crate) storage: ComponentStorage<BackgroundGradient>,
}

#[pymethods]
impl PyBackgroundGradient {
    #[new]
    #[pyo3(signature = (gradients = vec![]))]
    pub fn new(gradients: Vec<PyGradient>) -> (Self, PyComponent) {
        Self::from_owned(BackgroundGradient(
            gradients.into_iter().map(|g| g.inner).collect(),
        ))
    }

    pub fn add_gradient(&mut self, gradient: PyGradient) -> PyResult<()> {
        self.as_mut()?.0.push(gradient.inner);
        Ok(())
    }

    #[getter]
    pub fn gradients(&self) -> PyResult<Vec<PyGradient>> {
        Ok(self.as_ref()?.0.iter().cloned().map(|g| g.into()).collect())
    }

    pub fn len(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.0.len())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.is_empty())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let inner = self.as_ref()?;
        Ok(format!("BackgroundGradient(gradients={})", inner.0.len()))
    }
}
