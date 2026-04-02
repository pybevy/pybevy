use bevy::{
    math::primitives::{CircularSector, Measured2d},
    mesh::Meshable,
};
use pybevy_math::{primitives::PyArc2d, vec2::PyVec2};
use pyo3::prelude::*;

use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyCircularSectorMeshBuilder,
};

#[pyclass(name = "CircularSector", extends = PyMeshable, eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyCircularSector(pub(crate) CircularSector);

impl From<PyCircularSector> for CircularSector {
    fn from(py: PyCircularSector) -> Self {
        py.0
    }
}

impl From<CircularSector> for PyCircularSector {
    fn from(sector: CircularSector) -> Self {
        PyCircularSector(sector)
    }
}

#[pymethods]
impl PyCircularSector {
    #[new]
    #[pyo3(signature = (radius = 1.0, half_angle = std::f32::consts::FRAC_PI_2, *, arc = None))]
    pub fn new(radius: f32, half_angle: f32, arc: Option<PyArc2d>) -> (Self, PyMeshable) {
        if let Some(a) = arc {
            return (Self(CircularSector { arc: a.into() }), PyMeshable);
        }
        (Self(CircularSector::new(radius, half_angle)), PyMeshable)
    }

    #[staticmethod]
    pub fn from_radians(py: Python<'_>, radius: f32, angle: f32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self(CircularSector::from_radians(radius, angle)),
                PyMeshable,
            ),
        )
    }

    #[staticmethod]
    pub fn from_degrees(py: Python<'_>, radius: f32, angle: f32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self(CircularSector::from_degrees(radius, angle)),
                PyMeshable,
            ),
        )
    }

    #[staticmethod]
    pub fn from_turns(py: Python<'_>, radius: f32, fraction: f32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self(CircularSector::from_turns(radius, fraction)),
                PyMeshable,
            ),
        )
    }

    #[getter]
    pub fn arc(&self) -> PyArc2d {
        self.0.arc.into()
    }

    pub fn radius(&self) -> f32 {
        self.0.radius()
    }

    pub fn half_angle(&self) -> f32 {
        self.0.half_angle()
    }

    pub fn angle(&self) -> f32 {
        self.0.angle()
    }

    pub fn arc_length(&self) -> f32 {
        self.0.arc_length()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn half_chord_length(&self) -> f32 {
        self.0.half_chord_length()
    }

    pub fn chord_length(&self) -> f32 {
        self.0.chord_length()
    }

    pub fn chord_midpoint(&self) -> PyVec2 {
        PyVec2::from_vec2(self.0.chord_midpoint())
    }

    pub fn apothem(&self) -> f32 {
        self.0.apothem()
    }

    pub fn sagitta(&self) -> f32 {
        self.0.sagitta()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyCircularSectorMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "CircularSector(radius={}, angle={})",
            self.0.radius(),
            self.0.angle()
        )
    }
}
