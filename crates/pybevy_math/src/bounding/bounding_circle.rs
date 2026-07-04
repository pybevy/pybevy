use bevy::math::{
    Vec2,
    bounding::{BoundingCircle, BoundingVolume, IntersectsVolume},
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::prelude::*;

use super::aabb2d::PyAabb2d;
use crate::vec2::PyVec2;

#[pyclass(name = "BoundingCircle", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyBoundingCircle {
    storage: ValueStorage<BoundingCircle>,
}

impl From<PyBoundingCircle> for BoundingCircle {
    #[inline(always)]
    fn from(py_circle: PyBoundingCircle) -> Self {
        match py_circle.storage.get() {
            Ok(val) => val,
            Err(_) => BoundingCircle::new(Vec2::ZERO, 0.0),
        }
    }
}

impl From<&PyBoundingCircle> for BoundingCircle {
    #[inline(always)]
    fn from(py_circle: &PyBoundingCircle) -> Self {
        match py_circle.storage.get() {
            Ok(val) => val,
            Err(_) => BoundingCircle::new(Vec2::ZERO, 0.0),
        }
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
    fn as_ref(&self) -> PyResult<&BoundingCircle> {
        Ok(self.storage.as_ref()?)
    }

    pub(crate) fn to_bounding_circle(&self) -> BoundingCircle {
        match self.storage.get() {
            Ok(val) => val,
            Err(_) => BoundingCircle::new(Vec2::ZERO, 0.0),
        }
    }
}

#[pymethods]
impl PyBoundingCircle {
    #[new]
    pub fn new(center: PyVec2, radius: f32) -> Self {
        let center_vec: Vec2 = center.into();
        PyBoundingCircle::from_bounding_circle(BoundingCircle::new(center_vec, radius))
    }

    #[getter]
    pub fn center(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.center))
    }

    pub fn radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.radius())
    }

    pub fn closest_point(&self, point: PyVec2) -> PyResult<PyVec2> {
        let point_vec: Vec2 = point.into();
        Ok(PyVec2::from_vec2(self.as_ref()?.closest_point(point_vec)))
    }

    pub fn contains(&self, other: &PyBoundingCircle) -> PyResult<bool> {
        let other_circle: BoundingCircle = other.into();
        Ok(self.as_ref()?.contains(&other_circle))
    }

    pub fn merge(&self, other: &PyBoundingCircle) -> PyResult<PyBoundingCircle> {
        let other_circle: BoundingCircle = other.into();
        Ok(PyBoundingCircle::from_bounding_circle(
            self.as_ref()?.merge(&other_circle),
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
        let other_circle: BoundingCircle = other.into();
        Ok(self.as_ref()?.intersects(&other_circle))
    }

    pub fn intersects_aabb(&self, aabb: &PyAabb2d) -> PyResult<bool> {
        let aabb_2d: bevy::math::bounding::Aabb2d = aabb.into();
        Ok(self.as_ref()?.intersects(&aabb_2d))
    }

    #[staticmethod]
    pub fn from_point_cloud(
        isometry: super::aabb2d::PyIsometry2d,
        points: Vec<PyVec2>,
    ) -> PyResult<PyBoundingCircle> {
        use bevy::math::Isometry2d;

        let iso: Isometry2d = isometry.into();
        let point_refs: Vec<Vec2> = points.into_iter().map(|p| p.into()).collect();

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
