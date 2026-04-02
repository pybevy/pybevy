use bevy::ecs::entity_disabling::Disabled;
use pybevy_core::PyComponent;
use pybevy_macros::component_storage;
use pyo3::prelude::*;

#[component_storage(Disabled, unit, bridge)]
#[pyclass(name = "Disabled", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyDisabled;

impl From<Disabled> for PyDisabled {
    fn from(_: Disabled) -> Self {
        PyDisabled
    }
}

impl From<PyDisabled> for Disabled {
    fn from(_: PyDisabled) -> Self {
        Disabled
    }
}

impl TryFrom<&Disabled> for PyDisabled {
    type Error = PyErr;
    fn try_from(_: &Disabled) -> PyResult<Self> {
        Ok(PyDisabled)
    }
}

#[pymethods]
impl PyDisabled {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (PyDisabled, PyComponent)
    }

    fn __repr__(&self) -> &'static str {
        "Disabled"
    }
}
