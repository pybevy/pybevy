use bevy::math::{
    Isometry3d, Vec3, Vec3A,
    bounding::{Aabb3d, BoundingSphere, BoundingVolume, IntersectsVolume},
};
use pybevy_core::{
    FromBorrowedStorage, ValueStorage,
    public_error::{EMPTY_POINT_CLOUD, bounding_operation, bounding_radius, bounding_shrink},
};
use pybevy_macros::pyvalue;
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use super::aabb3d::PyAabb3d;
use crate::{
    quat::extract_quat_from_any,
    vec3::PyVec3,
    vec3a::{PyVec3A, extract_vec3a_from_any},
};

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
    #[new]
    pub fn new(center: &Bound<'_, PyAny>, radius: f32) -> PyResult<Self> {
        // Reject inputs that fail Bevy nonnegative radius precondition.
        if radius.is_nan() || radius < 0.0 {
            return Err(PyValueError::new_err(bounding_radius(
                "BoundingSphere",
                radius,
            )));
        }
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

    pub fn closest_point(&self, point: &Bound<'_, PyAny>) -> PyResult<PyVec3> {
        let point = extract_vec3a_from_any(point)?;
        Ok(PyVec3::from_vec3(
            self.as_ref()?.closest_point(point).into(),
        ))
    }

    pub fn contains(&self, other: &PyBoundingSphere) -> PyResult<bool> {
        let other_sphere: BoundingSphere = other.try_into()?;
        Ok(self.as_ref()?.contains(&other_sphere))
    }

    /// True if this BoundingSphere intersects another BoundingSphere or an Aabb3d.
    pub fn intersects(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if other.is_instance_of::<PyBoundingSphere>() {
            let other_sphere: BoundingSphere =
                BoundingSphere::try_from(&*other.cast::<PyBoundingSphere>()?.borrow())?;
            return Ok(self.as_ref()?.intersects(&other_sphere));
        }
        if other.is_instance_of::<PyAabb3d>() {
            let aabb: Aabb3d = Aabb3d::try_from(&*other.cast::<PyAabb3d>()?.borrow())?;
            return Ok(self.as_ref()?.intersects(&aabb));
        }
        Err(PyTypeError::new_err("expected BoundingSphere or Aabb3d"))
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
        if amount.is_nan() || amount < 0.0 {
            return Err(PyValueError::new_err(bounding_operation(
                "BoundingSphere",
                "grow",
            )));
        }
        Ok(PyBoundingSphere::from_owned(bounds.grow(amount)))
    }

    pub fn shrink(&self, amount: f32) -> PyResult<PyBoundingSphere> {
        let bounds = self.as_ref()?;
        if amount.is_nan()
            || amount < 0.0
            || bounds
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
        if scale.is_nan() || scale < 0.0 {
            return Err(PyValueError::new_err(bounding_operation(
                "BoundingSphere",
                "scale_around_center",
            )));
        }
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

    pub fn intersects_sphere(&self, other: &PyBoundingSphere) -> PyResult<bool> {
        let other_sphere: BoundingSphere = other.try_into()?;
        Ok(self.as_ref()?.intersects(&other_sphere))
    }

    pub fn intersects_aabb(&self, aabb: &PyAabb3d) -> PyResult<bool> {
        let aabb_3d: Aabb3d = aabb.try_into()?;
        Ok(self.as_ref()?.intersects(&aabb_3d))
    }

    #[staticmethod]
    pub fn from_point_cloud(
        isometry: super::aabb3d::PyIsometry3d,
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
            "BoundingSphere(center={:?}, radius={})",
            sphere.center,
            sphere.radius()
        ))
    }
}
