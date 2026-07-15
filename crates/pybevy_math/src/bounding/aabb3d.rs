use bevy::math::{
    Vec3, Vec3A,
    bounding::{Aabb3d, BoundingVolume, IntersectsVolume},
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::prelude::*;

use super::bounding_sphere::PyBoundingSphere;
use crate::vec3::PyVec3;

#[pyclass(name = "Isometry3d", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyIsometry3d {
    rotation: bevy::math::Quat,
    translation: Vec3A,
}

impl From<PyIsometry3d> for bevy::math::Isometry3d {
    fn from(iso: PyIsometry3d) -> Self {
        bevy::math::Isometry3d::new(iso.translation, iso.rotation)
    }
}

impl From<bevy::math::Isometry3d> for PyIsometry3d {
    fn from(iso: bevy::math::Isometry3d) -> Self {
        PyIsometry3d {
            rotation: iso.rotation,
            translation: iso.translation,
        }
    }
}

#[pymethods]
impl PyIsometry3d {
    #[new]
    #[pyo3(signature = (translation = PyVec3::ZERO, rotation = crate::quat::PyQuat::IDENTITY))]
    pub fn new(translation: PyVec3, rotation: crate::quat::PyQuat) -> Self {
        let t: Vec3 = translation.into();
        PyIsometry3d {
            rotation: rotation.into(),
            translation: Vec3A::from(t),
        }
    }

    #[classattr]
    #[pyo3(name = "IDENTITY")]
    fn identity() -> Self {
        PyIsometry3d {
            rotation: bevy::math::Quat::IDENTITY,
            translation: Vec3A::ZERO,
        }
    }

    #[staticmethod]
    pub fn from_rotation(rotation: crate::quat::PyQuat) -> Self {
        PyIsometry3d {
            rotation: rotation.into(),
            translation: Vec3A::ZERO,
        }
    }

    #[staticmethod]
    pub fn from_translation(translation: PyVec3) -> Self {
        let t: Vec3 = translation.into();
        PyIsometry3d {
            rotation: bevy::math::Quat::IDENTITY,
            translation: Vec3A::from(t),
        }
    }

    #[staticmethod]
    pub fn from_xyz(x: f32, y: f32, z: f32) -> Self {
        PyIsometry3d {
            rotation: bevy::math::Quat::IDENTITY,
            translation: Vec3A::new(x, y, z),
        }
    }

    #[getter]
    pub fn translation(&self) -> PyVec3 {
        let v: Vec3 = self.translation.into();
        v.into()
    }

    #[getter]
    pub fn rotation(&self) -> crate::quat::PyQuat {
        self.rotation.into()
    }

    pub fn inverse(&self) -> Self {
        let inv_rot = self.rotation.inverse();
        PyIsometry3d {
            rotation: inv_rot,
            translation: inv_rot * -self.translation,
        }
    }

    pub fn inverse_mul(&self, rhs: &PyIsometry3d) -> Self {
        let inv_rot = self.rotation.inverse();
        let delta_translation = rhs.translation - self.translation;
        PyIsometry3d {
            rotation: inv_rot * rhs.rotation,
            translation: inv_rot * delta_translation,
        }
    }

    pub fn transform_point(&self, point: PyVec3) -> PyVec3 {
        let p: Vec3 = point.into();
        let result: Vec3 = (self.rotation * Vec3A::from(p) + self.translation).into();
        result.into()
    }

    pub fn inverse_transform_point(&self, point: PyVec3) -> PyVec3 {
        let p: Vec3 = point.into();
        let result: Vec3 = (self.rotation.inverse() * (Vec3A::from(p) - self.translation)).into();
        result.into()
    }

    fn __repr__(&self) -> String {
        let t: Vec3 = self.translation.into();
        format!(
            "Isometry3d(translation=Vec3({}, {}, {}), rotation=Quat({}, {}, {}, {}))",
            t.x, t.y, t.z, self.rotation.x, self.rotation.y, self.rotation.z, self.rotation.w
        )
    }
}

#[pyclass(name = "Aabb3d", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAabb3d {
    storage: ValueStorage<Aabb3d>,
}

impl From<PyAabb3d> for Aabb3d {
    #[inline(always)]
    fn from(py_aabb: PyAabb3d) -> Self {
        match py_aabb.storage.get() {
            Ok(val) => val,
            Err(_) => Aabb3d::new(Vec3A::ZERO, Vec3A::ZERO),
        }
    }
}

impl From<&PyAabb3d> for Aabb3d {
    #[inline(always)]
    fn from(py_aabb: &PyAabb3d) -> Self {
        match py_aabb.storage.get() {
            Ok(val) => val,
            Err(_) => Aabb3d::new(Vec3A::ZERO, Vec3A::ZERO),
        }
    }
}

impl From<Aabb3d> for PyAabb3d {
    #[inline(always)]
    fn from(aabb: Aabb3d) -> Self {
        PyAabb3d::from_aabb3d(aabb)
    }
}

