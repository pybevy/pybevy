use bevy::mesh::{ConicalFrustumMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

const MAX_VERTICES: u128 = 1_000_000;

fn validate_counts(builder: &ConicalFrustumMeshBuilder) -> PyResult<()> {
    if builder.resolution < 3 {
        return Err(PyValueError::new_err(
            "ConicalFrustumMeshBuilder.resolution must be at least 3",
        ));
    }
    if builder.segments == 0 {
        return Err(PyValueError::new_err(
            "ConicalFrustumMeshBuilder.segments must be at least 1",
        ));
    }

    Ok(())
}

fn validate_build(builder: &ConicalFrustumMeshBuilder) -> PyResult<()> {
    validate_counts(builder)?;
    let resolution = u128::from(builder.resolution);
    let segments = u128::from(builder.segments);
    let vertices = 2 * resolution + (segments + 1) * (resolution + 1);
    if vertices > MAX_VERTICES {
        return Err(PyValueError::new_err(format!(
            "ConicalFrustumMeshBuilder would create {vertices} vertices; maximum is {MAX_VERTICES}"
        )));
    }
    Ok(())
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
    pub fn resolution(&self, py: Python<'_>, resolution: u32) -> PyResult<Py<Self>> {
        let builder = self.0.resolution(resolution);
        validate_counts(&builder)?;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn segments(&self, py: Python<'_>, segments: u32) -> PyResult<Py<Self>> {
        let builder = self.0.segments(segments);
        validate_counts(&builder)?;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn build(&self, py: Python<'_>) -> PyResult<Py<PyMesh>> {
        validate_build(&self.0)?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
