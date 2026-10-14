use bevy::mesh::{CircularSegmentMeshBuilder, MeshBuilder};
use pybevy_core::{PyAsset, domain::IntegerInput};
use pyo3::prelude::*;

use crate::{
    circular_mesh_uv_mode::PyCircularMeshUvMode,
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::validation::{AT_LEAST_TWO, minimum_count, stored_count, vertex_budget},
};

#[pyclass(name = "CircularSegmentMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyCircularSegmentMeshBuilder(pub(crate) CircularSegmentMeshBuilder);

impl From<CircularSegmentMeshBuilder> for PyCircularSegmentMeshBuilder {
    fn from(builder: CircularSegmentMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCircularSegmentMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: IntegerInput<u32>) -> PyResult<Py<Self>> {
        let resolution = minimum_count(
            "CircularSegmentMeshBuilder",
            "resolution",
            resolution,
            AT_LEAST_TWO,
        )?;
        let mut builder = self.0;
        builder.resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn uv_mode(&self, py: Python<'_>, mode: PyCircularMeshUvMode) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.uv_mode(mode.into())), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        stored_count(
            "CircularSegmentMeshBuilder",
            "resolution",
            self.0.resolution,
            AT_LEAST_TWO,
        )?;
        vertex_budget(
            "CircularSegmentMeshBuilder",
            u128::from(self.0.resolution) + 1,
        )?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
