use bevy::window::PrimaryMonitor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(PrimaryMonitor, bridge)]
#[pyclass(name = "PrimaryMonitor", extends = PyComponent, frozen)]
#[derive(Clone)]
pub struct PyPrimaryMonitor {
    pub(crate) storage: ComponentStorage<PrimaryMonitor>,
}

#[pymethods]
impl PyPrimaryMonitor {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        Self::from_owned(PrimaryMonitor)
    }

    pub fn __repr__(&self) -> &str {
        "PrimaryMonitor"
    }
}
