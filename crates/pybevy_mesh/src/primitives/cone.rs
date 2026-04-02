use bevy::mesh::{ConeMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "ConeMeshBuilder", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyConeMeshBuilder(ConeMeshBuilder);

impl From<ConeMeshBuilder> for PyConeMeshBuilder {
    fn from(builder: ConeMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyConeMeshBuilder {
    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
