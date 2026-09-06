use bevy::math::{Vec3, cubic_splines::RationalCurve};
use pyo3::prelude::*;

use crate::vec3::PyVec3;

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

    #[getter]
    pub fn segment_count(&self) -> usize {
        self.curve.segments().len()
    }

    fn __repr__(&self) -> String {
        format!("RationalCurve3d({} segments)", self.segment_count())
    }
}
