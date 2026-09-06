use bevy::math::{Vec2, cubic_splines::RationalCurve};
use pyo3::prelude::*;

use crate::vec2::PyVec2;

#[pyclass(
    name = "RationalCurve2d",
    module = "pybevy.math",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyRationalCurve2d {
    curve: RationalCurve<Vec2>,
}

impl PyRationalCurve2d {
    pub fn from_curve(curve: RationalCurve<Vec2>) -> Self {
        PyRationalCurve2d { curve }
    }
}

#[pymethods]
impl PyRationalCurve2d {
    pub fn position(&self, t: f32) -> PyVec2 {
        PyVec2::from_vec2(self.curve.position(t))
    }

    pub fn velocity(&self, t: f32) -> PyVec2 {
        PyVec2::from_vec2(self.curve.velocity(t))
    }

    pub fn acceleration(&self, t: f32) -> PyVec2 {
        PyVec2::from_vec2(self.curve.acceleration(t))
    }

    #[getter]
    pub fn segment_count(&self) -> usize {
        self.curve.segments().len()
    }

    fn __repr__(&self) -> String {
        format!("RationalCurve2d({} segments)", self.segment_count())
    }
}
