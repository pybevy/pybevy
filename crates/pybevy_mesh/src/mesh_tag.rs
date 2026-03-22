use bevy::mesh::MeshTag;
use pybevy_core::{ComponentStorage, PyComponent};
use pyo3::prelude::*;
/// A simple tag component that can be used to identify mesh entities.
#[pyclass(name = "MeshTag", extends = PyComponent)]
#[derive(Clone)]
pub struct PyMeshTag {
    pub(crate) storage: ComponentStorage<MeshTag>,
}

impl PyMeshTag {
    pub fn from_owned(value: MeshTag) -> (Self, PyComponent) {
        (
            PyMeshTag {
                storage: ComponentStorage::owned(value),
            },
            PyComponent,
        )
    }
    pub fn from_borrowed(storage: ComponentStorage<MeshTag>) -> (Self, PyComponent) {
        (PyMeshTag { storage }, PyComponent)
    }

    #[inline(always)]
    pub fn as_ref(&self) -> PyResult<&MeshTag> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> PyResult<&mut MeshTag> {
        Ok(self.storage.as_mut()?)
    }
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
