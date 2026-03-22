use bevy::mesh::{CircularSectorMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "CircularSectorMeshBuilder", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyCircularSectorMeshBuilder(pub(crate) CircularSectorMeshBuilder);

impl From<CircularSectorMeshBuilder> for PyCircularSectorMeshBuilder {
    fn from(builder: CircularSectorMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCircularSectorMeshBuilder {
    pub fn resolution(mut self_: PyRefMut<'_, Self>, resolution: u32) -> PyRefMut<'_, Self> {
        self_.0.resolution = resolution;
        self_
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
