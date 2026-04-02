use bevy::mesh::{MeshBuilder, TetrahedronMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "TetrahedronMeshBuilder", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyTetrahedronMeshBuilder(pub(crate) TetrahedronMeshBuilder);

impl From<TetrahedronMeshBuilder> for PyTetrahedronMeshBuilder {
    fn from(builder: TetrahedronMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyTetrahedronMeshBuilder {
    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
