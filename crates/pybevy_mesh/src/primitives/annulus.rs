use bevy::mesh::{AnnulusMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder, primitives::validation::minimum_count};

#[pyclass(name = "AnnulusMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder)]
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
        minimum_count("AnnulusMeshBuilder.resolution", resolution, 1)?;
        let mut builder = self.0;
        builder.resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        minimum_count("AnnulusMeshBuilder.resolution", self.0.resolution, 1)?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
