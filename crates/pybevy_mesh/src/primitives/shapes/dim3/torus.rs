use bevy::{
    math::primitives::{Measured3d, Torus},
    mesh::Meshable,
};
use pybevy_math::PyTorusKind;
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyTorusMeshBuilder};

#[pyclass(name = "Torus", extends = PyMeshable, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyTorus(pub(crate) Torus);

impl From<PyTorus> for Torus {
    fn from(py_torus: PyTorus) -> Self {
        py_torus.0
    }
}

impl From<Torus> for PyTorus {
    fn from(torus: Torus) -> Self {
        PyTorus(torus)
    }
}

#[pymethods]
impl PyTorus {
    #[new]
    #[pyo3(signature = (inner_radius = 0.5, outer_radius = 1.0, *, minor_radius = None, major_radius = None))]
    pub fn new(
        inner_radius: f32,
        outer_radius: f32,
        minor_radius: Option<f32>,
        major_radius: Option<f32>,
    ) -> (Self, PyMeshable) {
        if let (Some(minor), Some(major)) = (minor_radius, major_radius) {
            return (
                Self(Torus {
                    minor_radius: minor,
                    major_radius: major,
                }),
                PyMeshable,
            );
        }
        (Self(Torus::new(inner_radius, outer_radius)), PyMeshable)
    }

    #[getter]
    pub fn minor_radius(&self) -> f32 {
        self.0.minor_radius
    }

    #[setter]
    pub fn set_minor_radius(&mut self, value: f32) {
        self.0.minor_radius = value;
    }

    #[getter]
    pub fn major_radius(&self) -> f32 {
        self.0.major_radius
    }

    #[setter]
    pub fn set_major_radius(&mut self, value: f32) {
        self.0.major_radius = value;
    }

    pub fn inner_radius(&self) -> f32 {
        self.0.inner_radius()
    }

    pub fn outer_radius(&self) -> f32 {
        self.0.outer_radius()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn volume(&self) -> f32 {
        self.0.volume()
    }

    pub fn kind(&self) -> PyTorusKind {
        self.0.kind().into()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyTorusMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "Torus(minor_radius={}, major_radius={})",
            self.0.minor_radius, self.0.major_radius
        )
    }
}
