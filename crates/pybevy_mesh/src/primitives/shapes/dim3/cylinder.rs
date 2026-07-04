use bevy::{
    math::primitives::{Cylinder, Measured3d},
    mesh::Meshable,
};
use pyo3::prelude::*;

use crate::{
    mesh_builder::PyMeshBuilder,
    meshable::PyMeshable,
    primitives::{PyCylinderMeshBuilder, shapes::dim2::circle::PyCircle},
};

#[pyclass(name = "Cylinder", extends = PyMeshable, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyCylinder(pub(crate) Cylinder);

#[pymethods]
impl PyCylinder {
    #[new]
    #[pyo3(signature = (radius=0.5, height=1.0))]
    pub fn new(radius: f32, height: f32) -> (Self, PyMeshable) {
        (Self(Cylinder::new(radius, height)), PyMeshable)
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
    pub fn half_height(&self) -> f32 {
        self.0.half_height
    }

    #[setter]
    pub fn set_half_height(&mut self, value: f32) {
        self.0.half_height = value;
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

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn volume(&self) -> f32 {
        self.0.volume()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyCylinderMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    pub fn __repr__(&self) -> String {
        format!(
            "Cylinder(radius={}, half_height={})",
            self.0.radius, self.0.half_height
        )
    }
}

impl From<Cylinder> for PyCylinder {
    fn from(cylinder: Cylinder) -> Self {
        PyCylinder(cylinder)
    }
}
