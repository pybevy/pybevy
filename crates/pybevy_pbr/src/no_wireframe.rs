use bevy::pbr::wireframe::NoWireframe;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

#[component_storage(NoWireframe, bridge)]
#[pyclass(name = "NoWireframe", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyNoWireframe {
    pub(crate) storage: ComponentStorage<NoWireframe>,
}

#[pymethods]
impl PyNoWireframe {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        Self::from_owned(NoWireframe)
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
