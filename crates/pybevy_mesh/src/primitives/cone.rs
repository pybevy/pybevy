use bevy::mesh::{ConeAnchor, ConeMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::{
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::validation::{minimum_count, vertex_budget},
};

#[pyenum(ConeAnchor)]
#[pyclass(
    name = "ConeAnchor",
    module = "pybevy.mesh",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyConeAnchor {
    MidPoint,
    Tip,
    Base,
}

#[pyclass(name = "ConeMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyConeMeshBuilder(ConeMeshBuilder);

impl From<ConeMeshBuilder> for PyConeMeshBuilder {
    fn from(builder: ConeMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyConeMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: u32) -> PyResult<Py<Self>> {
        minimum_count("ConeMeshBuilder.resolution", resolution, 3)?;
        Py::new(py, (Self(self.0.resolution(resolution)), PyMeshBuilder))
    }

    pub fn anchor(&self, py: Python<'_>, anchor: PyConeAnchor) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.anchor(anchor.into())), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        minimum_count("ConeMeshBuilder.resolution", self.0.resolution, 3)?;
        vertex_budget("ConeMeshBuilder", 2 * u128::from(self.0.resolution) + 1)?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
