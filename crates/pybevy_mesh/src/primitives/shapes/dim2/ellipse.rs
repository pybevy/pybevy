use bevy::{
    math::{
        Vec2,
        primitives::{Ellipse, Measured2d},
    },
    mesh::Meshable,
};
use pybevy_math::PyVec2;
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyEllipseMeshBuilder};

#[pyclass(name = "Ellipse", extends = PyMeshable, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyEllipse(pub(crate) Ellipse);

impl From<PyEllipse> for Ellipse {
    fn from(py_ellipse: PyEllipse) -> Self {
        py_ellipse.0
    }
}

impl From<Ellipse> for PyEllipse {
    fn from(ellipse: Ellipse) -> Self {
        PyEllipse(ellipse)
    }
}

#[pymethods]
impl PyEllipse {
    #[new]
    #[pyo3(signature = (half_size = PyVec2::ONE))]
    pub fn new(half_size: PyVec2) -> (Self, PyMeshable) {
        let size: Vec2 = half_size.into();
        (Self(Ellipse::from_size(size)), PyMeshable)
    }

    #[getter]
    pub fn half_size(&self) -> PyVec2 {
        PyVec2::from_vec2(self.0.half_size)
    }

    #[setter]
    pub fn set_half_size(&mut self, value: PyVec2) {
        self.0.half_size = value.into();
    }

    pub fn eccentricity(&self) -> f32 {
        self.0.eccentricity()
    }

    pub fn focal_length(&self) -> f32 {
        self.0.focal_length()
    }

    pub fn semi_major(&self) -> f32 {
        self.0.semi_major()
    }

    pub fn semi_minor(&self) -> f32 {
        self.0.semi_minor()
    }

    #[staticmethod]
    pub fn from_size(py: Python, size: PyVec2) -> PyResult<Py<Self>> {
        let bevy_size: Vec2 = size.into();
        Py::new(py, (Self(Ellipse::from_size(bevy_size)), PyMeshable))
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyEllipseMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "Ellipse(half_size=Vec2({}, {}))",
            self.0.half_size.x, self.0.half_size.y
        )
    }
}
