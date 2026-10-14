use bevy::mesh::{ConicalFrustumMeshBuilder, MeshBuilder};
use pybevy_core::{
    PyAsset,
    domain::{IntegerInput, check_for},
};
use pyo3::prelude::*;

use crate::{
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::validation::{AT_LEAST_ONE, AT_LEAST_THREE, vertex_budget},
};

#[pyclass(name = "ConicalFrustumMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyConicalFrustumMeshBuilder(ConicalFrustumMeshBuilder);

impl From<ConicalFrustumMeshBuilder> for PyConicalFrustumMeshBuilder {
    fn from(builder: ConicalFrustumMeshBuilder) -> Self {
        Self(builder)
    }
}

impl PyConicalFrustumMeshBuilder {
    fn validate_build(&self) -> PyResult<()> {
        check_for::<Self, _>("resolution", self.0.resolution, AT_LEAST_THREE)?;
        check_for::<Self, _>("segments", self.0.segments, AT_LEAST_ONE)?;
        let resolution = u128::from(self.0.resolution);
        let segments = u128::from(self.0.segments);
        let vertices = 2 * resolution + (segments + 1) * (resolution + 1);
        vertex_budget("ConicalFrustumMeshBuilder", vertices)
    }
}

#[pymethods]
impl PyConicalFrustumMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: IntegerInput<u32>) -> PyResult<Py<Self>> {
        let resolution = check_for::<Self, _>("resolution", resolution, AT_LEAST_THREE)?.stored();
        check_for::<Self, _>("segments", self.0.segments, AT_LEAST_ONE)?;
        Py::new(py, (Self(self.0.resolution(resolution)), PyMeshBuilder))
    }

    pub fn segments(&self, py: Python<'_>, segments: IntegerInput<u32>) -> PyResult<Py<Self>> {
        let segments = check_for::<Self, _>("segments", segments, AT_LEAST_ONE)?.stored();
        check_for::<Self, _>("resolution", self.0.resolution, AT_LEAST_THREE)?;
        Py::new(py, (Self(self.0.segments(segments)), PyMeshBuilder))
    }

    pub fn build(&self, py: Python<'_>) -> PyResult<Py<PyMesh>> {
        self.validate_build()?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
