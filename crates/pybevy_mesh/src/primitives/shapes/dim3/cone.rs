use bevy::{
    math::primitives::{Cone, Measured3d},
    mesh::Meshable,
};
use pyo3::prelude::*;

use crate::{
    mesh_builder::PyMeshBuilder,
    meshable::PyMeshable,
    primitives::{PyConeMeshBuilder, shapes::dim2::circle::PyCircle},
};

#[pyclass(name = "Cone", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyCone(pub(crate) Cone);

#[pymethods]
impl PyCone {
    #[new]
    #[pyo3(signature = (radius=0.5, height=1.0))]
    pub fn new(radius: f32, height: f32) -> PyClassInitializer<Self> {
        (Self(Cone::new(radius, height)), PyMeshable).into()
    }

    #[getter]
    pub fn radius(&self) -> f32 {
        self.0.radius
    }

    #[setter]
    pub fn set_radius(&mut self, value: f32) {
        self.0.radius = value;
    }

    #[getter]
    pub fn height(&self) -> f32 {
        self.0.height
    }

    #[setter]
    pub fn set_height(&mut self, value: f32) {
        self.0.height = value;
    }

    pub fn base(&self, py: Python) -> PyResult<Py<PyCircle>> {
        Py::new(py, (PyCircle::from(self.0.base()), PyMeshable))
    }

    pub fn base_area(&self) -> f32 {
        self.0.base_area()
    }

    pub fn lateral_area(&self) -> f32 {
        self.0.lateral_area()
    }

    pub fn slant_height(&self) -> f32 {
        self.0.slant_height()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn volume(&self) -> f32 {
        self.0.volume()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyConeMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    pub fn __repr__(&self) -> String {
        format!("Cone(radius={}, height={})", self.0.radius, self.0.height)
    }
}

impl From<Cone> for PyCone {
    fn from(cone: Cone) -> Self {
        PyCone(cone)
    }
}
