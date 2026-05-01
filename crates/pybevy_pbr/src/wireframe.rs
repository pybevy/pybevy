use bevy::pbr::wireframe::Wireframe;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(Wireframe, bridge)]
#[pyclass(name = "Wireframe", extends = PyComponent)]
#[derive(Debug)]
pub struct PyWireframe {
    pub(crate) storage: ComponentStorage<Wireframe>,
}

#[pymethods]
impl PyWireframe {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        Self::from_owned(Wireframe)
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
