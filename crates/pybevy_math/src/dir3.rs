use bevy::math::{Dir3, StableInterpolate, Vec3};
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use crate::{quat::PyQuat, vec3::PyVec3};

#[pyclass(name = "Dir3", module = "pybevy.math", eq, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyDir3 {
    storage: ValueStorage<Dir3>,
}

impl PartialEq for PyDir3 {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(left), Ok(right)) if *left == *right)
    }
}

impl TryFrom<PyDir3> for Dir3 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_dir: PyDir3) -> PyResult<Self> {
        py_dir.get()
    }
}

impl TryFrom<&PyDir3> for Dir3 {
    type Error = PyErr;

    fn try_from(py_dir: &PyDir3) -> Result<Self, Self::Error> {
        py_dir.get()
    }
}

impl From<Dir3> for PyDir3 {
    #[inline(always)]
    fn from(dir: Dir3) -> Self {
        PyDir3::dir3(dir)
    }
}

impl FromBorrowedStorage<ValueStorage<Dir3>> for PyDir3 {
    fn from_borrowed(storage: ValueStorage<Dir3>) -> Self {
        Self { storage }
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
    pub fn into_dir3(self) -> PyResult<Dir3> {
        self.get()
    }

    #[inline(always)]
    pub const fn dir3(dir: Dir3) -> Self {
        Self {
            storage: ValueStorage::owned(dir),
        }
    }

    #[inline(always)]
    pub fn get(&self) -> PyResult<Dir3> {
        Ok(*self.as_ref()?)
    }

    fn as_ref(&self) -> PyResult<StorageRef<'_, Dir3>> {
        Ok(self.storage.as_ref()?)
    }

    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Dir3>> {
        Ok(self.storage.as_mut()?)
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
        Dir3::new(vec.try_get()?)
            .map(PyDir3::dir3)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[getter]
    pub fn x(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.x)
    }

    #[getter]
    pub fn y(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.y)
    }

    #[getter]
    pub fn z(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.z)
    }

    pub fn as_vec3(&self) -> PyResult<PyVec3> {
        Ok(self.get()?.into())
    }

    pub fn dot(&self, other: &PyDir3) -> PyResult<f32> {
        Ok(self.get()?.dot(other.get()?.into()))
    }

    pub fn cross(&self, other: &PyDir3) -> PyResult<PyDir3> {
        Ok(PyDir3::dir3(Dir3::new_unchecked(
            self.get()?.cross(other.get()?.into()).normalize(),
        )))
    }

    pub fn slerp(&self, rhs: &PyDir3, s: f32) -> PyResult<PyDir3> {
        Ok(PyDir3::dir3(self.get()?.slerp(rhs.get()?, s)))
    }

    pub fn fast_renormalize(&self) -> PyResult<PyDir3> {
        Ok(PyDir3::dir3(self.get()?.fast_renormalize()))
    }

    #[staticmethod]
    pub fn from_xyz_unchecked(x: f32, y: f32, z: f32) -> PyDir3 {
        PyDir3::dir3(Dir3::from_xyz_unchecked(x, y, z))
    }

    #[staticmethod]
    pub fn new_unchecked(value: PyVec3) -> PyResult<PyDir3> {
        Ok(PyDir3::dir3(Dir3::new_unchecked(value.try_into()?)))
    }

    pub fn interpolate_stable(&self, other: &PyDir3, t: f32) -> PyResult<PyDir3> {
        let other = other.get()?;
        Ok(PyDir3::dir3(self.get()?.interpolate_stable(&other, t)))
    }

    pub fn interpolate_stable_assign(&mut self, other: &PyDir3, t: f32) -> PyResult<()> {
        let other = other.get()?;
        self.as_mut()?.interpolate_stable_assign(&other, t);
        Ok(())
    }

    pub fn smooth_nudge(&mut self, target: &PyDir3, decay_rate: f32, delta: f32) -> PyResult<()> {
        let target = target.get()?;
        self.as_mut()?.smooth_nudge(&target, decay_rate, delta);
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let value = self.as_ref()?;
        Ok(format!("Dir3({}, {}, {})", value.x, value.y, value.z))
    }

    pub fn as_tuple(&self) -> PyResult<(f32, f32, f32)> {
        let value = self.as_ref()?;
        Ok((value.x, value.y, value.z))
    }

    pub fn __neg__(&self) -> PyResult<PyDir3> {
        Ok(PyDir3::dir3(-self.get()?))
    }

    pub fn __mul__(&self, py: Python, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec3::from_vec3(self.get()? * scalar))?.into_any())
        } else if other.extract::<PyQuat>().is_ok() {
            // Dir3 * Quat is not standard, suggest Quat * Dir3 instead
            Err(PyTypeError::new_err(
                "Dir3 * Quat is not supported. Use Quat * Dir3 to rotate a direction.",
            ))
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __rmul__(&self, py: Python, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, PyVec3::from_vec3(scalar * self.get()?))?.into_any())
        } else if let Ok(quat) = other.extract::<PyQuat>() {
            // Quat * Dir3 -> Dir3 (rotation)
            Ok(Py::new(py, PyDir3::dir3(quat.try_get()? * self.get()?))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }
}
