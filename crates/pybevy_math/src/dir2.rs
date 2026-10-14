use bevy::math::{Dir2, StableInterpolate, Vec2};
use pybevy_core::{
    FromBorrowedStorage, StorageRef, ValueStorage,
    domain::{FINITE, FINITE_NON_NEGATIVE, NON_NEGATIVE, check_for},
    repr::float_repr,
};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use super::{rot2::PyRot2, vec2::PyVec2};
use crate::check_direction_length;

#[pyclass(name = "Dir2", module = "pybevy.math", eq, from_py_object)]
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
}

#[pymethods]
impl PyDir2 {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Dir2::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[staticmethod]
    #[pyo3(name = "X")]
    fn const_x() -> Self {
        PyDir2::dir2(Dir2::X)
    }

    #[staticmethod]
    #[pyo3(name = "Y")]
    fn const_y() -> Self {
        PyDir2::dir2(Dir2::Y)
    }

    #[staticmethod]
    #[pyo3(name = "NEG_X")]
    fn const_neg_x() -> Self {
        PyDir2::dir2(Dir2::NEG_X)
    }

    #[staticmethod]
    #[pyo3(name = "NEG_Y")]
    fn const_neg_y() -> Self {
        PyDir2::dir2(Dir2::NEG_Y)
    }

    #[staticmethod]
    #[pyo3(name = "NORTH")]
    fn const_north() -> Self {
        PyDir2::dir2(Dir2::NORTH)
    }

    #[staticmethod]
    #[pyo3(name = "SOUTH")]
    fn const_south() -> Self {
        PyDir2::dir2(Dir2::SOUTH)
    }

    #[staticmethod]
    #[pyo3(name = "EAST")]
    fn const_east() -> Self {
        PyDir2::dir2(Dir2::EAST)
    }

    #[staticmethod]
    #[pyo3(name = "WEST")]
    fn const_west() -> Self {
        PyDir2::dir2(Dir2::WEST)
    }

    #[staticmethod]
    #[pyo3(name = "NORTH_EAST")]
    fn const_north_east() -> Self {
        PyDir2::dir2(Dir2::NORTH_EAST)
    }

    #[staticmethod]
    #[pyo3(name = "NORTH_WEST")]
    fn const_north_west() -> Self {
        PyDir2::dir2(Dir2::NORTH_WEST)
    }

    #[staticmethod]
    #[pyo3(name = "SOUTH_EAST")]
    fn const_south_east() -> Self {
        PyDir2::dir2(Dir2::SOUTH_EAST)
    }

    #[staticmethod]
    #[pyo3(name = "SOUTH_WEST")]
    fn const_south_west() -> Self {
        PyDir2::dir2(Dir2::SOUTH_WEST)
    }

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
        let s = check_for::<Self, _>("s", s, FINITE)?;
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
    pub fn from_xy_unchecked(x: f32, y: f32) -> PyResult<PyDir2> {
        check_direction_length("Dir2.from_xy_unchecked", Vec2::new(x, y).length_squared())?;
        Ok(PyDir2::dir2(Dir2::from_xy_unchecked(x, y)))
    }

    #[staticmethod]
    pub fn new_unchecked(value: PyVec2) -> PyResult<PyDir2> {
        let value: Vec2 = value.try_into()?;
        check_direction_length("Dir2.new_unchecked", value.length_squared())?;
        Ok(PyDir2::dir2(Dir2::new_unchecked(value)))
    }

    pub fn interpolate_stable(&self, other: &PyDir2, t: f32) -> PyResult<PyDir2> {
        let t = check_for::<Self, _>("t", t, FINITE)?;
        let other = other.get()?;
        Ok(PyDir2::dir2(self.get()?.interpolate_stable(&other, t)))
    }

    pub fn interpolate_stable_assign(&mut self, other: &PyDir2, t: f32) -> PyResult<()> {
        let t = check_for::<Self, _>("t", t, FINITE)?;
        let other = other.get()?;
        self.storage
            .update(|dir| dir.interpolate_stable_assign(&other, t))?;
        Ok(())
    }

    pub fn smooth_nudge(&mut self, target: &PyDir2, decay_rate: f32, delta: f32) -> PyResult<()> {
        let decay_rate = check_for::<Self, _>("decay_rate", decay_rate, NON_NEGATIVE)?;
        let delta = check_for::<Self, _>("delta", delta, FINITE_NON_NEGATIVE)?;
        let target = target.get()?;
        // No time passes: Bevy's result, without an infinite rate times zero.
        if delta == 0.0 {
            return Ok(());
        }
        self.storage
            .update(|dir| dir.smooth_nudge(&target, decay_rate, delta))?;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let value = self.as_ref()?;
        Ok(format!(
            "Dir2({}, {})",
            float_repr(value.x),
            float_repr(value.y)
        ))
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

    #[staticmethod]
    pub fn from_xy(x: f32, y: f32) -> PyResult<PyDir2> {
        Dir2::from_xy(x, y)
            .map(PyDir2::dir2)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[staticmethod]
    pub fn new_and_length(value: &PyVec2) -> PyResult<(PyDir2, f32)> {
        Dir2::new_and_length(value.try_get()?)
            .map(|(direction, length)| (PyDir2::dir2(direction), length))
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }
}

/// Accept a Dir2 or a Vec2 that normalizes to one.
pub fn extract_dir2_from_any(obj: &Bound<'_, PyAny>) -> PyResult<Dir2> {
    if let Ok(direction) = obj.cast::<PyDir2>() {
        return direction.try_borrow()?.get();
    }
    if let Ok(vector) = obj.cast::<PyVec2>() {
        return Dir2::new(vector.try_borrow()?.try_get()?)
            .map_err(|e| PyValueError::new_err(e.to_string()));
    }
    Err(PyTypeError::new_err(format!(
        "expected Dir2 or Vec2, got {}",
        obj.get_type().name()?
    )))
}
