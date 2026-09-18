use bevy::mesh::{MeshBuilder, Polyline2dMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "Polyline2dMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyPolyline2dMeshBuilder(Polyline2dMeshBuilder);

impl From<Polyline2dMeshBuilder> for PyPolyline2dMeshBuilder {
    fn from(builder: Polyline2dMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyPolyline2dMeshBuilder {
    pub fn build(&self, py: Python<'_>) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
