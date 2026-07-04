use bevy::pbr::wireframe::NoWireframe;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(NoWireframe, bridge)]
#[pyclass(name = "NoWireframe", extends = PyComponent)]
#[derive(Debug)]
pub struct PyNoWireframe {
    pub(crate) storage: ComponentStorage<NoWireframe>,
}

#[pymethods]
impl PyNoWireframe {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(NoWireframe).into()
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
