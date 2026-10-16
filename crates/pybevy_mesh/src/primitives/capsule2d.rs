use bevy::mesh::{Capsule2dMeshBuilder, MeshBuilder};
use pybevy_core::{
    PyAsset,
    domain::{IntegerInput, check_for},
};
use pyo3::prelude::*;

use crate::{
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::validation::{AT_LEAST_TWO, vertex_budget},
};

#[pyclass(name = "Capsule2dMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyCapsule2dMeshBuilder(pub(crate) Capsule2dMeshBuilder);

impl From<Capsule2dMeshBuilder> for PyCapsule2dMeshBuilder {
    fn from(builder: Capsule2dMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCapsule2dMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: IntegerInput<u32>) -> PyResult<Py<Self>> {
        let resolution = check_for::<Self, _>("resolution", resolution, AT_LEAST_TWO)?.stored();
        let mut builder = self.0;
        builder.resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        check_for::<Self, _>("resolution", self.0.resolution, AT_LEAST_TWO)?;
        vertex_budget("Capsule2dMeshBuilder", 2 * u128::from(self.0.resolution))?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
