use bevy::mesh::{MeshBuilder, PlaneMeshBuilder};
use pybevy_core::PyAsset;
use pybevy_math::{dir3::PyDir3, vec2::PyVec2};
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "PlaneMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyPlaneMeshBuilder(PlaneMeshBuilder);

impl From<PlaneMeshBuilder> for PyPlaneMeshBuilder {
    fn from(builder: PlaneMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyPlaneMeshBuilder {
    #[staticmethod]
    #[allow(clippy::new_ret_no_self)]
    pub fn new(py: Python, normal: PyDir3, size: PyVec2) -> PyResult<Py<PyAny>> {
        let builder = PlaneMeshBuilder::new(normal.try_into()?, size.try_into()?);
        Ok(Py::new(py, (Self(builder), PyMeshBuilder))?.into_any())
    }

    #[staticmethod]
    pub fn from_size(py: Python, size: PyVec2) -> PyResult<Py<PyAny>> {
        let builder = PlaneMeshBuilder::from_size(size.try_into()?);
        Ok(Py::new(py, (Self(builder), PyMeshBuilder))?.into_any())
    }

    #[staticmethod]
    pub fn from_length(py: Python, length: f32) -> PyResult<Py<PyAny>> {
        let builder = PlaneMeshBuilder::from_length(length);
        Ok(Py::new(py, (Self(builder), PyMeshBuilder))?.into_any())
    }

    pub fn normal(&self, py: Python<'_>, normal: PyDir3) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.normal(normal.try_into()?)), PyMeshBuilder))
    }

    pub fn size(&self, py: Python<'_>, width: f32, height: f32) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.size(width, height)), PyMeshBuilder))
    }

    pub fn subdivisions(&self, py: Python<'_>, subdivisions: u32) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.subdivisions(subdivisions)), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
