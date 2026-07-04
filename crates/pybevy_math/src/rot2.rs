use std::f32::consts::PI;

use bevy::math::{Rot2, StableInterpolate};
use pyo3::prelude::*;

use crate::vec2::PyVec2;

#[pyclass(name = "Rot2", eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyRot2(pub(crate) Rot2);

impl From<Rot2> for PyRot2 {
    fn from(value: Rot2) -> Self {
        PyRot2(value)
    }
}

impl From<PyRot2> for Rot2 {
    fn from(value: PyRot2) -> Self {
        value.0
    }
}

impl PyRot2 {
    pub(crate) fn inner(&self) -> Rot2 {
        self.0
    }
}

#[pymethods]
impl PyRot2 {
    #[new]
    #[pyo3(signature = (*, cos = None, sin = None))]
    pub fn new(cos: Option<f32>, sin: Option<f32>) -> Self {
        if let (Some(c), Some(s)) = (cos, sin) {
            return PyRot2(Rot2::from_sin_cos(s, c));
        }
        PyRot2(Rot2::IDENTITY)
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    pub fn identity() -> Self {
        PyRot2(Rot2::IDENTITY)
    }

    #[staticmethod]
    #[pyo3(name = "PI")]
    pub fn pi() -> Self {
        PyRot2(Rot2::from_sin_cos(0.0, -1.0))
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_2")]
    pub fn frac_pi_2() -> Self {
        PyRot2(Rot2::radians(PI / 2.0))
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_3")]
    pub fn frac_pi_3() -> Self {
        PyRot2(Rot2::radians(PI / 3.0))
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_4")]
    pub fn frac_pi_4() -> Self {
        PyRot2(Rot2::radians(PI / 4.0))
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_6")]
    pub fn frac_pi_6() -> Self {
        PyRot2(Rot2::radians(PI / 6.0))
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_8")]
    pub fn frac_pi_8() -> Self {
        PyRot2(Rot2::radians(PI / 8.0))
    }

    #[staticmethod]
    pub fn radians(radians: f32) -> Self {
        PyRot2(Rot2::radians(radians))
    }

    #[staticmethod]
    pub fn degrees(degrees: f32) -> Self {
        PyRot2(Rot2::degrees(degrees))
    }

    #[staticmethod]
    pub fn turn_fraction(fraction: f32) -> Self {
        PyRot2(Rot2::turn_fraction(fraction))
    }

    #[staticmethod]
    pub fn from_sin_cos(sin: f32, cos: f32) -> Self {
        PyRot2(Rot2::from_sin_cos(sin, cos))
    }

    #[getter]
    pub fn cos(&self) -> f32 {
        self.0.cos
    }

    #[getter]
    pub fn sin(&self) -> f32 {
        self.0.sin
    }

    pub fn as_radians(&self) -> f32 {
        self.0.as_radians()
    }

    pub fn as_degrees(&self) -> f32 {
        self.0.as_degrees()
    }

    pub fn as_turn_fraction(&self) -> f32 {
        self.0.as_turn_fraction()
    }

    pub fn inverse(&self) -> Self {
        PyRot2(self.0.inverse())
    }

    pub fn sin_cos(&self) -> (f32, f32) {
        self.0.sin_cos()
    }

    pub fn length(&self) -> f32 {
        self.0.length()
    }

    pub fn length_squared(&self) -> f32 {
        self.0.length_squared()
    }

    pub fn length_recip(&self) -> f32 {
        self.0.length_recip()
    }

    pub fn try_normalize(&self) -> Option<Self> {
        self.0.try_normalize().map(PyRot2)
    }

    pub fn is_normalized(&self) -> bool {
        self.0.is_normalized()
    }

    pub fn normalize(&self) -> Self {
        PyRot2(self.0.normalize())
    }

    pub fn fast_renormalize(&self) -> Self {
        PyRot2(self.0.fast_renormalize())
    }

    pub fn is_near_identity(&self) -> bool {
        self.0.is_near_identity()
    }

    pub fn is_finite(&self) -> bool {
        self.0.is_finite()
    }

    pub fn is_nan(&self) -> bool {
        self.0.is_nan()
    }

    pub fn angle_to(&self, other: PyRot2) -> f32 {
        self.0.angle_to(other.0)
    }

    pub fn rotate(&self, vec: PyVec2) -> PyVec2 {
        (self.0 * bevy::math::Vec2::from(vec)).into()
    }

    pub fn slerp(&self, rhs: PyRot2, s: f32) -> Self {
        PyRot2(self.0.slerp(rhs.0, s))
    }

    pub fn nlerp(&self, rhs: PyRot2, s: f32) -> Self {
        PyRot2(self.0.nlerp(rhs.0, s))
    }

    pub fn interpolate_stable(&self, other: PyRot2, t: f32) -> Self {
        PyRot2(self.0.interpolate_stable(&other.0, t))
    }

    pub fn interpolate_stable_assign(&mut self, other: PyRot2, t: f32) {
        self.0.interpolate_stable_assign(&other.0, t);
    }

    pub fn smooth_nudge(&mut self, target: PyRot2, decay_rate: f32, delta: f32) {
        self.0.smooth_nudge(&target.0, decay_rate, delta);
    }

    pub fn __mul__(&self, other: PyRot2) -> Self {
        PyRot2(self.0 * other.0)
    }

    pub fn __repr__(&self) -> String {
        format!("Rot2(radians={})", self.0.as_radians())
    }
}
