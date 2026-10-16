use bevy::mesh::{EllipseMeshBuilder, MeshBuilder};
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

#[pyclass(name = "EllipseMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyEllipseMeshBuilder(pub(crate) EllipseMeshBuilder);

impl From<EllipseMeshBuilder> for PyEllipseMeshBuilder {
    fn from(builder: EllipseMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyEllipseMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: IntegerInput<u32>) -> PyResult<Py<Self>> {
        let resolution = check_for::<Self, _>("resolution", resolution, AT_LEAST_THREE)?.stored();
        let mut builder = self.0;
        builder.resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        check_for::<Self, _>("resolution", self.0.resolution, AT_LEAST_THREE)?;
        vertex_budget("EllipseMeshBuilder", u128::from(self.0.resolution))?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
