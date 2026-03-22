use bevy::math::{Dir2, StableInterpolate, Vec2};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::{rot2::PyRot2, vec2::PyVec2};

#[pyclass(name = "Dir2", eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyDir2(pub(crate) Dir2);

impl From<PyDir2> for Dir2 {
    #[inline(always)]
    fn from(py_dir: PyDir2) -> Self {
        py_dir.0
    }
}

impl TryFrom<&PyDir2> for Dir2 {
    type Error = PyErr;

    fn try_from(py_dir: &PyDir2) -> Result<Self, Self::Error> {
        Ok(py_dir.0)
    }
}

impl From<Dir2> for PyDir2 {
    #[inline(always)]
    fn from(dir: Dir2) -> Self {
        PyDir2::dir2(dir)
    }
}

impl From<Dir2> for PyVec2 {
    #[inline(always)]
    fn from(dir: Dir2) -> Self {
        PyVec2::from_vec2(dir.into())
    }
}

impl PyDir2 {
    #[inline(always)]
    pub fn from_dir2(dir: Dir2) -> Self {
        PyDir2::dir2(dir)
    }

    #[inline(always)]
    pub fn into_dir2(self) -> Dir2 {
        self.0
    }

    #[inline(always)]
    pub const fn dir2(dir: Dir2) -> Self {
        PyDir2(dir)
    }

    #[inline(always)]
    pub fn get(&self) -> Dir2 {
        self.0
    }
}

#[pymethods]
impl PyDir2 {
    #[classattr]
    pub const X: PyDir2 = PyDir2::dir2(Dir2::X);

    #[classattr]
    pub const Y: PyDir2 = PyDir2::dir2(Dir2::Y);

    #[classattr]
    pub const NEG_X: PyDir2 = PyDir2::dir2(Dir2::NEG_X);

    #[classattr]
    pub const NEG_Y: PyDir2 = PyDir2::dir2(Dir2::NEG_Y);

    #[classattr]
    pub const NORTH: PyDir2 = PyDir2::dir2(Dir2::NORTH);

    #[classattr]
    pub const SOUTH: PyDir2 = PyDir2::dir2(Dir2::SOUTH);

    #[classattr]
    pub const EAST: PyDir2 = PyDir2::dir2(Dir2::EAST);

    #[classattr]
    pub const WEST: PyDir2 = PyDir2::dir2(Dir2::WEST);

    #[classattr]
    pub const NORTH_EAST: PyDir2 = PyDir2::dir2(Dir2::NORTH_EAST);

    #[classattr]
    pub const NORTH_WEST: PyDir2 = PyDir2::dir2(Dir2::NORTH_WEST);

    #[classattr]
    pub const SOUTH_EAST: PyDir2 = PyDir2::dir2(Dir2::SOUTH_EAST);

    #[classattr]
    pub const SOUTH_WEST: PyDir2 = PyDir2::dir2(Dir2::SOUTH_WEST);

    #[new]
    pub fn new(x: f32, y: f32) -> PyResult<Self> {
        Dir2::new(Vec2::new(x, y))
            .map(PyDir2::dir2)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[staticmethod]
    pub fn from_vec2(vec: &PyVec2) -> PyResult<Self> {
        Dir2::new(vec.get())
            .map(PyDir2::dir2)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[getter]
    pub fn x(&self) -> f32 {
        self.0.x
    }

    #[getter]
    pub fn y(&self) -> f32 {
        self.0.y
    }

    pub fn as_vec2(&self) -> PyVec2 {
        self.0.into()
    }

    pub fn dot(&self, other: &PyDir2) -> f32 {
        self.0.dot(other.0.into())
    }

    pub fn perp(&self) -> PyDir2 {
        PyDir2::dir2(Dir2::new_unchecked(self.0.perp()))
    }

    pub fn slerp(&self, rhs: &PyDir2, s: f32) -> PyDir2 {
        PyDir2::dir2(self.0.slerp(rhs.0, s))
    }

    pub fn rotation_to(&self, other: &PyDir2) -> PyRot2 {
        PyRot2::from(self.0.rotation_to(other.0))
    }

    pub fn rotation_from(&self, other: &PyDir2) -> PyRot2 {
        PyRot2::from(self.0.rotation_from(other.0))
    }

    pub fn rotation_from_x(&self) -> PyRot2 {
        PyRot2::from(self.0.rotation_from_x())
    }

    pub fn rotation_to_x(&self) -> PyRot2 {
        PyRot2::from(self.0.rotation_to_x())
    }

    pub fn rotation_from_y(&self) -> PyRot2 {
        PyRot2::from(self.0.rotation_from_y())
    }

    pub fn rotation_to_y(&self) -> PyRot2 {
        PyRot2::from(self.0.rotation_to_y())
    }

    pub fn fast_renormalize(&self) -> PyDir2 {
        PyDir2::dir2(self.0.fast_renormalize())
    }

    #[staticmethod]
    pub fn from_xy_unchecked(x: f32, y: f32) -> PyDir2 {
        PyDir2::dir2(Dir2::from_xy_unchecked(x, y))
    }

    #[staticmethod]
    pub fn new_unchecked(value: PyVec2) -> PyDir2 {
        PyDir2::dir2(Dir2::new_unchecked(value.into()))
    }

    pub fn interpolate_stable(&self, other: PyDir2, t: f32) -> PyDir2 {
        PyDir2::dir2(self.0.interpolate_stable(&other.0, t))
    }

    pub fn interpolate_stable_assign(&mut self, other: PyDir2, t: f32) {
        self.0.interpolate_stable_assign(&other.0, t);
    }

    pub fn smooth_nudge(&mut self, target: PyDir2, decay_rate: f32, delta: f32) {
        self.0.smooth_nudge(&target.0, decay_rate, delta);
    }

    pub fn __repr__(&self) -> String {
        format!("Dir2({}, {})", self.0.x, self.0.y)
    }

    pub fn as_tuple(&self) -> (f32, f32) {
        (self.0.x, self.0.y)
    }

    pub fn __neg__(&self) -> PyDir2 {
        PyDir2::dir2(-self.0)
    }

    pub fn __mul__(&self, scalar: f32) -> PyVec2 {
        PyVec2::from_vec2(self.0 * scalar)
    }

    pub fn __rmul__(&self, scalar: f32) -> PyVec2 {
        PyVec2::from_vec2(scalar * self.0)
    }
}
