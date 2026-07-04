use bevy::{
    math::{
        Vec2,
        primitives::{Circle, Measured2d},
    },
    mesh::Meshable,
};
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyCircleMeshBuilder};

#[pyclass(name = "Circle", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyCircle(pub(crate) Circle);

#[pymethods]
impl PyCircle {
    #[new]
    #[pyo3(signature = (radius=0.5))]
    pub fn new(radius: f32) -> PyClassInitializer<Self> {
        (Self(Circle::new(radius)), PyMeshable).into()
    }

    #[getter]
    pub fn radius(&self) -> f32 {
        self.0.radius
    }

    #[setter]
    pub fn set_radius(&mut self, value: f32) {
        self.0.radius = value;
    }

    pub fn diameter(&self) -> f32 {
        self.0.diameter()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn closest_point(&self, point: PyVec2) -> PyVec2 {
        let bevy_point: Vec2 = point.into();
        PyVec2::from_vec2(self.0.closest_point(bevy_point))
    }

    pub fn mesh(&self, py: Python<'_>) -> PyResult<Py<PyCircleMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    pub fn __repr__(&self) -> String {
        format!("Circle(radius={})", self.0.radius)
    }
}

impl From<Circle> for PyCircle {
    fn from(circle: Circle) -> Self {
        PyCircle(circle)
    }
}
