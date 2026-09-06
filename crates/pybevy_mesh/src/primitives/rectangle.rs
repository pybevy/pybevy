use bevy::mesh::{MeshBuilder, RectangleMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "RectangleMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyRectangleMeshBuilder(RectangleMeshBuilder);

impl From<RectangleMeshBuilder> for PyRectangleMeshBuilder {
    fn from(value: RectangleMeshBuilder) -> Self {
        PyRectangleMeshBuilder(value)
    }
}

#[pymethods]
impl PyRectangleMeshBuilder {
    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
