use bevy::mesh::{CircleMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::validation::{minimum_count, vertex_budget},
};

#[pyclass(name = "CircleMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyCircleMeshBuilder(CircleMeshBuilder);

impl PyCircleMeshBuilder {
    pub fn new(inner: CircleMeshBuilder) -> Self {
        PyCircleMeshBuilder(inner)
    }
}

impl From<CircleMeshBuilder> for PyCircleMeshBuilder {
    fn from(value: CircleMeshBuilder) -> Self {
        PyCircleMeshBuilder::new(value)
    }
}

#[pymethods]
impl PyCircleMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: u32) -> PyResult<Py<Self>> {
        minimum_count("CircleMeshBuilder.resolution", resolution, 3)?;
        Py::new(py, (Self(self.0.resolution(resolution)), PyMeshBuilder))
    }

    pub fn build(&self, py: Python<'_>) -> PyResult<Py<PyMesh>> {
        minimum_count("CircleMeshBuilder.resolution", self.0.resolution, 3)?;
        vertex_budget("CircleMeshBuilder", u128::from(self.0.resolution))?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
