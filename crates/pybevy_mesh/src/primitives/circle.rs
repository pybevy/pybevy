use bevy::mesh::{CircleMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "CircleMeshBuilder", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyCircleMeshBuilder(CircleMeshBuilder);

impl PyCircleMeshBuilder {
    pub fn new(inner: CircleMeshBuilder) -> Self {
        PyCircleMeshBuilder(inner)
    }
}

impl From<CircleMeshBuilder> for PyCircleMeshBuilder {
    fn from(value: CircleMeshBuilder) -> Self {
        PyCircleMeshBuilder::new(value)
    }
}

#[pymethods]
impl PyCircleMeshBuilder {
    pub fn build(&self, py: Python<'_>) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
