use bevy::math::{Dir2, StableInterpolate, Vec2};
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::{rot2::PyRot2, vec2::PyVec2};

#[pyclass(name = "Dir2", eq, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyDir2 {
    storage: ValueStorage<Dir2>,
}

impl PartialEq for PyDir2 {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(left), Ok(right)) if *left == *right)
    }
}

impl TryFrom<PyDir2> for Dir2 {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_dir: PyDir2) -> PyResult<Self> {
        py_dir.get()
    }
}

impl TryFrom<&PyDir2> for Dir2 {
    type Error = PyErr;

    fn try_from(py_dir: &PyDir2) -> Result<Self, Self::Error> {
        py_dir.get()
    }
}

impl From<Dir2> for PyDir2 {
    #[inline(always)]
    fn from(dir: Dir2) -> Self {
        PyDir2::dir2(dir)
    }
}

impl FromBorrowedStorage<ValueStorage<Dir2>> for PyDir2 {
    fn from_borrowed(storage: ValueStorage<Dir2>) -> Self {
        Self { storage }
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
    pub fn into_dir2(self) -> PyResult<Dir2> {
        self.get()
    }

    #[inline(always)]
    pub const fn dir2(dir: Dir2) -> Self {
        Self {
            storage: ValueStorage::owned(dir),
        }
    }

    #[inline(always)]
    pub fn get(&self) -> PyResult<Dir2> {
        Ok(*self.as_ref()?)
    }

    fn as_ref(&self) -> PyResult<StorageRef<'_, Dir2>> {
        Ok(self.storage.as_ref()?)
    }

    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Dir2>> {
        Ok(self.storage.as_mut()?)
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
        Dir2::new(vec.try_get()?)
            .map(PyDir2::dir2)
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

    pub fn as_vec2(&self) -> PyResult<PyVec2> {
        Ok(self.get()?.into())
    }

    pub fn dot(&self, other: &PyDir2) -> PyResult<f32> {
        Ok(self.get()?.dot(other.get()?.into()))
    }

    pub fn perp(&self) -> PyResult<PyDir2> {
        Ok(PyDir2::dir2(Dir2::new_unchecked(self.get()?.perp())))
    }

    pub fn slerp(&self, rhs: &PyDir2, s: f32) -> PyResult<PyDir2> {
        Ok(PyDir2::dir2(self.get()?.slerp(rhs.get()?, s)))
    }

    pub fn rotation_to(&self, other: &PyDir2) -> PyResult<PyRot2> {
        Ok(PyRot2::from(self.get()?.rotation_to(other.get()?)))
    }

    pub fn rotation_from(&self, other: &PyDir2) -> PyResult<PyRot2> {
        Ok(PyRot2::from(self.get()?.rotation_from(other.get()?)))
    }

    pub fn rotation_from_x(&self) -> PyResult<PyRot2> {
        Ok(PyRot2::from(self.get()?.rotation_from_x()))
    }

    pub fn rotation_to_x(&self) -> PyResult<PyRot2> {
        Ok(PyRot2::from(self.get()?.rotation_to_x()))
    }

    pub fn rotation_from_y(&self) -> PyResult<PyRot2> {
        Ok(PyRot2::from(self.get()?.rotation_from_y()))
    }

    pub fn rotation_to_y(&self) -> PyResult<PyRot2> {
        Ok(PyRot2::from(self.get()?.rotation_to_y()))
    }

    pub fn fast_renormalize(&self) -> PyResult<PyDir2> {
        Ok(PyDir2::dir2(self.get()?.fast_renormalize()))
    }

    #[staticmethod]
    pub fn from_xy_unchecked(x: f32, y: f32) -> PyDir2 {
        PyDir2::dir2(Dir2::from_xy_unchecked(x, y))
    }

    #[staticmethod]
    pub fn new_unchecked(value: PyVec2) -> PyResult<PyDir2> {
        Ok(PyDir2::dir2(Dir2::new_unchecked(value.try_into()?)))
    }

    pub fn interpolate_stable(&self, other: &PyDir2, t: f32) -> PyResult<PyDir2> {
        let other = other.get()?;
        Ok(PyDir2::dir2(self.get()?.interpolate_stable(&other, t)))
    }

    pub fn interpolate_stable_assign(&mut self, other: &PyDir2, t: f32) -> PyResult<()> {
        let other = other.get()?;
        self.as_mut()?.interpolate_stable_assign(&other, t);
        Ok(())
    }

    pub fn smooth_nudge(&mut self, target: &PyDir2, decay_rate: f32, delta: f32) -> PyResult<()> {
        let target = target.get()?;
        self.as_mut()?.smooth_nudge(&target, decay_rate, delta);
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let value = self.as_ref()?;
        Ok(format!("Dir2({}, {})", value.x, value.y))
    }

    pub fn as_tuple(&self) -> PyResult<(f32, f32)> {
        let value = self.as_ref()?;
        Ok((value.x, value.y))
    }

    pub fn __neg__(&self) -> PyResult<PyDir2> {
        Ok(PyDir2::dir2(-self.get()?))
    }

    pub fn __mul__(&self, scalar: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.get()? * scalar))
    }

    pub fn __rmul__(&self, scalar: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(scalar * self.get()?))
    }
}
