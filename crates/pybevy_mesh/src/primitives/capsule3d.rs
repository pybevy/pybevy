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
    pub fn rings(&self, py: Python<'_>, rings: u32) -> PyResult<Py<Self>> {
        let mut builder = self.0;
        builder.rings = rings;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn longitudes(&self, py: Python<'_>, longitudes: u32) -> PyResult<Py<Self>> {
        let mut builder = self.0;
        builder.longitudes = longitudes;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn latitudes(&self, py: Python<'_>, latitudes: u32) -> PyResult<Py<Self>> {
        let mut builder = self.0;
        builder.latitudes = latitudes;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
