use bevy::mesh::{CylinderMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "CylinderMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyCylinderMeshBuilder(CylinderMeshBuilder);

impl From<CylinderMeshBuilder> for PyCylinderMeshBuilder {
    fn from(builder: CylinderMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCylinderMeshBuilder {
    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
