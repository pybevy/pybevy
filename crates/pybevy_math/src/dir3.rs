use bevy::math::{Dir3, StableInterpolate, Vec3};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use crate::{quat::PyQuat, vec3::PyVec3};

#[pyclass(name = "Dir3", eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyDir3(pub(crate) Dir3);

impl From<PyDir3> for Dir3 {
    #[inline(always)]
    fn from(py_dir: PyDir3) -> Self {
        py_dir.0
    }
}

impl TryFrom<&PyDir3> for Dir3 {
    type Error = PyErr;

    fn try_from(py_dir: &PyDir3) -> Result<Self, Self::Error> {
        Ok(py_dir.0)
    }
}

impl From<Dir3> for PyDir3 {
    #[inline(always)]
    fn from(dir: Dir3) -> Self {
        PyDir3::dir3(dir)
    }
}

impl From<Dir3> for PyVec3 {
    #[inline(always)]
    fn from(dir: Dir3) -> Self {
        PyVec3::from_vec3(dir.into())
    }
}

impl PyDir3 {
    #[inline(always)]
    pub fn from_dir3(dir: Dir3) -> Self {
        PyDir3::dir3(dir)
    }

    #[inline(always)]
    pub fn into_dir3(self) -> Dir3 {
        self.0
    }

    #[inline(always)]
    pub const fn dir3(dir: Dir3) -> Self {
        PyDir3(dir)
    }

    #[inline(always)]
    pub fn get(&self) -> Dir3 {
        self.0
    }
}

#[pymethods]
impl PyDir3 {
    #[classattr]
    pub const X: PyDir3 = PyDir3::dir3(Dir3::X);

    #[classattr]
    pub const Y: PyDir3 = PyDir3::dir3(Dir3::Y);

    #[classattr]
    pub const Z: PyDir3 = PyDir3::dir3(Dir3::Z);

    #[classattr]
    pub const NEG_X: PyDir3 = PyDir3::dir3(Dir3::NEG_X);

    #[classattr]
    pub const NEG_Y: PyDir3 = PyDir3::dir3(Dir3::NEG_Y);

    #[classattr]
    pub const NEG_Z: PyDir3 = PyDir3::dir3(Dir3::NEG_Z);

    #[new]
    pub fn new(x: f32, y: f32, z: f32) -> PyResult<Self> {
        Dir3::new(Vec3::new(x, y, z))
            .map(PyDir3::dir3)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[staticmethod]
    pub fn from_vec3(vec: &PyVec3) -> PyResult<Self> {
        Dir3::new(vec.get())
            .map(PyDir3::dir3)
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

    #[getter]
    pub fn z(&self) -> f32 {
        self.0.z
    }

    pub fn as_vec3(&self) -> PyVec3 {
        self.0.into()
    }

    pub fn dot(&self, other: &PyDir3) -> f32 {
        self.0.dot(other.0.into())
    }

    pub fn cross(&self, other: &PyDir3) -> PyDir3 {
        PyDir3::dir3(Dir3::new_unchecked(
            self.0.cross(other.0.into()).normalize(),
        ))
    }

    pub fn slerp(&self, rhs: &PyDir3, s: f32) -> PyDir3 {
        PyDir3::dir3(self.0.slerp(rhs.0, s))
    }

    pub fn fast_renormalize(&self) -> PyDir3 {
        PyDir3::dir3(self.0.fast_renormalize())
    }

    #[staticmethod]
    pub fn from_xyz_unchecked(x: f32, y: f32, z: f32) -> PyDir3 {
        PyDir3::dir3(Dir3::from_xyz_unchecked(x, y, z))
    }

    #[staticmethod]
    pub fn new_unchecked(value: PyVec3) -> PyDir3 {
        PyDir3::dir3(Dir3::new_unchecked(value.into()))
    }

    pub fn interpolate_stable(&self, other: PyDir3, t: f32) -> PyDir3 {
        PyDir3::dir3(self.0.interpolate_stable(&other.0, t))
    }

    pub fn interpolate_stable_assign(&mut self, other: PyDir3, t: f32) {
        self.0.interpolate_stable_assign(&other.0, t);
    }

    pub fn smooth_nudge(&mut self, target: PyDir3, decay_rate: f32, delta: f32) {
        self.0.smooth_nudge(&target.0, decay_rate, delta);
    }

    pub fn __repr__(&self) -> String {
        format!("Dir3({}, {}, {})", self.0.x, self.0.y, self.0.z)
    }

    pub fn as_tuple(&self) -> (f32, f32, f32) {
        (self.0.x, self.0.y, self.0.z)
    }

    pub fn __neg__(&self) -> PyDir3 {
        PyDir3::dir3(-self.0)
    }

    pub fn __mul__(&self, py: Python, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec3::from_vec3(self.0 * scalar))?.into_any())
        } else if other.extract::<PyQuat>().is_ok() {
            // Dir3 * Quat is not standard, suggest Quat * Dir3 instead
            Err(PyTypeError::new_err(
                "Dir3 * Quat is not supported. Use Quat * Dir3 to rotate a direction.",
            ))
        } else {
            Err(PyTypeError::new_err("Dir3 can only be multiplied by float"))
        }
    }

    pub fn __rmul__(&self, py: Python, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec3::from_vec3(scalar * self.0))?.into_any())
        } else if let Ok(quat) = other.extract::<PyQuat>() {
            // Quat * Dir3 -> Dir3 (rotation)
            Ok(Py::new(py, PyDir3::dir3(quat.get() * self.0))?.into_any())
        } else {
            Err(PyTypeError::new_err(
                "Dir3 can only be multiplied by float or Quat",
            ))
        }
    }
}
