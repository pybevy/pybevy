use bevy::mesh::{Capsule2dMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "Capsule2dMeshBuilder", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyCapsule2dMeshBuilder(pub(crate) Capsule2dMeshBuilder);

impl From<Capsule2dMeshBuilder> for PyCapsule2dMeshBuilder {
    fn from(builder: Capsule2dMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCapsule2dMeshBuilder {
    pub fn resolution(mut self_: PyRefMut<'_, Self>, resolution: u32) -> PyRefMut<'_, Self> {
        self_.0.resolution = resolution;
        self_
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
