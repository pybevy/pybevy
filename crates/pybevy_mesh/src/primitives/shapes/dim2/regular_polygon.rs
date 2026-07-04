use bevy::{
    math::primitives::{Measured2d, RegularPolygon},
    mesh::Meshable,
};
use pyo3::prelude::*;

use super::circle::PyCircle;
use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyRegularPolygonMeshBuilder,
};

#[pyclass(name = "RegularPolygon", extends = PyMeshable, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyRegularPolygon(pub(crate) RegularPolygon);

impl From<PyRegularPolygon> for RegularPolygon {
    fn from(py_polygon: PyRegularPolygon) -> Self {
        py_polygon.0
    }
}

impl From<RegularPolygon> for PyRegularPolygon {
    fn from(polygon: RegularPolygon) -> Self {
        PyRegularPolygon(polygon)
    }
}

#[pymethods]
impl PyRegularPolygon {
    #[new]
    #[pyo3(signature = (circumradius = 0.5, sides = 6))]
    pub fn new(circumradius: f32, sides: u32) -> (Self, PyMeshable) {
        (Self(RegularPolygon::new(circumradius, sides)), PyMeshable)
    }

    #[getter]
    pub fn circumcircle(&self, py: Python<'_>) -> PyResult<Py<PyCircle>> {
        Py::new(py, (self.0.circumcircle.into(), PyMeshable))
    }

    #[getter]
    pub fn sides(&self) -> u32 {
        self.0.sides
    }

    #[setter]
    pub fn set_sides(&mut self, value: u32) {
        self.0.sides = value;
    }

    pub fn circumradius(&self) -> f32 {
        self.0.circumradius()
    }

    pub fn inradius(&self) -> f32 {
        self.0.inradius()
    }

    pub fn side_length(&self) -> f32 {
        self.0.side_length()
    }

    pub fn internal_angle_degrees(&self) -> f32 {
        self.0.internal_angle_degrees()
    }

    pub fn internal_angle_radians(&self) -> f32 {
        self.0.internal_angle_radians()
    }

    pub fn external_angle_degrees(&self) -> f32 {
        self.0.external_angle_degrees()
    }

    pub fn external_angle_radians(&self) -> f32 {
        self.0.external_angle_radians()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyRegularPolygonMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "RegularPolygon(circumradius={}, sides={})",
            self.0.circumradius(),
            self.0.sides
        )
    }
}
