use bevy::mesh::{MeshBuilder, SphereMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder, sphere_kind::PySphereKind};

#[pyclass(name = "SphereMeshBuilder", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PySphereMeshBuilder(SphereMeshBuilder);

impl From<SphereMeshBuilder> for PySphereMeshBuilder {
    fn from(builder: SphereMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PySphereMeshBuilder {
    pub fn kind(&self) -> PySphereKind {
        self.0.kind.into()
    }

    pub fn ico(&self, py: Python, subdivisions: u32) -> PyResult<Py<PyMesh>> {
        let builder = self
            .0
            .ico(subdivisions)
            .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?
            .into();

        Py::new(py, (builder, PyAsset))
    }

    pub fn uv(&self, py: Python, sectors: u32, stacks: u32) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.uv(sectors, stacks).into(), PyAsset))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
