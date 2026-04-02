use bevy::mesh::{MeshBuilder, TorusMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "TorusMeshBuilder", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyTorusMeshBuilder(pub(crate) TorusMeshBuilder);

impl From<TorusMeshBuilder> for PyTorusMeshBuilder {
    fn from(builder: TorusMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyTorusMeshBuilder {
    pub fn minor_resolution(
        mut self_: PyRefMut<'_, Self>,
        resolution: usize,
    ) -> PyRefMut<'_, Self> {
        self_.0.minor_resolution = resolution;
        self_
    }

    pub fn major_resolution(
        mut self_: PyRefMut<'_, Self>,
        resolution: usize,
    ) -> PyRefMut<'_, Self> {
        self_.0.major_resolution = resolution;
        self_
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
