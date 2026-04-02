use bevy::mesh::{Capsule3dMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "Capsule3dMeshBuilder", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyCapsule3dMeshBuilder(pub(crate) Capsule3dMeshBuilder);

impl From<Capsule3dMeshBuilder> for PyCapsule3dMeshBuilder {
    fn from(builder: Capsule3dMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCapsule3dMeshBuilder {
    pub fn rings(mut self_: PyRefMut<'_, Self>, rings: u32) -> PyRefMut<'_, Self> {
        self_.0.rings = rings;
        self_
    }

    pub fn longitudes(mut self_: PyRefMut<'_, Self>, longitudes: u32) -> PyRefMut<'_, Self> {
        self_.0.longitudes = longitudes;
        self_
    }

    pub fn latitudes(mut self_: PyRefMut<'_, Self>, latitudes: u32) -> PyRefMut<'_, Self> {
        self_.0.latitudes = latitudes;
        self_
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
