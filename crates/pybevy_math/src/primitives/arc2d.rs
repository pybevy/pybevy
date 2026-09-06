use bevy::math::primitives::Arc2d;
use pyo3::prelude::*;

use crate::vec2::PyVec2;

#[pyclass(name = "Arc2d", module = "pybevy.math", eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyArc2d {
    pub(crate) arc: Arc2d,
}

#[pymethods]
impl PyArc2d {
    #[new]
    #[pyo3(signature = (radius = 1.0, half_angle = std::f32::consts::FRAC_PI_4))]
    pub fn new(radius: f32, half_angle: f32) -> Self {
        Self {
            arc: Arc2d::new(radius, half_angle),
        }
    }

    #[staticmethod]
    pub fn from_radians(radius: f32, angle: f32) -> Self {
        Self {
            arc: Arc2d::from_radians(radius, angle),
        }
    }

    #[staticmethod]
    pub fn from_degrees(radius: f32, angle: f32) -> Self {
        Self {
            arc: Arc2d::from_degrees(radius, angle),
        }
    }

    #[staticmethod]
    pub fn from_turns(radius: f32, fraction: f32) -> Self {
        Self {
            arc: Arc2d::from_turns(radius, fraction),
        }
    }

    #[getter]
    pub fn radius(&self) -> f32 {
        self.arc.radius
    }

    #[setter]
    pub fn set_radius(&mut self, value: f32) {
        self.arc.radius = value;
    }

    #[getter]
    pub fn half_angle(&self) -> f32 {
        self.arc.half_angle
    }

    #[setter]
    pub fn set_half_angle(&mut self, value: f32) {
        self.arc.half_angle = value;
    }

    pub fn angle(&self) -> f32 {
        self.arc.angle()
    }

    pub fn length(&self) -> f32 {
        self.arc.length()
    }

    pub fn right_endpoint(&self) -> PyVec2 {
        PyVec2::from_vec2(self.arc.right_endpoint())
    }

    pub fn left_endpoint(&self) -> PyVec2 {
        PyVec2::from_vec2(self.arc.left_endpoint())
    }

    pub fn endpoints(&self) -> [PyVec2; 2] {
        let [left, right] = self.arc.endpoints();
        [PyVec2::from_vec2(left), PyVec2::from_vec2(right)]
    }

    pub fn midpoint(&self) -> PyVec2 {
        PyVec2::from_vec2(self.arc.midpoint())
    }

    pub fn half_chord_length(&self) -> f32 {
        self.arc.half_chord_length()
    }

    pub fn chord_length(&self) -> f32 {
        self.arc.chord_length()
    }

    pub fn chord_midpoint(&self) -> PyVec2 {
        PyVec2::from_vec2(self.arc.chord_midpoint())
    }

    pub fn apothem(&self) -> f32 {
        self.arc.apothem()
    }

    pub fn sagitta(&self) -> f32 {
        self.arc.sagitta()
    }

    pub fn is_minor(&self) -> bool {
        self.arc.is_minor()
    }

    pub fn is_major(&self) -> bool {
        self.arc.is_major()
    }

    fn __repr__(&self) -> String {
        format!(
            "Arc2d(radius={}, half_angle={})",
            self.arc.radius, self.arc.half_angle
        )
    }
}

impl From<Arc2d> for PyArc2d {
    fn from(arc: Arc2d) -> Self {
        Self { arc }
    }
}

impl From<PyArc2d> for Arc2d {
    fn from(py_arc: PyArc2d) -> Self {
        py_arc.arc
    }
}
