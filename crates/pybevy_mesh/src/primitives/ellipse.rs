use bevy::mesh::{EllipseMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "EllipseMeshBuilder", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyEllipseMeshBuilder(pub(crate) EllipseMeshBuilder);

impl From<EllipseMeshBuilder> for PyEllipseMeshBuilder {
    fn from(builder: EllipseMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyEllipseMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: u32) -> PyResult<Py<Self>> {
        let mut builder = self.0;
        builder.resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
