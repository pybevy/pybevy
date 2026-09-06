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

    #[classattr]
    #[pyo3(name = "HARDWARE_2X2")]
    pub fn hardware_2x2(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(ShadowFilteringMethod::Hardware2x2))
    }

    #[classattr]
    #[pyo3(name = "GAUSSIAN")]
    pub fn gaussian(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(ShadowFilteringMethod::Gaussian))
    }

    #[classattr]
    #[pyo3(name = "TEMPORAL")]
    pub fn temporal(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(ShadowFilteringMethod::Temporal))
    }
}
