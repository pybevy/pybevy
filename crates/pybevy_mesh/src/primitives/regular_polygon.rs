use bevy::mesh::{MeshBuilder, RegularPolygonMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder, primitives::validation::vertex_budget};

#[pyclass(name = "RegularPolygonMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyRegularPolygonMeshBuilder {
    builder: RegularPolygonMeshBuilder,
    sides: u32,
}

impl PyRegularPolygonMeshBuilder {
    pub(crate) fn new(builder: RegularPolygonMeshBuilder, sides: u32) -> Self {
        Self { builder, sides }
    }
}

#[pymethods]
impl PyRegularPolygonMeshBuilder {
    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        vertex_budget("RegularPolygonMeshBuilder", u128::from(self.sides))?;
        Py::new(py, (self.builder.build().into(), PyAsset))
    }
}
