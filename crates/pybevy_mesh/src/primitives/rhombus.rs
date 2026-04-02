use bevy::mesh::{MeshBuilder, RhombusMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "RhombusMeshBuilder", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyRhombusMeshBuilder(pub(crate) RhombusMeshBuilder);

impl From<RhombusMeshBuilder> for PyRhombusMeshBuilder {
    fn from(builder: RhombusMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyRhombusMeshBuilder {
    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