impl FromBorrowedStorage<ValueStorage<Aabb3d>> for PyAabb3d {
    fn from_borrowed(storage: ValueStorage<Aabb3d>) -> Self {
        PyAabb3d { storage }
    }
}

impl PyAabb3d {
    #[inline(always)]
    pub fn from_aabb3d(aabb: Aabb3d) -> Self {
        PyAabb3d {
            storage: ValueStorage::owned(aabb),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<&Aabb3d> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Aabb3d> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyAabb3d {
    #[new]
    #[pyo3(signature = (center, half_size))]
    pub fn new(center: PyVec3, half_size: PyVec3) -> Self {
        let center_vec: Vec3 = center.into();
        let half_size_vec: Vec3 = half_size.into();
        PyAabb3d::from_aabb3d(Aabb3d::new(
            Vec3A::from(center_vec),
            Vec3A::from(half_size_vec),
        ))
    }

    #[staticmethod]
    pub fn from_min_max(min: PyVec3, max: PyVec3) -> Self {
        let min_vec: Vec3 = min.into();
        let max_vec: Vec3 = max.into();
        PyAabb3d::from_aabb3d(Aabb3d::from_min_max(
            Vec3A::from(min_vec),
            Vec3A::from(max_vec),
        ))
    }

    #[getter]
    pub fn min(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.min.into()))
    }

    #[setter]
    pub fn set_min(&mut self, value: PyVec3) -> PyResult<()> {
        self.as_mut()?.min = Vec3::from(value).into();
        Ok(())
    }

    #[getter]
    pub fn max(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.max.into()))
    }

    #[setter]
    pub fn set_max(&mut self, value: PyVec3) -> PyResult<()> {
        self.as_mut()?.max = Vec3::from(value).into();
        Ok(())
    }

    pub fn center(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.center().into()))
    }

    pub fn half_size(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.half_size().into()))
    }

    pub fn closest_point(&self, point: PyVec3) -> PyResult<PyVec3> {
        let point_vec: Vec3 = point.into();
        Ok(PyVec3::from_vec3(
            self.as_ref()?.closest_point(Vec3A::from(point_vec)).into(),
        ))
    }

    pub fn contains(&self, other: &PyAabb3d) -> PyResult<bool> {
        let other_aabb: Aabb3d = other.into();
        Ok(self.as_ref()?.contains(&other_aabb))
    }

    pub fn merge(&self, other: &PyAabb3d) -> PyResult<PyAabb3d> {
        let other_aabb: Aabb3d = other.into();
        Ok(PyAabb3d::from_aabb3d(self.as_ref()?.merge(&other_aabb)))
    }

    pub fn grow(&self, amount: PyVec3) -> PyResult<PyAabb3d> {
        let amount_vec: Vec3 = amount.into();
        Ok(PyAabb3d::from_aabb3d(
            self.as_ref()?.grow(Vec3A::from(amount_vec)),
        ))
    }

    pub fn shrink(&self, amount: PyVec3) -> PyResult<PyAabb3d> {
        let amount_vec: Vec3 = amount.into();
        Ok(PyAabb3d::from_aabb3d(
            self.as_ref()?.shrink(Vec3A::from(amount_vec)),
        ))
    }

    pub fn scale_around_center(&self, scale: PyVec3) -> PyResult<PyAabb3d> {
        let scale_vec: Vec3 = scale.into();
        Ok(PyAabb3d::from_aabb3d(
            self.as_ref()?.scale_around_center(Vec3A::from(scale_vec)),
        ))
    }

    pub fn visible_area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.visible_area())
    }

    pub fn bounding_sphere(&self) -> PyResult<PyBoundingSphere> {
        Ok(PyBoundingSphere::from_bounding_sphere(
            self.as_ref()?.bounding_sphere(),
        ))
    }

    pub fn intersects_aabb(&self, other: &PyAabb3d) -> PyResult<bool> {
        let other_aabb: Aabb3d = other.into();
        Ok(self.as_ref()?.intersects(&other_aabb))
    }

    pub fn intersects_sphere(&self, sphere: &PyBoundingSphere) -> PyResult<bool> {
        let bounding_sphere: bevy::math::bounding::BoundingSphere = sphere.into();
        Ok(self.as_ref()?.intersects(&bounding_sphere))
    }

    #[staticmethod]
    pub fn from_point_cloud(isometry: PyIsometry3d, points: Vec<PyVec3>) -> PyResult<PyAabb3d> {
        use bevy::math::Isometry3d;

        let iso: Isometry3d = isometry.into();
        let point_refs: Vec<Vec3A> = points
            .into_iter()
            .map(|p| {
                let v: Vec3 = p.into();
                v.into()
            })
            .collect();

        Ok(PyAabb3d::from_aabb3d(Aabb3d::from_point_cloud(
            iso,
            point_refs.into_iter(),
        )))
    }

    pub fn __eq__(&self, other: &PyAabb3d) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    fn __repr__(&self) -> PyResult<String> {
        let aabb = self.as_ref()?;
        Ok(format!("Aabb3d(min={:?}, max={:?})", aabb.min, aabb.max))
    }
}
