use bevy::ui::FocusPolicy;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(FocusPolicy, bridge)]
#[pyclass(name = "FocusPolicy", module = "pybevy.ui", extends = PyComponent, skip_from_py_object)]
#[derive(Debug)]
pub struct PyFocusPolicy {
    pub(crate) storage: ComponentStorage<FocusPolicy>,
}

#[pymethods]
impl PyFocusPolicy {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(FocusPolicy::default()).into()
    }

    #[staticmethod]
    #[pyo3(name = "Block")]
    pub fn block(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(FocusPolicy::Block))
    }

    #[staticmethod]
    #[pyo3(name = "Pass")]
    pub fn pass(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(FocusPolicy::Pass))
    }

    #[getter]
    pub fn is_block(&self) -> PyResult<bool> {
        Ok(matches!(*self.as_ref()?, FocusPolicy::Block))
    }

    #[getter]
    pub fn is_pass(&self) -> PyResult<bool> {
        Ok(matches!(*self.as_ref()?, FocusPolicy::Pass))
    }

    pub fn set_block(&mut self) -> PyResult<()> {
        *self.as_mut()? = FocusPolicy::Block;
        Ok(())
    }

    pub fn set_pass(&mut self) -> PyResult<()> {
        *self.as_mut()? = FocusPolicy::Pass;
        Ok(())
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(match *self.as_ref()? {
            FocusPolicy::Block => "FocusPolicy.Block".to_string(),
            FocusPolicy::Pass => "FocusPolicy.Pass".to_string(),
        })
    }
}
