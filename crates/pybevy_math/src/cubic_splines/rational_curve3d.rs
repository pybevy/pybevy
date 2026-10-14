use bevy::curve::{Curve, cubic_splines::RationalCurve};
use bevy::math::Vec3;
use pyo3::prelude::*;

use super::rational_segment3d::PyRationalSegment3d;
use crate::{interval::PyInterval, vec3::PyVec3};

#[pyclass(
    name = "RationalCurve3d",
    module = "pybevy.math",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyRationalCurve3d {
    curve: RationalCurve<Vec3>,
}

impl PyRationalCurve3d {
    pub fn from_curve(curve: RationalCurve<Vec3>) -> Self {
        PyRationalCurve3d { curve }
    }
}

#[pymethods]
impl PyRationalCurve3d {
    pub fn position(&self, t: f32) -> PyVec3 {
        PyVec3::from_vec3(self.curve.position(t))
    }

    pub fn velocity(&self, t: f32) -> PyVec3 {
        PyVec3::from_vec3(self.curve.velocity(t))
    }

    pub fn acceleration(&self, t: f32) -> PyVec3 {
        PyVec3::from_vec3(self.curve.acceleration(t))
    }

    pub fn segments(&self) -> Vec<PyRationalSegment3d> {
        self.curve
            .segments()
            .iter()
            .copied()
            .map(PyRationalSegment3d::from)
            .collect()
    }

    pub fn domain(&self) -> PyInterval {
        self.curve.domain().into()
    }

    pub fn sample(&self, t: f32) -> Option<PyVec3> {
        self.curve.sample(t).map(PyVec3::from_vec3)
    }

    pub fn sample_clamped(&self, t: f32) -> PyVec3 {
        PyVec3::from_vec3(self.curve.sample_clamped(t))
    }

    fn __repr__(&self) -> String {
        format!("RationalCurve3d({} segments)", self.curve.segments().len())
    }
}
