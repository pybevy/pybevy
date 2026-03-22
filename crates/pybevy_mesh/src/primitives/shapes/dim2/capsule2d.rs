use bevy::{
    math::primitives::{Capsule2d, Measured2d},
    mesh::Meshable,
};
use pyo3::prelude::*;

use super::rectangle::PyRectangle;
use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyCapsule2dMeshBuilder,
};

#[pyclass(name = "Capsule2d", extends = PyMeshable, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyCapsule2d(pub(crate) Capsule2d);

impl From<PyCapsule2d> for Capsule2d {
    fn from(py_capsule: PyCapsule2d) -> Self {
        py_capsule.0
    }
}

impl From<Capsule2d> for PyCapsule2d {
    fn from(capsule: Capsule2d) -> Self {
        PyCapsule2d(capsule)
    }
}

#[pymethods]
impl PyCapsule2d {
    #[new]
    #[pyo3(signature = (radius = 0.5, length = 1.0))]
    pub fn new(radius: f32, length: f32) -> (Self, PyMeshable) {
        (Self(Capsule2d::new(radius, length)), PyMeshable)
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
    pub fn half_length(&self) -> f32 {
        self.0.half_length
    }

    #[setter]
    pub fn set_half_length(&mut self, value: f32) {
        self.0.half_length = value;
    }

    pub fn inner_rectangle(&self, py: Python<'_>) -> PyResult<Py<PyRectangle>> {
        Py::new(py, (self.0.to_inner_rectangle().into(), PyMeshable))
    }

    pub fn to_inner_rectangle(&self, py: Python<'_>) -> PyResult<Py<PyRectangle>> {
        Py::new(py, (self.0.to_inner_rectangle().into(), PyMeshable))
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyCapsule2dMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "Capsule2d(radius={}, length={})",
            self.0.radius,
            self.0.half_length * 2.0
        )
    }
}
