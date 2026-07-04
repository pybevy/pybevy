use bevy::ecs::entity_disabling::Disabled;
use pybevy_core::PyComponent;
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(Disabled, unit, bridge)]
#[pyclass(name = "Disabled", extends = PyComponent, frozen, eq, skip_from_py_object)]
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
    pub fn new() -> PyClassInitializer<Self> {
        (PyDisabled, PyComponent).into()
    }

    fn __repr__(&self) -> &'static str {
        "Disabled"
    }
}
