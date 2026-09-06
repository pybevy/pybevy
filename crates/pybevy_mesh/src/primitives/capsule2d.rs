use bevy::mesh::{Capsule2dMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "Capsule2dMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyCapsule2dMeshBuilder(pub(crate) Capsule2dMeshBuilder);

impl From<Capsule2dMeshBuilder> for PyCapsule2dMeshBuilder {
    fn from(builder: Capsule2dMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCapsule2dMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: u32) -> PyResult<Py<Self>> {
        let mut builder = self.0;
        builder.resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
