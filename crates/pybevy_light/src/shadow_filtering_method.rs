use bevy::light::ShadowFilteringMethod;
use pybevy_core::PyComponent;
use pyo3::prelude::*;

/// Shadow filtering method for shadow-casting lights.
///
/// Determines the quality/performance tradeoff for shadow filtering.
#[pyclass(name = "ShadowFilteringMethod", extends = PyComponent, frozen, eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyShadowFilteringMethod(pub(crate) ShadowFilteringMethod);

impl PyShadowFilteringMethod {
    pub fn from_owned(value: ShadowFilteringMethod) -> (Self, PyComponent) {
        (PyShadowFilteringMethod(value), PyComponent)
    }
}

#[pymethods]
impl PyShadowFilteringMethod {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        Self::from_owned(ShadowFilteringMethod::Gaussian)
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
