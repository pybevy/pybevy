use bevy::mesh::{MeshBuilder, PlaneMeshBuilder};
use pybevy_core::PyAsset;
use pybevy_math::{PyDir3, PyVec2};
use pyo3::prelude::*;

use crate::{PyMesh, mesh_builder::PyMeshBuilder};

#[pyclass(name = "Plane3dMeshBuilder", extends = PyMeshBuilder)]
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
    pub fn new(py: Python, normal: PyDir3, size: PyVec2) -> PyResult<Py<PyAny>> {
        let builder = PlaneMeshBuilder::new(normal.into(), size.into());
        Ok(Py::new(py, (Self(builder), PyMeshBuilder))?.into_any())
    }

    #[staticmethod]
    pub fn from_size(py: Python, size: PyVec2) -> PyResult<Py<PyAny>> {
        let builder = PlaneMeshBuilder::from_size(size.into());
        Ok(Py::new(py, (Self(builder), PyMeshBuilder))?.into_any())
    }

    #[staticmethod]
    pub fn from_length(py: Python, length: f32) -> PyResult<Py<PyAny>> {
        let builder = PlaneMeshBuilder::from_length(length);
        Ok(Py::new(py, (Self(builder), PyMeshBuilder))?.into_any())
    }

    pub fn normal(mut pyself: PyRefMut<Self>, normal: PyDir3) -> PyRefMut<Self> {
        pyself.0 = pyself.0.normal(normal.into());
        pyself
    }

    pub fn size(mut pyself: PyRefMut<Self>, width: f32, height: f32) -> PyRefMut<Self> {
        pyself.0 = pyself.0.size(width, height);
        pyself
    }

    pub fn subdivisions(mut pyself: PyRefMut<Self>, subdivisions: u32) -> PyRefMut<Self> {
        pyself.0 = pyself.0.subdivisions(subdivisions);
        pyself
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
