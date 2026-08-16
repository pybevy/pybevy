use bevy::mesh::{AnnulusMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "AnnulusMeshBuilder", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyAnnulusMeshBuilder(pub(crate) AnnulusMeshBuilder);

impl From<AnnulusMeshBuilder> for PyAnnulusMeshBuilder {
    fn from(builder: AnnulusMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyAnnulusMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: u32) -> PyResult<Py<Self>> {
        let mut builder = self.0;
        builder.resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
