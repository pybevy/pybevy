use std::f32::consts::TAU;

use bevy::math::{Rot2, StableInterpolate, Vec2};
use pybevy_core::{
    FromBorrowedStorage, StorageMut, StorageRef, ValueStorage,
    domain::{FINITE, FINITE_NON_NEGATIVE, FieldDomain, NON_NEGATIVE, check_for},
    public_error,
    repr::float_repr,
};
use pybevy_macros::pyconstructor;
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use crate::vec2::PyVec2;

/// A turn fraction whose angle, `TAU * fraction`, stays finite.
const FRACTION_DOMAIN: &[FieldDomain] = &[FieldDomain::RangeInclusive {
    lo: -(f32::MAX / TAU) as f64,
    hi: (f32::MAX / TAU) as f64,
}];

#[pyclass(name = "Rot2", module = "pybevy.math", eq, from_py_object)]
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

/// A rotation from its sine and cosine; Bevy's `from_sin_cos` panics in debug
/// builds unless they are normalized.
fn checked_rotation(operation: &str, sin: f32, cos: f32) -> PyResult<Rot2> {
    let rotation = Rot2 { cos, sin };
    if !cos.is_finite() || !sin.is_finite() || !rotation.is_normalized() {
        return Err(PyValueError::new_err(
            public_error::rotation_not_normalized(operation),
        ));
    }
    Ok(rotation)
}

#[pyconstructor("Rot2", keyword_only(cos, sin), complete(cos, sin))]
#[pymethods]
impl PyRot2 {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Rot2::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    pub fn new(cos: Option<f32>, sin: Option<f32>) -> PyResult<Self> {
        if let (Some(c), Some(s)) = (cos, sin) {
            return Ok(Self::rot2(checked_rotation("Rot2()", s, c)?));
        }
        Ok(Self::rot2(Rot2::IDENTITY))
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    pub fn identity() -> Self {
        Self::rot2(Rot2::IDENTITY)
    }

    #[staticmethod]
    #[pyo3(name = "PI")]
    pub fn pi() -> Self {
        Self::rot2(Rot2::PI)
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_2")]
    pub fn frac_pi_2() -> Self {
        Self::rot2(Rot2::FRAC_PI_2)
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_3")]
    pub fn frac_pi_3() -> Self {
        Self::rot2(Rot2::FRAC_PI_3)
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_4")]
    pub fn frac_pi_4() -> Self {
        Self::rot2(Rot2::FRAC_PI_4)
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_6")]
    pub fn frac_pi_6() -> Self {
        Self::rot2(Rot2::FRAC_PI_6)
    }

    #[staticmethod]
    #[pyo3(name = "FRAC_PI_8")]
    pub fn frac_pi_8() -> Self {
        Self::rot2(Rot2::FRAC_PI_8)
    }

    #[staticmethod]
    pub fn radians(radians: f32) -> PyResult<Self> {
        let radians = check_for::<Self, _>("radians", radians, FINITE)?;
        Ok(Self::rot2(Rot2::radians(radians)))
    }

    #[staticmethod]
    pub fn degrees(degrees: f32) -> PyResult<Self> {
        let degrees = check_for::<Self, _>("degrees", degrees, FINITE)?;
        Ok(Self::rot2(Rot2::degrees(degrees)))
    }

    #[staticmethod]
    pub fn turn_fraction(fraction: f32) -> PyResult<Self> {
        let fraction = check_for::<Self, _>("fraction", fraction, FRACTION_DOMAIN)?;
        Ok(Self::rot2(Rot2::turn_fraction(fraction)))
    }

    #[staticmethod]
    pub fn from_sin_cos(sin: f32, cos: f32) -> PyResult<Self> {
        Ok(Self::rot2(checked_rotation("Rot2.from_sin_cos", sin, cos)?))
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

    pub fn slerp(&self, rhs: &PyRot2, s: f32) -> PyResult<Self> {
        let s = check_for::<Self, _>("s", s, FINITE)?;
        Ok(Self::rot2(self.as_ref()?.slerp(*rhs.as_ref()?, s)))
    }

    pub fn nlerp(&self, rhs: &PyRot2, s: f32) -> PyResult<Self> {
        let s = check_for::<Self, _>("s", s, FINITE)?;
        Ok(Self::rot2(self.as_ref()?.nlerp(*rhs.as_ref()?, s)))
    }

    pub fn interpolate_stable(&self, other: &PyRot2, t: f32) -> PyResult<Self> {
        let t = check_for::<Self, _>("t", t, FINITE)?;
        Ok(Self::rot2(
            self.as_ref()?
                .interpolate_stable(other.as_ref()?.reborrow(), t),
        ))
    }

    pub fn interpolate_stable_assign(&mut self, other: &PyRot2, t: f32) -> PyResult<()> {
        let t = check_for::<Self, _>("t", t, FINITE)?;
        let other = *other.as_ref()?;
        self.as_mut()?.interpolate_stable_assign(&other, t);
        Ok(())
    }

    pub fn smooth_nudge(&mut self, target: &PyRot2, decay_rate: f32, delta: f32) -> PyResult<()> {
        let decay_rate = check_for::<Self, _>("decay_rate", decay_rate, NON_NEGATIVE)?;
        let delta = check_for::<Self, _>("delta", delta, FINITE_NON_NEGATIVE)?;
        let target = *target.as_ref()?;
        // No time passes: Bevy's result, without an infinite rate times zero.
        if delta == 0.0 {
            return Ok(());
        }
        self.as_mut()?.smooth_nudge(&target, decay_rate, delta);
        Ok(())
    }

    pub fn __mul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let rotation = *self.as_ref()?;
        if let Ok(other_rotation) = other.cast::<PyRot2>() {
            let other_rotation = Rot2::try_from(&*other_rotation.try_borrow()?)?;
            Ok(Py::new(py, Self::rot2(rotation * other_rotation))?.into_any())
        } else if let Ok(vector) = other.cast::<PyVec2>() {
            let vector = Vec2::try_from(&*vector.try_borrow()?)?;
            Ok(Py::new(py, PyVec2::from_vec2(rotation * vector))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let rotation = self.as_ref()?;
        Ok(format!(
            "Rot2(cos={}, sin={})",
            float_repr(rotation.cos),
            float_repr(rotation.sin)
        ))
    }
}

/// Accept a PyRot2 or a float angle in radians, matching bevy's `impl Into<Rot2>`.
pub fn extract_rot2_from_any(obj: &Bound<'_, PyAny>) -> PyResult<Rot2> {
    if let Ok(value) = obj.extract::<PyRot2>() {
        return Rot2::try_from(value);
    }
    if let Ok(radians) = obj.extract::<f32>() {
        return Ok(Rot2::from(radians));
    }
    Err(PyTypeError::new_err(format!(
        "expected Rot2 or float radians, got {}",
        obj.get_type().name()?
    )))
}
