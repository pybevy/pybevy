use std::ops::Range;

use bevy::mesh::{MeshBuilder, TorusMeshBuilder};
use pybevy_core::PyAsset;
use pybevy_math::range::PyRange;
use pyo3::prelude::*;

use crate::{
    mesh::PyMesh, mesh_builder::PyMeshBuilder, primitives::validation::vertex_budget_product,
};

#[pyclass(name = "TorusMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyTorusMeshBuilder(pub(crate) TorusMeshBuilder);

impl From<TorusMeshBuilder> for PyTorusMeshBuilder {
    fn from(builder: TorusMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyTorusMeshBuilder {
    pub fn minor_resolution(&self, py: Python<'_>, resolution: usize) -> PyResult<Py<Self>> {
        let mut builder = self.0.clone();
        builder.minor_resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn major_resolution(&self, py: Python<'_>, resolution: usize) -> PyResult<Py<Self>> {
        let mut builder = self.0.clone();
        builder.major_resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn angle_range(&self, py: Python<'_>, range: &PyRange) -> PyResult<Py<Self>> {
        let range: Range<f32> = range.try_into()?;
        Py::new(
            py,
            (
                Self(self.0.clone().angle_range(range.start..=range.end)),
                PyMeshBuilder,
            ),
        )
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        vertex_budget_product(
            "TorusMeshBuilder",
            self.0.major_resolution as u128 + 1,
            self.0.minor_resolution as u128 + 1,
        )?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
