use bevy::math::{
    Isometry2d, Vec2,
    bounding::{Aabb2d, BoundingCircle, BoundingVolume, IntersectsVolume},
};
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pyo3::{exceptions::PyTypeError, prelude::*};

use super::aabb2d::PyAabb2d;
use crate::{
    rot2::extract_rot2_from_any,
    vec2::{PyVec2, extract_vec2_from_any},
};

#[pyclass(name = "BoundingCircle", module = "pybevy.math", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyBoundingCircle {
    storage: ValueStorage<BoundingCircle>,
}

impl TryFrom<PyBoundingCircle> for BoundingCircle {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_circle: PyBoundingCircle) -> PyResult<Self> {
        Ok(py_circle.storage.get()?)
    }
}

impl TryFrom<&PyBoundingCircle> for BoundingCircle {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_circle: &PyBoundingCircle) -> PyResult<Self> {
        Ok(py_circle.storage.get()?)
    }
}

impl From<BoundingCircle> for PyBoundingCircle {
    #[inline(always)]
    fn from(circle: BoundingCircle) -> Self {
        PyBoundingCircle::from_bounding_circle(circle)
    }
}

impl FromBorrowedStorage<ValueStorage<BoundingCircle>> for PyBoundingCircle {
    fn from_borrowed(storage: ValueStorage<BoundingCircle>) -> Self {
        PyBoundingCircle { storage }
    }
}

impl PyBoundingCircle {
    #[inline(always)]
    pub fn from_bounding_circle(circle: BoundingCircle) -> Self {
        PyBoundingCircle {
            storage: ValueStorage::owned(circle),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, BoundingCircle>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, BoundingCircle>> {
        Ok(self.storage.as_mut()?)
    }

    pub(crate) fn to_bounding_circle(&self) -> PyResult<BoundingCircle> {
        Ok(self.storage.get()?)
    }
}

#[pymethods]
impl PyBoundingCircle {
    #[new]
    pub fn new(center: PyVec2, radius: f32) -> PyResult<Self> {
        let center_vec: Vec2 = center.try_into()?;
        Ok(PyBoundingCircle::from_bounding_circle(BoundingCircle::new(
            center_vec, radius,
        )))
    }

    #[getter]
    pub fn center(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|c| &c.center)?)
    }

    #[setter]
    pub fn set_center(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.center = value.try_into()?;
        Ok(())
    }

    pub fn radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.radius())
    }

    pub fn closest_point(&self, point: PyVec2) -> PyResult<PyVec2> {
        let point_vec: Vec2 = point.try_into()?;
        Ok(PyVec2::from_vec2(self.as_ref()?.closest_point(point_vec)))
    }

    pub fn contains(&self, other: &PyBoundingCircle) -> PyResult<bool> {
        let other_circle: BoundingCircle = other.try_into()?;
        Ok(self.as_ref()?.contains(&other_circle))
    }

    /// True if this BoundingCircle intersects another BoundingCircle or an Aabb2d.
    pub fn intersects(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if other.is_instance_of::<PyBoundingCircle>() {
            let other_circle: BoundingCircle =
                BoundingCircle::try_from(&*other.cast::<PyBoundingCircle>()?.borrow())?;
            return Ok(self.as_ref()?.intersects(&other_circle));
        }
        if other.is_instance_of::<PyAabb2d>() {
            let aabb: Aabb2d = Aabb2d::try_from(&*other.cast::<PyAabb2d>()?.borrow())?;
            return Ok(self.as_ref()?.intersects(&aabb));
        }
        Err(PyTypeError::new_err("expected BoundingCircle or Aabb2d"))
    }

    pub fn merge(&self, other: &PyBoundingCircle) -> PyResult<PyBoundingCircle> {
        let other_circle: BoundingCircle = other.try_into()?;
        Ok(PyBoundingCircle::from_bounding_circle(
            self.as_ref()?.merge(&other_circle),
        ))
    }

    pub fn rotated_by(&self, rotation: &Bound<'_, PyAny>) -> PyResult<PyBoundingCircle> {
        let rotation = extract_rot2_from_any(rotation)?;
        Ok(PyBoundingCircle::from_bounding_circle(
            (*self.as_ref()?).rotated_by(rotation),
        ))
    }

    pub fn transformed_by(
        &self,
        translation: &Bound<'_, PyAny>,
        rotation: &Bound<'_, PyAny>,
    ) -> PyResult<PyBoundingCircle> {
        let translation = extract_vec2_from_any(translation)?;
        let rotation = extract_rot2_from_any(rotation)?;
        Ok(PyBoundingCircle::from_bounding_circle(
            (*self.as_ref()?).transformed_by(translation, rotation),
        ))
    }

    pub fn grow(&self, amount: f32) -> PyResult<PyBoundingCircle> {
        Ok(PyBoundingCircle::from_bounding_circle(
            self.as_ref()?.grow(amount),
        ))
    }

    pub fn shrink(&self, amount: f32) -> PyResult<PyBoundingCircle> {
        Ok(PyBoundingCircle::from_bounding_circle(
            self.as_ref()?.shrink(amount),
        ))
    }

    pub fn scale_around_center(&self, scale: f32) -> PyResult<PyBoundingCircle> {
        Ok(PyBoundingCircle::from_bounding_circle(
            self.as_ref()?.scale_around_center(scale),
        ))
    }

    pub fn visible_area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.visible_area())
    }

    pub fn aabb_2d(&self) -> PyResult<PyAabb2d> {
        Ok(PyAabb2d::from_aabb2d(self.as_ref()?.aabb_2d()))
    }

    pub fn intersects_circle(&self, other: &PyBoundingCircle) -> PyResult<bool> {
        let other_circle: BoundingCircle = other.try_into()?;
        Ok(self.as_ref()?.intersects(&other_circle))
    }

    pub fn intersects_aabb(&self, aabb: &PyAabb2d) -> PyResult<bool> {
        let aabb_2d: Aabb2d = aabb.try_into()?;
        Ok(self.as_ref()?.intersects(&aabb_2d))
    }

    #[staticmethod]
    pub fn from_point_cloud(
        isometry: super::aabb2d::PyIsometry2d,
        points: Vec<PyVec2>,
    ) -> PyResult<PyBoundingCircle> {
        let iso: Isometry2d = isometry.into();
        let point_refs: Vec<Vec2> = points
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;

        Ok(PyBoundingCircle::from_bounding_circle(
            BoundingCircle::from_point_cloud(iso, &point_refs),
        ))
    }

    pub fn __eq__(&self, other: &PyBoundingCircle) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    fn __repr__(&self) -> PyResult<String> {
        let circle = self.as_ref()?;
        Ok(format!(
            "BoundingCircle(center={:?}, radius={})",
            circle.center,
            circle.radius()
        ))
    }
}
