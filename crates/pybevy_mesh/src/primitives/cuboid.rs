use bevy::mesh::{CuboidMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "CuboidMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyCuboidMeshBuilder(CuboidMeshBuilder);

impl From<CuboidMeshBuilder> for PyCuboidMeshBuilder {
    fn from(builder: CuboidMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCuboidMeshBuilder {
    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
