use bevy::mesh::{MeshBuilder, Segment2dMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "Segment2dMeshBuilder", extends = PyMeshBuilder, frozen)]
pub struct PySegment2dMeshBuilder(Segment2dMeshBuilder);

impl From<Segment2dMeshBuilder> for PySegment2dMeshBuilder {
    fn from(builder: Segment2dMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PySegment2dMeshBuilder {
    pub fn build(&self, py: Python<'_>) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
