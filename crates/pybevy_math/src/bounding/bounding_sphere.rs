use bevy::math::{Isometry3d, Vec3, Vec3A};
use bevy::shape::{BoundingSphere, BoundingVolume};
use pybevy_core::{
    FromBorrowedStorage, ValueStorage,
    domain::{NON_NEGATIVE, check_for},
    public_error::{EMPTY_POINT_CLOUD, bounding_operation, bounding_shrink},
    repr::{ReprLiteral, float_repr},
};
use pybevy_macros::pyvalue;
use pyo3::{exceptions::PyValueError, prelude::*};

use super::aabb3d::{PyAabb3d, intersects_3d};
use crate::{
    isometry3d::PyIsometry3d,
    quat::extract_quat_from_any,
    vec3::PyVec3,
    vec3a::{PyVec3A, extract_vec3a_from_any},
};

/// Correct Bevy's translated-sphere projection until its implementation uses
/// the center-relative direction (still present at bevyengine/bevy@70c2b021).
fn bounding_sphere_closest_point(sphere: &BoundingSphere, point: Vec3A) -> Vec3A {
    let offset = point - sphere.center;
    let distance_squared = offset.length_squared();
    let radius = sphere.radius();

    if distance_squared <= radius * radius {
        point
    } else {
        sphere.center + radius * (offset / distance_squared.sqrt())
    }
}

#[pyvalue]
#[pyclass(name = "BoundingSphere", module = "pybevy.math", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyBoundingSphere {
    storage: ValueStorage<BoundingSphere>,
}

impl TryFrom<PyBoundingSphere> for BoundingSphere {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_sphere: PyBoundingSphere) -> PyResult<Self> {
        Ok(py_sphere.storage.get()?)
    }
}

impl TryFrom<&PyBoundingSphere> for BoundingSphere {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_sphere: &PyBoundingSphere) -> PyResult<Self> {
        Ok(py_sphere.storage.get()?)
    }
}

impl From<BoundingSphere> for PyBoundingSphere {
    #[inline(always)]
    fn from(sphere: BoundingSphere) -> Self {
        PyBoundingSphere::from_owned(sphere)
    }
}

