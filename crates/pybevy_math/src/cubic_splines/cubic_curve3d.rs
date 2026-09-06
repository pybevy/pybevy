use bevy::math::{Vec3, cubic_splines::CubicCurve};
use pyo3::prelude::*;

use crate::vec3::PyVec3;

#[pyclass(
    name = "CubicCurve3d",
    module = "pybevy.math",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyCubicCurve3d {
    curve: CubicCurve<Vec3>,
}

impl PyCubicCurve3d {
    pub fn from_curve(curve: CubicCurve<Vec3>) -> Self {
        PyCubicCurve3d { curve }
    }
}

#[pymethods]
impl PyCubicCurve3d {
    pub fn position(&self, t: f32) -> PyVec3 {
        PyVec3::from_vec3(self.curve.position(t))
    }

    pub fn velocity(&self, t: f32) -> PyVec3 {
        PyVec3::from_vec3(self.curve.velocity(t))
    }

    pub fn acceleration(&self, t: f32) -> PyVec3 {
        PyVec3::from_vec3(self.curve.acceleration(t))
    }

    pub fn iter_positions(&self, subdivisions: usize) -> Vec<PyVec3> {
        self.curve
            .iter_positions(subdivisions)
            .map(PyVec3::from_vec3)
            .collect()
    }

    pub fn iter_velocities(&self, subdivisions: usize) -> Vec<PyVec3> {
        self.curve
            .iter_velocities(subdivisions)
            .map(PyVec3::from_vec3)
            .collect()
    }

    pub fn iter_accelerations(&self, subdivisions: usize) -> Vec<PyVec3> {
        self.curve
            .iter_accelerations(subdivisions)
            .map(PyVec3::from_vec3)
            .collect()
    }

    #[getter]
    pub fn segment_count(&self) -> usize {
        self.curve.segments().len()
    }

    fn __repr__(&self) -> String {
        format!("CubicCurve3d({} segments)", self.segment_count())
    }
}
