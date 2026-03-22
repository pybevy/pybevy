use bevy::mesh::{CircularSegmentMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "CircularSegmentMeshBuilder", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyCircularSegmentMeshBuilder(pub(crate) CircularSegmentMeshBuilder);

impl From<CircularSegmentMeshBuilder> for PyCircularSegmentMeshBuilder {
    fn from(builder: CircularSegmentMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCircularSegmentMeshBuilder {
    pub fn resolution(mut self_: PyRefMut<'_, Self>, resolution: u32) -> PyRefMut<'_, Self> {
        self_.0.resolution = resolution;
        self_
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
