use bevy::ecs::entity_disabling::Disabled;
use pybevy_core::{PyComponent, registry::global_registry};
use pybevy_macros::unit_bridge;
use pyo3::prelude::*;

unit_bridge!(Disabled, PyDisabled);

pub fn register_disabled_bridge() {
    global_registry::register_component_bridge(DisabledBridge);
}

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
