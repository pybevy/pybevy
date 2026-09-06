use bevy::mesh::{MeshBuilder, Triangle3dMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "Triangle3dMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyTriangle3dMeshBuilder(pub(crate) Triangle3dMeshBuilder);

impl From<Triangle3dMeshBuilder> for PyTriangle3dMeshBuilder {
    fn from(builder: Triangle3dMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyTriangle3dMeshBuilder {
    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
