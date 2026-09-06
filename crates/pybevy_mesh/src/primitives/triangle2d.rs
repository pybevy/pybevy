use bevy::mesh::{MeshBuilder, Triangle2dMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "Triangle2dMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyTriangle2dMeshBuilder(pub(crate) Triangle2dMeshBuilder);

impl From<Triangle2dMeshBuilder> for PyTriangle2dMeshBuilder {
    fn from(builder: Triangle2dMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyTriangle2dMeshBuilder {
    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
