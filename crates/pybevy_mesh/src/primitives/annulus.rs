use bevy::mesh::{AnnulusMeshBuilder, MeshBuilder};
use pybevy_core::{PyAsset, domain::IntegerInput};
use pyo3::prelude::*;

use crate::{
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::validation::{AT_LEAST_ONE, minimum_count, stored_count, vertex_budget},
};

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
    pub fn resolution(&self, py: Python<'_>, resolution: IntegerInput<u32>) -> PyResult<Py<Self>> {
        let resolution =
            minimum_count("AnnulusMeshBuilder", "resolution", resolution, AT_LEAST_ONE)?;
        let mut builder = self.0;
        builder.resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        stored_count(
            "AnnulusMeshBuilder",
            "resolution",
            self.0.resolution,
            AT_LEAST_ONE,
        )?;
        vertex_budget(
            "AnnulusMeshBuilder",
            2 * (u128::from(self.0.resolution) + 1),
        )?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
