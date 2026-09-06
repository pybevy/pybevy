use bevy::{
    math::primitives::{CircularSegment, Measured2d},
    mesh::Meshable,
};
use pybevy_math::{primitives::PyArc2d, vec2::PyVec2};
use pyo3::prelude::*;

use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyCircularSegmentMeshBuilder,
};

#[pyclass(name = "CircularSegment", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub struct PyCircularSegment(pub(crate) CircularSegment);

impl From<PyCircularSegment> for CircularSegment {
    fn from(py: PyCircularSegment) -> Self {
        py.0
    }
}

impl From<CircularSegment> for PyCircularSegment {
    fn from(segment: CircularSegment) -> Self {
        PyCircularSegment(segment)
    }
}

#[pymethods]
impl PyCircularSegment {
    #[new]
    #[pyo3(signature = (radius = 0.5, half_angle = 2.0 * std::f32::consts::FRAC_PI_3, *, arc = None))]
    pub fn new(radius: f32, half_angle: f32, arc: Option<PyArc2d>) -> PyClassInitializer<Self> {
        if let Some(a) = arc {
            return (Self(CircularSegment { arc: a.into() }), PyMeshable).into();
        }
        (Self(CircularSegment::new(radius, half_angle)), PyMeshable).into()
    }

    #[staticmethod]
    pub fn from_radians(py: Python<'_>, radius: f32, angle: f32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self(CircularSegment::from_radians(radius, angle)),
                PyMeshable,
            ),
        )
    }

    #[staticmethod]
    pub fn from_degrees(py: Python<'_>, radius: f32, angle: f32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self(CircularSegment::from_degrees(radius, angle)),
                PyMeshable,
            ),
        )
    }

    #[staticmethod]
    pub fn from_turns(py: Python<'_>, radius: f32, fraction: f32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self(CircularSegment::from_turns(radius, fraction)),
                PyMeshable,
            ),
        )
    }

    #[getter]
    pub fn arc(&self) -> PyArc2d {
        self.0.arc.into()
    }

    #[setter]
    pub fn set_arc(&mut self, value: PyArc2d) {
        self.0.arc = value.into();
    }

    pub fn half_angle(&self) -> f32 {
        self.0.half_angle()
    }

    pub fn angle(&self) -> f32 {
        self.0.angle()
    }

    pub fn radius(&self) -> f32 {
        self.0.radius()
    }

    pub fn arc_length(&self) -> f32 {
        self.0.arc_length()
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

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyCircularSegmentMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "CircularSegment(radius={}, angle={})",
            self.0.radius(),
            self.0.angle()
        )
    }
}
