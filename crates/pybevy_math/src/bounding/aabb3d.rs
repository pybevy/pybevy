use bevy::math::{
    Vec3, Vec3A,
    bounding::{Aabb3d, BoundingVolume, IntersectsVolume},
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

use super::bounding_sphere::PyBoundingSphere;
use crate::{
    vec3::PyVec3,
    vec3a::{PyVec3A, extract_vec3a_from_any},
};

#[pyvalue(bevy::math::Isometry3d)]
#[pyclass(name = "Isometry3d", eq, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyIsometry3d {
    pub(crate) storage: ValueStorage<bevy::math::Isometry3d>,
}

impl PartialEq for PyIsometry3d {
    fn eq(&self, other: &Self) -> bool {
        match (self.to_bevy(), other.to_bevy()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl TryFrom<PyIsometry3d> for bevy::math::Isometry3d {
    type Error = PyErr;

    fn try_from(iso: PyIsometry3d) -> PyResult<Self> {
        Ok(iso.storage.get()?)
    }
}

impl TryFrom<&PyIsometry3d> for bevy::math::Isometry3d {
    type Error = PyErr;

    fn try_from(iso: &PyIsometry3d) -> PyResult<Self> {
        Ok(iso.storage.get()?)
    }
}

impl From<bevy::math::Isometry3d> for PyIsometry3d {
    fn from(iso: bevy::math::Isometry3d) -> Self {
        PyIsometry3d::from_owned(iso)
    }
}

#[pymethods]
impl PyIsometry3d {
    #[new]
    #[pyo3(signature = (translation = None, rotation = crate::quat::PyQuat::IDENTITY))]
    pub fn new(
        translation: Option<&Bound<'_, PyAny>>,
        rotation: crate::quat::PyQuat,
    ) -> PyResult<Self> {
        let t = match translation {
            Some(obj) => extract_vec3a_from_any(obj)?,
            None => Vec3A::ZERO,
        };
        Ok(PyIsometry3d::from_owned(bevy::math::Isometry3d::new(
            t,
            rotation.try_into()?,
        )))
    }

    #[classattr]
    #[pyo3(name = "IDENTITY")]
    fn identity() -> Self {
        PyIsometry3d::from_owned(bevy::math::Isometry3d::IDENTITY)
    }

    #[staticmethod]
    pub fn from_rotation(rotation: crate::quat::PyQuat) -> PyResult<Self> {
        Ok(PyIsometry3d::from_owned(
            bevy::math::Isometry3d::from_rotation(rotation.try_into()?),
        ))
    }

    #[staticmethod]
    pub fn from_translation(translation: &Bound<'_, PyAny>) -> PyResult<Self> {
        let t = extract_vec3a_from_any(translation)?;
        Ok(PyIsometry3d::from_owned(
            bevy::math::Isometry3d::from_translation(t),
        ))
    }

    #[staticmethod]
    pub fn from_xyz(x: f32, y: f32, z: f32) -> Self {
        PyIsometry3d::from_owned(bevy::math::Isometry3d::from_xyz(x, y, z))
    }

    #[getter]
    pub fn translation(&self) -> PyResult<PyVec3A> {
        Ok(self.storage.borrow_field_as(|i| &i.translation)?)
    }

    #[setter]
    pub fn set_translation(&mut self, value: PyVec3A) -> PyResult<()> {
        self.storage.as_mut()?.translation = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn rotation(&self) -> PyResult<crate::quat::PyQuat> {
        Ok(self.storage.borrow_field_as(|i| &i.rotation)?)
    }

    #[setter]
    pub fn set_rotation(&mut self, value: crate::quat::PyQuat) -> PyResult<()> {
        self.storage.as_mut()?.rotation = value.try_into()?;
        Ok(())
    }

    pub fn inverse(&self) -> PyResult<Self> {
        Ok(PyIsometry3d::from_owned(self.to_bevy()?.inverse()))
    }

    pub fn inverse_mul(&self, rhs: &PyIsometry3d) -> PyResult<Self> {
        Ok(PyIsometry3d::from_owned(
            self.to_bevy()?.inverse_mul(rhs.to_bevy()?),
        ))
    }

    pub fn transform_point(&self, point: &Bound<'_, PyAny>) -> PyResult<PyVec3> {
        let p = extract_vec3a_from_any(point)?;
        let result: Vec3 = self.to_bevy()?.transform_point(p).into();
        Ok(result.into())
    }

    pub fn inverse_transform_point(&self, point: &Bound<'_, PyAny>) -> PyResult<PyVec3> {
        let p = extract_vec3a_from_any(point)?;
        let result: Vec3 = self.to_bevy()?.inverse_transform_point(p).into();
        Ok(result.into())
    }

    fn __repr__(&self) -> PyResult<String> {
        let iso = self.to_bevy()?;
        let t = iso.translation;
        let r = iso.rotation;
        Ok(format!(
            "Isometry3d(translation=Vec3A({}, {}, {}), rotation=Quat({}, {}, {}, {}))",
            t.x, t.y, t.z, r.x, r.y, r.z, r.w
        ))
    }
}

#[pyvalue]
#[pyclass(name = "Aabb3d", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAabb3d {
    storage: ValueStorage<Aabb3d>,
}

impl PyAabb3d {
    pub fn try_to_bevy(&self) -> PyResult<Aabb3d> {
        Ok(self.storage.get()?)
    }
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
        PyAabb3d::from_owned(aabb)
    }
}

#[pymethods]
impl PyAabb3d {
    #[new]
    #[pyo3(signature = (center, half_size))]
    pub fn new(center: &Bound<'_, PyAny>, half_size: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(PyAabb3d::from_owned(Aabb3d::new(
            extract_vec3a_from_any(center)?,
            extract_vec3a_from_any(half_size)?,
        )))
    }

    #[staticmethod]
    pub fn from_min_max(min: &Bound<'_, PyAny>, max: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(PyAabb3d::from_owned(Aabb3d::from_min_max(
            extract_vec3a_from_any(min)?,
            extract_vec3a_from_any(max)?,
        )))
    }

    #[getter]
    pub fn min(&self) -> PyResult<PyVec3A> {
        Ok(self.storage.borrow_field_as(|a| &a.min)?)
    }

    #[setter]
    pub fn set_min(&mut self, value: PyVec3A) -> PyResult<()> {
        self.as_mut()?.min = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn max(&self) -> PyResult<PyVec3A> {
        Ok(self.storage.borrow_field_as(|a| &a.max)?)
    }

    #[setter]
    pub fn set_max(&mut self, value: PyVec3A) -> PyResult<()> {
        self.as_mut()?.max = value.try_into()?;
        Ok(())
    }

    pub fn center(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.center().into()))
    }

    pub fn half_size(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.half_size().into()))
    }

    pub fn closest_point(&self, point: &Bound<'_, PyAny>) -> PyResult<PyVec3> {
        let point = extract_vec3a_from_any(point)?;
        Ok(PyVec3::from_vec3(
            self.as_ref()?.closest_point(point).into(),
        ))
    }

    pub fn contains(&self, other: &PyAabb3d) -> PyResult<bool> {
        let other_aabb: Aabb3d = other.into();
        Ok(self.as_ref()?.contains(&other_aabb))
    }

    pub fn merge(&self, other: &PyAabb3d) -> PyResult<PyAabb3d> {
        let other_aabb: Aabb3d = other.into();
        Ok(PyAabb3d::from_owned(self.as_ref()?.merge(&other_aabb)))
    }

    pub fn grow(&self, amount: &Bound<'_, PyAny>) -> PyResult<PyAabb3d> {
        let amount = extract_vec3a_from_any(amount)?;
        Ok(PyAabb3d::from_owned(self.as_ref()?.grow(amount)))
    }

    pub fn shrink(&self, amount: &Bound<'_, PyAny>) -> PyResult<PyAabb3d> {
        let amount = extract_vec3a_from_any(amount)?;
        Ok(PyAabb3d::from_owned(self.as_ref()?.shrink(amount)))
    }

    pub fn scale_around_center(&self, scale: &Bound<'_, PyAny>) -> PyResult<PyAabb3d> {
        let scale = extract_vec3a_from_any(scale)?;
        Ok(PyAabb3d::from_owned(
            self.as_ref()?.scale_around_center(scale),
        ))
    }

    pub fn visible_area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.visible_area())
    }

    pub fn bounding_sphere(&self) -> PyResult<PyBoundingSphere> {
        Ok(PyBoundingSphere::from_owned(
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

        let iso: Isometry3d = isometry.try_into()?;
        let point_refs: Vec<Vec3A> = points
            .into_iter()
            .map(|p| {
                let v: Vec3 = p.try_into()?;
                Ok(Vec3A::from(v))
            })
            .collect::<PyResult<Vec<_>>>()?;

        Ok(PyAabb3d::from_owned(Aabb3d::from_point_cloud(
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
