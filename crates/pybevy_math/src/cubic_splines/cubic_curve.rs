use bevy::curve::{Curve, cubic_splines::CubicCurve};
use bevy::math::Vec2;
use pyo3::prelude::*;

use super::cubic_segment2d::PyCubicSegment2d;
use crate::{interval::PyInterval, vec2::PyVec2};

#[pyclass(
    name = "CubicCurve2d",
    module = "pybevy.math",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyCubicCurve2d {
    curve: CubicCurve<Vec2>,
}

impl PyCubicCurve2d {
    pub fn from_curve(curve: CubicCurve<Vec2>) -> Self {
        PyCubicCurve2d { curve }
    }
}

#[pymethods]
impl PyCubicCurve2d {
    pub fn position(&self, t: f32) -> PyVec2 {
        PyVec2::from_vec2(self.curve.position(t))
    }

    pub fn velocity(&self, t: f32) -> PyVec2 {
        PyVec2::from_vec2(self.curve.velocity(t))
    }

    pub fn acceleration(&self, t: f32) -> PyVec2 {
        PyVec2::from_vec2(self.curve.acceleration(t))
    }

    pub fn iter_positions(&self, subdivisions: usize) -> Vec<PyVec2> {
        self.curve
            .iter_positions(subdivisions)
            .map(PyVec2::from_vec2)
            .collect()
    }

    pub fn iter_velocities(&self, subdivisions: usize) -> Vec<PyVec2> {
        self.curve
            .iter_velocities(subdivisions)
            .map(PyVec2::from_vec2)
            .collect()
    }

    pub fn iter_accelerations(&self, subdivisions: usize) -> Vec<PyVec2> {
        self.curve
            .iter_accelerations(subdivisions)
            .map(PyVec2::from_vec2)
            .collect()
    }

    pub fn segments(&self) -> Vec<PyCubicSegment2d> {
        self.curve
            .segments()
            .iter()
            .copied()
            .map(PyCubicSegment2d::from)
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
        format!("CubicCurve2d({} segments)", self.curve.segments().len())
    }
}