#[pymethods]
impl PyBoundingSphere {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(BoundingSphere::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    pub fn new(center: &Bound<'_, PyAny>, radius: f32) -> PyResult<Self> {
        let radius = check_for::<Self, _>("radius", radius, NON_NEGATIVE)?;
        Ok(PyBoundingSphere::from_owned(BoundingSphere::new(
            extract_vec3a_from_any(center)?,
            radius,
        )))
    }

    #[getter]
    pub fn center(&self) -> PyResult<PyVec3A> {
        Ok(self.storage.borrow_field_as(|s| &s.center)?)
    }

    #[setter]
    pub fn set_center(&mut self, value: PyVec3A) -> PyResult<()> {
        self.as_mut()?.center = value.try_into()?;
        Ok(())
    }

    pub fn radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.radius())
    }

    pub fn closest_point(&self, point: &Bound<'_, PyAny>) -> PyResult<PyVec3A> {
        let point = extract_vec3a_from_any(point)?;
        let sphere = self.as_ref()?;
        Ok(PyVec3A::from_vec3a(bounding_sphere_closest_point(
            &sphere, point,
        )))
    }

    pub fn contains(&self, other: &PyBoundingSphere) -> PyResult<bool> {
        let other_sphere: BoundingSphere = other.try_into()?;
        Ok(self.as_ref()?.contains(&other_sphere))
    }

    /// True if this BoundingSphere intersects an Aabb3d or a BoundingSphere.
    pub fn intersects(&self, volume: &Bound<'_, PyAny>) -> PyResult<bool> {
        intersects_3d(&*self.as_ref()?, volume)
    }

    pub fn merge(&self, other: &PyBoundingSphere) -> PyResult<PyBoundingSphere> {
        let other_sphere: BoundingSphere = other.try_into()?;
        let bounds = self.as_ref()?;
        let length = (bounds.center - other_sphere.center).length();
        if !(bounds.radius() >= length + other_sphere.radius()
            || other_sphere.radius() >= length + bounds.radius())
        {
            let radius = (length + bounds.radius() + other_sphere.radius()) / 2.0;
            if radius.is_nan() || radius < 0.0 {
                return Err(PyValueError::new_err(bounding_operation(
                    "BoundingSphere",
                    "merge",
                )));
            }
        }
        Ok(PyBoundingSphere::from_owned(bounds.merge(&other_sphere)))
    }

    pub fn rotated_by(&self, rotation: &Bound<'_, PyAny>) -> PyResult<PyBoundingSphere> {
        let rotation = extract_quat_from_any(rotation)?;
        Ok(PyBoundingSphere::from_owned(
            (*self.as_ref()?).rotated_by(rotation),
        ))
    }

    pub fn transformed_by(
        &self,
        translation: &Bound<'_, PyAny>,
        rotation: &Bound<'_, PyAny>,
    ) -> PyResult<PyBoundingSphere> {
        let translation = extract_vec3a_from_any(translation)?;
        let rotation = extract_quat_from_any(rotation)?;
        Ok(PyBoundingSphere::from_owned(
            (*self.as_ref()?).transformed_by(translation, rotation),
        ))
    }

    pub fn grow(&self, amount: f32) -> PyResult<PyBoundingSphere> {
        let bounds = self.as_ref()?;
        let amount = check_for::<Self, _>("amount", amount, NON_NEGATIVE)?;
        Ok(PyBoundingSphere::from_owned(bounds.grow(amount)))
    }

    pub fn shrink(&self, amount: f32) -> PyResult<PyBoundingSphere> {
        let bounds = self.as_ref()?;
        let amount = check_for::<Self, _>("amount", amount, NON_NEGATIVE)?;
        if bounds
            .radius()
            .partial_cmp(&amount)
            .is_none_or(|order| order.is_lt())
        {
            return Err(PyValueError::new_err(bounding_shrink("BoundingSphere")));
        }
        Ok(PyBoundingSphere::from_owned(bounds.shrink(amount)))
    }

    pub fn scale_around_center(&self, scale: f32) -> PyResult<PyBoundingSphere> {
        let bounds = self.as_ref()?;
        let scale = check_for::<Self, _>("scale", scale, NON_NEGATIVE)?;
        Ok(PyBoundingSphere::from_owned(
            bounds.scale_around_center(scale),
        ))
    }

    pub fn visible_area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.visible_area())
    }

    pub fn aabb_3d(&self) -> PyResult<PyAabb3d> {
        Ok(PyAabb3d::from_owned(self.as_ref()?.aabb_3d()))
    }

    #[staticmethod]
    pub fn from_point_cloud(
        isometry: PyIsometry3d,
        points: Vec<PyVec3>,
    ) -> PyResult<PyBoundingSphere> {
        if points.is_empty() {
            return Err(PyValueError::new_err(EMPTY_POINT_CLOUD));
        }
        let iso: Isometry3d = isometry.try_into()?;
        let point_refs: Vec<Vec3A> = points
            .into_iter()
            .map(|p| {
                let v: Vec3 = p.try_into()?;
                Ok(Vec3A::from(v))
            })
            .collect::<PyResult<Vec<_>>>()?;

        Ok(PyBoundingSphere::from_owned(
            BoundingSphere::from_point_cloud(iso, &point_refs),
        ))
    }

    pub fn __eq__(&self, other: &PyBoundingSphere) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    fn __repr__(&self) -> PyResult<String> {
        let sphere = self.as_ref()?;
        Ok(format!(
            "BoundingSphere(center={}, radius={})",
            sphere.center.repr_literal(),
            float_repr(sphere.radius())
        ))
    }

    pub fn translated_by(&self, translation: &Bound<'_, PyAny>) -> PyResult<PyBoundingSphere> {
        let translation = extract_vec3a_from_any(translation)?;
        Ok(BoundingSphere::try_from(self)?
            .translated_by(translation)
            .into())
    }

    pub fn translate_by(&mut self, translation: &Bound<'_, PyAny>) -> PyResult<()> {
        let translation = extract_vec3a_from_any(translation)?;
        self.as_mut()?.translate_by(translation);
        Ok(())
    }

    pub fn rotate_by(&mut self, rotation: &Bound<'_, PyAny>) -> PyResult<()> {
        let rotated = BoundingSphere::try_from(&self.rotated_by(rotation)?)?;
        *self.as_mut()? = rotated;
        Ok(())
    }

    pub fn transform_by(
        &mut self,
        translation: &Bound<'_, PyAny>,
        rotation: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let transformed = BoundingSphere::try_from(&self.transformed_by(translation, rotation)?)?;
        *self.as_mut()? = transformed;
        Ok(())
    }
}
