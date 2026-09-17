use bevy::mesh::{ConicalFrustumMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "ConicalFrustumMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyConicalFrustumMeshBuilder(ConicalFrustumMeshBuilder);

impl From<ConicalFrustumMeshBuilder> for PyConicalFrustumMeshBuilder {
    fn from(builder: ConicalFrustumMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyConicalFrustumMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: u32) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.resolution(resolution)), PyMeshBuilder))
    }

    pub fn segments(&self, py: Python<'_>, segments: u32) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.segments(segments)), PyMeshBuilder))
    }

    pub fn build(&self, py: Python<'_>) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
