use bevy::ui::FocusPolicy;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;

#[pywrap(FocusPolicy, bridge)]
#[pyclass(name = "FocusPolicy", extends = PyComponent, eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyFocusPolicy(pub(crate) FocusPolicy);

#[pymethods]
impl PyFocusPolicy {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        Self::from_owned(FocusPolicy::default())
    }

    #[staticmethod]
    pub fn block(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(FocusPolicy::Block))
    }

    #[staticmethod]
    #[pyo3(name = "pass_")]
    pub fn pass(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(FocusPolicy::Pass))
    }

    #[getter]
    pub fn is_block(&self) -> bool {
        matches!(self.0, FocusPolicy::Block)
    }

    #[getter]
    pub fn is_pass(&self) -> bool {
        matches!(self.0, FocusPolicy::Pass)
    }

    pub fn set_block(&mut self) {
        self.0 = FocusPolicy::Block;
    }

    pub fn set_pass(&mut self) {
        self.0 = FocusPolicy::Pass;
    }

    pub fn __repr__(&self) -> String {
        match self.0 {
            FocusPolicy::Block => "FocusPolicy.block()".to_string(),
            FocusPolicy::Pass => "FocusPolicy.pass()".to_string(),
        }
    }
}
