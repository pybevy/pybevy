use bevy::light::ShadowFilteringMethod;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;

#[pywrap(ShadowFilteringMethod, bridge)]
#[pyclass(name = "ShadowFilteringMethod", module = "pybevy.light", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyShadowFilteringMethod(pub(crate) ShadowFilteringMethod);

#[pymethods]
impl PyShadowFilteringMethod {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(ShadowFilteringMethod::Gaussian).into()
    }

    #[staticmethod]
    #[pyo3(name = "Hardware2x2")]
    pub fn hardware_2x2(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(ShadowFilteringMethod::Hardware2x2))
    }

    #[staticmethod]
    #[pyo3(name = "Gaussian")]
    pub fn gaussian(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(ShadowFilteringMethod::Gaussian))
    }

    #[staticmethod]
    #[pyo3(name = "Temporal")]
    pub fn temporal(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(ShadowFilteringMethod::Temporal))
    }

    pub fn __repr__(&self) -> &'static str {
        match self.0 {
            ShadowFilteringMethod::Hardware2x2 => "ShadowFilteringMethod.Hardware2x2",
            ShadowFilteringMethod::Gaussian => "ShadowFilteringMethod.Gaussian",
            ShadowFilteringMethod::Temporal => "ShadowFilteringMethod.Temporal",
        }
    }
}
