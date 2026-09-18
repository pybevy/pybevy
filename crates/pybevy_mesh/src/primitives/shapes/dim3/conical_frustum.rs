use bevy::{
    math::primitives::{ConicalFrustum, Measured3d},
    mesh::Meshable,
};
use pyo3::prelude::*;

use crate::{
    mesh_builder::PyMeshBuilder,
    meshable::PyMeshable,
    primitives::{PyCircle, PyConicalFrustumMeshBuilder},
};

#[pyclass(name = "ConicalFrustum", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyConicalFrustum(pub(crate) ConicalFrustum);

impl From<ConicalFrustum> for PyConicalFrustum {
    fn from(frustum: ConicalFrustum) -> Self {
        Self(frustum)
    }
}

impl From<PyConicalFrustum> for ConicalFrustum {
    fn from(py_frustum: PyConicalFrustum) -> Self {
        py_frustum.0
    }
}

#[pymethods]
impl PyConicalFrustum {
    #[new]
    #[pyo3(signature = (*, radius_top = 0.25, radius_bottom = 0.5, height = 0.5))]
    pub fn new(radius_top: f32, radius_bottom: f32, height: f32) -> PyClassInitializer<Self> {
        (
            Self(ConicalFrustum {
                radius_top,
                radius_bottom,
                height,
            }),
            PyMeshable,
        )
            .into()
    }

    #[getter]
    pub fn radius_top(&self) -> f32 {
        self.0.radius_top
    }

    #[setter]
    pub fn set_radius_top(&mut self, value: f32) {
        self.0.radius_top = value;
    }

    #[getter]
    pub fn radius_bottom(&self) -> f32 {
        self.0.radius_bottom
    }

    #[setter]
    pub fn set_radius_bottom(&mut self, value: f32) {
        self.0.radius_bottom = value;
    }

    #[getter]
    pub fn height(&self) -> f32 {
        self.0.height
    }

    #[setter]
    pub fn set_height(&mut self, value: f32) {
        self.0.height = value;
    }

    pub fn bottom_base(&self, py: Python<'_>) -> PyResult<Py<PyCircle>> {
        Py::new(py, (self.0.bottom_base().into(), PyMeshable))
    }

    pub fn top_base(&self, py: Python<'_>) -> PyResult<Py<PyCircle>> {
        Py::new(py, (self.0.top_base().into(), PyMeshable))
    }

    pub fn slant_height(&self) -> f32 {
        self.0.slant_height()
    }

    pub fn lateral_area(&self) -> f32 {
        self.0.lateral_area()
    }

    pub fn bottom_base_area(&self) -> f32 {
        self.0.bottom_base_area()
    }

    pub fn top_base_area(&self) -> f32 {
        self.0.top_base_area()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn volume(&self) -> f32 {
        self.0.volume()
    }

    pub fn mesh(&self, py: Python<'_>) -> PyResult<Py<PyConicalFrustumMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "ConicalFrustum(radius_top={}, radius_bottom={}, height={})",
            self.0.radius_top, self.0.radius_bottom, self.0.height
        )
    }
}
