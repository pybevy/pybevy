use bevy::mesh::MeshTag;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(MeshTag, bridge, view_fields = [0 as value])]
#[pyclass(name = "MeshTag", extends = PyComponent)]
pub struct PyMeshTag {
    pub(crate) storage: ComponentStorage<MeshTag>,
}

#[pymethods]
impl PyMeshTag {
    #[new]
    #[pyo3(signature = (value = 0))]
    pub fn new(value: u32) -> (Self, PyComponent) {
        Self::from_owned(MeshTag(value))
    }

    #[getter]
    pub fn value(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.0)
    }

    #[setter]
    pub fn set_value(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.0 = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("MeshTag({})", self.as_ref()?.0))
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()?.0 == other.as_ref()?.0)
    }
}
