use bevy::mesh::{ConicalFrustumMeshBuilder, MeshBuilder};
use pybevy_core::{PyAsset, domain::IntegerInput};
use pyo3::prelude::*;

use crate::{
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::validation::{
        AT_LEAST_ONE, AT_LEAST_THREE, minimum_count, stored_count, vertex_budget,
    },
};

const OWNER: &str = "ConicalFrustumMeshBuilder";

fn validate_build(builder: &ConicalFrustumMeshBuilder) -> PyResult<()> {
    stored_count(OWNER, "resolution", builder.resolution, AT_LEAST_THREE)?;
    stored_count(OWNER, "segments", builder.segments, AT_LEAST_ONE)?;
    let resolution = u128::from(builder.resolution);
    let segments = u128::from(builder.segments);
    let vertices = 2 * resolution + (segments + 1) * (resolution + 1);
    vertex_budget(OWNER, vertices)
}

#[pyclass(name = "ConicalFrustumMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyConicalFrustumMeshBuilder(ConicalFrustumMeshBuilder);

impl From<ConicalFrustumMeshBuilder> for PyConicalFrustumMeshBuilder {
    fn from(builder: ConicalFrustumMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyConicalFrustumMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: IntegerInput<u32>) -> PyResult<Py<Self>> {
        let resolution = minimum_count(OWNER, "resolution", resolution, AT_LEAST_THREE)?;
        stored_count(OWNER, "segments", self.0.segments, AT_LEAST_ONE)?;
        Py::new(py, (Self(self.0.resolution(resolution)), PyMeshBuilder))
    }

    pub fn segments(&self, py: Python<'_>, segments: IntegerInput<u32>) -> PyResult<Py<Self>> {
        let segments = minimum_count(OWNER, "segments", segments, AT_LEAST_ONE)?;
        stored_count(OWNER, "resolution", self.0.resolution, AT_LEAST_THREE)?;
        Py::new(py, (Self(self.0.segments(segments)), PyMeshBuilder))
    }

    pub fn build(&self, py: Python<'_>) -> PyResult<Py<PyMesh>> {
        validate_build(&self.0)?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
