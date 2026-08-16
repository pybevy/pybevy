use std::f32::consts::PI;

use bevy::math::{Rot2, StableInterpolate, Vec2};
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pyo3::prelude::*;

use crate::vec2::PyVec2;

#[pyclass(name = "Rot2", eq, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyRot2 {
    storage: ValueStorage<Rot2>,
}

impl PartialEq for PyRot2 {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(left), Ok(right)) if *left == *right)
    }
}

impl From<Rot2> for PyRot2 {
    fn from(value: Rot2) -> Self {
        Self::rot2(value)
    }
}

impl TryFrom<PyRot2> for Rot2 {
    type Error = PyErr;

    fn try_from(value: PyRot2) -> PyResult<Self> {
        Ok(*value.as_ref()?)
    }
}

impl TryFrom<&PyRot2> for Rot2 {
    type Error = PyErr;

    fn try_from(value: &PyRot2) -> PyResult<Self> {
        Ok(*value.as_ref()?)
    }
}

impl FromBorrowedStorage<ValueStorage<Rot2>> for PyRot2 {
    fn from_borrowed(storage: ValueStorage<Rot2>) -> Self {
        Self { storage }
    }
}

impl PyRot2 {
    pub(crate) fn inner(&self) -> PyResult<Rot2> {
        Ok(*self.as_ref()?)
    }

    pub const fn rot2(value: Rot2) -> Self {
        Self {
            storage: ValueStorage::owned(value),
        }
    }

    fn as_ref(&self) -> PyResult<StorageRef<'_, Rot2>> {
        Ok(self.storage.as_ref()?)
    }

    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Rot2>> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyRot2 {
    #[new]
    #[pyo3(signature = (*, cos = None, sin = None))]
    pub fn new(cos: Option<f32>, sin: Option<f32>) -> Self {
        if let (Some(c), Some(s)) = (cos, sin) {
            return Self::rot2(Rot2::from_sin_cos(s, c));
        }
        Self::rot2(Rot2::IDENTITY)
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    pub fn identity() -> Self {
        Self::rot2(Rot2::IDENTITY)
    }

    #[staticmethod]
    #[pyo3(name = "PI")]
    pub fn pi() -> Self {
        Self::rot2(Rot2::from_sin_cos(0.0, -1.0))
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_2")]
    pub fn frac_pi_2() -> Self {
        Self::rot2(Rot2::radians(PI / 2.0))
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_3")]
    pub fn frac_pi_3() -> Self {
        Self::rot2(Rot2::radians(PI / 3.0))
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_4")]
    pub fn frac_pi_4() -> Self {
        Self::rot2(Rot2::radians(PI / 4.0))
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_6")]
    pub fn frac_pi_6() -> Self {
        Self::rot2(Rot2::radians(PI / 6.0))
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_8")]
    pub fn frac_pi_8() -> Self {
        Self::rot2(Rot2::radians(PI / 8.0))
    }

    #[staticmethod]
    pub fn radians(radians: f32) -> Self {
        Self::rot2(Rot2::radians(radians))
    }

    #[staticmethod]
    pub fn degrees(degrees: f32) -> Self {
        Self::rot2(Rot2::degrees(degrees))
    }

    #[staticmethod]
    pub fn turn_fraction(fraction: f32) -> Self {
        Self::rot2(Rot2::turn_fraction(fraction))
    }

    #[staticmethod]
    pub fn from_sin_cos(sin: f32, cos: f32) -> Self {
        Self::rot2(Rot2::from_sin_cos(sin, cos))
    }

    #[getter]
    pub fn cos(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.cos)
    }

    #[getter]
    pub fn sin(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.sin)
    }

    pub fn as_radians(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.as_radians())
    }

    pub fn as_degrees(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.as_degrees())
    }

    pub fn as_turn_fraction(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.as_turn_fraction())
    }

    pub fn inverse(&self) -> PyResult<Self> {
        Ok(Self::rot2(self.as_ref()?.inverse()))
    }

    pub fn sin_cos(&self) -> PyResult<(f32, f32)> {
        Ok(self.as_ref()?.sin_cos())
    }

    pub fn length(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length())
    }

    pub fn length_squared(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length_squared())
    }

    pub fn length_recip(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length_recip())
    }

    pub fn try_normalize(&self) -> PyResult<Option<Self>> {
        Ok(self.as_ref()?.try_normalize().map(Self::rot2))
    }

    pub fn is_normalized(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_normalized())
    }

    pub fn normalize(&self) -> PyResult<Self> {
        Ok(Self::rot2(self.as_ref()?.normalize()))
    }

    pub fn fast_renormalize(&self) -> PyResult<Self> {
        Ok(Self::rot2(self.as_ref()?.fast_renormalize()))
    }

    pub fn is_near_identity(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_near_identity())
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    pub fn angle_to(&self, other: &PyRot2) -> PyResult<f32> {
        Ok(self.as_ref()?.angle_to(*other.as_ref()?))
    }

    pub fn rotate(&self, vec: PyVec2) -> PyResult<PyVec2> {
        Ok((*self.as_ref()? * Vec2::try_from(vec)?).into())
    }

    pub fn slerp(&self, rhs: &PyRot2, s: f32) -> PyResult<Self> {
        Ok(Self::rot2(self.as_ref()?.slerp(*rhs.as_ref()?, s)))
    }

    pub fn nlerp(&self, rhs: &PyRot2, s: f32) -> PyResult<Self> {
        Ok(Self::rot2(self.as_ref()?.nlerp(*rhs.as_ref()?, s)))
    }

    pub fn interpolate_stable(&self, other: &PyRot2, t: f32) -> PyResult<Self> {
        Ok(Self::rot2(
            self.as_ref()?
                .interpolate_stable(other.as_ref()?.reborrow(), t),
        ))
    }

    pub fn interpolate_stable_assign(&mut self, other: &PyRot2, t: f32) -> PyResult<()> {
        let other = *other.as_ref()?;
        self.as_mut()?.interpolate_stable_assign(&other, t);
        Ok(())
    }

    pub fn smooth_nudge(&mut self, target: &PyRot2, decay_rate: f32, delta: f32) -> PyResult<()> {
        let target = *target.as_ref()?;
        self.as_mut()?.smooth_nudge(&target, decay_rate, delta);
        Ok(())
    }

    pub fn __mul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let rotation = *self.as_ref()?;
        if let Ok(other_rotation) = other.extract::<PyRot2>() {
            Ok(Py::new(py, Self::rot2(rotation * *other_rotation.as_ref()?))?.into_any())
        } else if let Ok(vector) = other.extract::<PyVec2>() {
            Ok(Py::new(py, PyVec2::from_vec2(rotation * vector.try_get()?))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("Rot2(radians={})", self.as_ref()?.as_radians()))
    }
}
