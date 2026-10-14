use bevy::{
    curve::{Curve, cubic_splines::RationalCurve},
    math::Vec2,
};
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use super::rational_segment2d::PyRationalSegment2d;
use crate::interval::PyInterval;

#[pyclass(
    name = "RationalCurve2d",
    module = "pybevy.curve",
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

    pub fn segments(&self) -> Vec<PyRationalSegment2d> {
        self.curve
            .segments()
            .iter()
            .copied()
            .map(PyRationalSegment2d::from)
            .collect()
    }

    pub fn domain(&self) -> PyInterval {
        self.curve.domain().into()
    }

    pub fn sample(&self, t: f32) -> Option<PyVec2> {
        self.curve.sample(t).map(PyVec2::from_vec2)
    }

    pub fn sample_clamped(&self, t: f32) -> PyVec2 {
        PyVec2::from_vec2(self.curve.sample_clamped(t))
    }

    fn __repr__(&self) -> String {
        format!("RationalCurve2d({} segments)", self.curve.segments().len())
    }
}
