use bevy::{
    math::primitives::{Measured3d, Sphere},
    mesh::Meshable,
};
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PySphereMeshBuilder};

#[pyclass(name = "Sphere", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PySphere(pub Sphere);

#[pymethods]
impl PySphere {
    #[new]
    #[pyo3(signature = (radius=0.5))]
    pub fn new(radius: f32) -> PyClassInitializer<Self> {
        (Self(Sphere::new(radius)), PyMeshable).into()
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

    pub fn volume(&self) -> f32 {
        self.0.volume()
    }

    pub fn closest_point(&self, point: &PyVec3) -> PyResult<PyVec3> {
        Ok(self.0.closest_point(point.try_into()?).try_into()?)
    }

    pub fn __repr__(&self) -> String {
        format!("Sphere(radius={})", self.0.radius)
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PySphereMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }
}

impl From<Sphere> for PySphere {
    fn from(sphere: Sphere) -> Self {
        PySphere(sphere)
    }
}
