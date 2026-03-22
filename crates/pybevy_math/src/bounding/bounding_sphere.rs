use bevy::math::{
    Vec3, Vec3A,
    bounding::{BoundingSphere, BoundingVolume, IntersectsVolume},
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::prelude::*;

use super::aabb3d::PyAabb3d;
use crate::vec3::PyVec3;

#[pyclass(name = "BoundingSphere")]
#[derive(Debug, Clone)]
pub struct PyBoundingSphere {
    storage: ValueStorage<BoundingSphere>,
}

impl From<PyBoundingSphere> for BoundingSphere {
    #[inline(always)]
    fn from(py_sphere: PyBoundingSphere) -> Self {
        match py_sphere.storage.get() {
            Ok(val) => val,
            Err(_) => BoundingSphere::new(Vec3A::ZERO, 0.0),
        }
    }
}

impl From<&PyBoundingSphere> for BoundingSphere {
    #[inline(always)]
    fn from(py_sphere: &PyBoundingSphere) -> Self {
        match py_sphere.storage.get() {
            Ok(val) => val,
            Err(_) => BoundingSphere::new(Vec3A::ZERO, 0.0),
        }
    }
}

impl From<BoundingSphere> for PyBoundingSphere {
    #[inline(always)]
    fn from(sphere: BoundingSphere) -> Self {
        PyBoundingSphere::from_bounding_sphere(sphere)
    }
}

impl FromBorrowedStorage<ValueStorage<BoundingSphere>> for PyBoundingSphere {
    fn from_borrowed(storage: ValueStorage<BoundingSphere>) -> Self {
        PyBoundingSphere { storage }
    }
}

impl PyBoundingSphere {
    #[inline(always)]
    pub fn from_bounding_sphere(sphere: BoundingSphere) -> Self {
        PyBoundingSphere {
            storage: ValueStorage::owned(sphere),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&BoundingSphere> {
        Ok(self.storage.as_ref()?)
    }
}

#[pymethods]
impl PyBoundingSphere {
    #[new]
    pub fn new(center: PyVec3, radius: f32) -> Self {
        let center_vec: Vec3 = center.into();
        PyBoundingSphere::from_bounding_sphere(BoundingSphere::new(Vec3A::from(center_vec), radius))
    }

    #[getter]
    pub fn center(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.center.into()))
    }

    pub fn radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.radius())
    }

    pub fn closest_point(&self, point: PyVec3) -> PyResult<PyVec3> {
        let point_vec: Vec3 = point.into();
        Ok(PyVec3::from_vec3(
            self.as_ref()?.closest_point(Vec3A::from(point_vec)).into(),
        ))
    }

    pub fn contains(&self, other: &PyBoundingSphere) -> PyResult<bool> {
        let other_sphere: BoundingSphere = other.into();
        Ok(self.as_ref()?.contains(&other_sphere))
    }

    pub fn merge(&self, other: &PyBoundingSphere) -> PyResult<PyBoundingSphere> {
        let other_sphere: BoundingSphere = other.into();
        Ok(PyBoundingSphere::from_bounding_sphere(
            self.as_ref()?.merge(&other_sphere),
        ))
    }

    pub fn grow(&self, amount: f32) -> PyResult<PyBoundingSphere> {
        Ok(PyBoundingSphere::from_bounding_sphere(
            self.as_ref()?.grow(amount),
        ))
    }

    pub fn shrink(&self, amount: f32) -> PyResult<PyBoundingSphere> {
        Ok(PyBoundingSphere::from_bounding_sphere(
            self.as_ref()?.shrink(amount),
        ))
    }

    pub fn scale_around_center(&self, scale: f32) -> PyResult<PyBoundingSphere> {
        Ok(PyBoundingSphere::from_bounding_sphere(
            self.as_ref()?.scale_around_center(scale),
        ))
    }

    pub fn visible_area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.visible_area())
    }

    pub fn aabb_3d(&self) -> PyResult<PyAabb3d> {
        Ok(PyAabb3d::from_aabb3d(self.as_ref()?.aabb_3d()))
    }

    pub fn intersects_sphere(&self, other: &PyBoundingSphere) -> PyResult<bool> {
        let other_sphere: BoundingSphere = other.into();
        Ok(self.as_ref()?.intersects(&other_sphere))
    }

    pub fn intersects_aabb(&self, aabb: &PyAabb3d) -> PyResult<bool> {
        let aabb_3d: bevy::math::bounding::Aabb3d = aabb.into();
        Ok(self.as_ref()?.intersects(&aabb_3d))
    }

    #[staticmethod]
    pub fn from_point_cloud(
        isometry: super::aabb3d::PyIsometry3d,
        points: Vec<PyVec3>,
    ) -> PyResult<PyBoundingSphere> {
        use bevy::math::Isometry3d;

        let iso: Isometry3d = isometry.into();
        let point_refs: Vec<Vec3A> = points
            .into_iter()
            .map(|p| {
                let v: Vec3 = p.into();
                v.into()
            })
            .collect();

        Ok(PyBoundingSphere::from_bounding_sphere(
            BoundingSphere::from_point_cloud(iso, &point_refs),
        ))
    }

    pub fn __eq__(&self, other: &PyBoundingSphere) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    fn __repr__(&self) -> PyResult<String> {
        let sphere = self.as_ref()?;
        Ok(format!(
            "BoundingSphere(center={:?}, radius={})",
            sphere.center,
            sphere.radius()
        ))
    }
}
