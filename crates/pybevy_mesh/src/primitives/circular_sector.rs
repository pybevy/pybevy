use bevy::mesh::{CircularSectorMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "CircularSectorMeshBuilder", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyCircularSectorMeshBuilder(pub(crate) CircularSectorMeshBuilder);

impl From<CircularSectorMeshBuilder> for PyCircularSectorMeshBuilder {
    fn from(builder: CircularSectorMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCircularSectorMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: u32) -> PyResult<Py<Self>> {
        let mut builder = self.0.clone();
        builder.resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
