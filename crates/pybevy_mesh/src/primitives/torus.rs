use bevy::mesh::{MeshBuilder, TorusMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "TorusMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyTorusMeshBuilder(pub(crate) TorusMeshBuilder);

impl From<TorusMeshBuilder> for PyTorusMeshBuilder {
    fn from(builder: TorusMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyTorusMeshBuilder {
    pub fn minor_resolution(&self, py: Python<'_>, resolution: usize) -> PyResult<Py<Self>> {
        let mut builder = self.0.clone();
        builder.minor_resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn major_resolution(&self, py: Python<'_>, resolution: usize) -> PyResult<Py<Self>> {
        let mut builder = self.0.clone();
        builder.major_resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
