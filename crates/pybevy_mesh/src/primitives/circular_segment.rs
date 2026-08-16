use bevy::mesh::{CircularSegmentMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "CircularSegmentMeshBuilder", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyCircularSegmentMeshBuilder(pub(crate) CircularSegmentMeshBuilder);

impl From<CircularSegmentMeshBuilder> for PyCircularSegmentMeshBuilder {
    fn from(builder: CircularSegmentMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCircularSegmentMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: u32) -> PyResult<Py<Self>> {
        let mut builder = self.0;
        builder.resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
