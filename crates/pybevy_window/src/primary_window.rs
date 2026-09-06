use bevy::window::PrimaryWindow;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(PrimaryWindow, bridge)]
#[pyclass(name = "PrimaryWindow", module = "pybevy.window", extends = PyComponent, frozen, eq)]
#[derive(PartialEq)]
pub struct PyPrimaryWindow {
    pub(crate) storage: ComponentStorage<PrimaryWindow>,
}

#[pymethods]
impl PyPrimaryWindow {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(PrimaryWindow).into()
    }

    pub fn __repr__(&self) -> &str {
        "PrimaryWindow"
    }
}
