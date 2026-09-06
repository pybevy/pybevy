use bevy::mesh::{MeshBuilder, RegularPolygonMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "RegularPolygonMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyRegularPolygonMeshBuilder(pub(crate) RegularPolygonMeshBuilder);

impl From<RegularPolygonMeshBuilder> for PyRegularPolygonMeshBuilder {
    fn from(builder: RegularPolygonMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyRegularPolygonMeshBuilder {
    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
