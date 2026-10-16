use bevy::mesh::{CircleMeshBuilder, MeshBuilder};
use pybevy_core::{
    PyAsset,
    domain::{IntegerInput, check_for},
};
use pyo3::prelude::*;

use crate::{
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::validation::{AT_LEAST_THREE, vertex_budget},
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
    pub fn resolution(&self, py: Python<'_>, resolution: IntegerInput<u32>) -> PyResult<Py<Self>> {
        let resolution = check_for::<Self, _>("resolution", resolution, AT_LEAST_THREE)?.stored();
        Py::new(py, (Self(self.0.resolution(resolution)), PyMeshBuilder))
    }

    pub fn build(&self, py: Python<'_>) -> PyResult<Py<PyMesh>> {
        check_for::<Self, _>("resolution", self.0.resolution, AT_LEAST_THREE)?;
        vertex_budget("CircleMeshBuilder", u128::from(self.0.resolution))?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
