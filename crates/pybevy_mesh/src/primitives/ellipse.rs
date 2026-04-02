use bevy::mesh::{EllipseMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "EllipseMeshBuilder", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyEllipseMeshBuilder(pub(crate) EllipseMeshBuilder);

impl From<EllipseMeshBuilder> for PyEllipseMeshBuilder {
    fn from(builder: EllipseMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyEllipseMeshBuilder {
    pub fn resolution(mut self_: PyRefMut<'_, Self>, resolution: u32) -> PyRefMut<'_, Self> {
        self_.0.resolution = resolution;
        self_
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
