use bevy::math::{Isometry2d, Vec2};
use bevy::shape::{BoundingCircle, BoundingVolume};
use pybevy_core::{
    FromBorrowedStorage, StorageMut, StorageRef, ValueStorage,
    domain::{NON_NEGATIVE, check_for},
    public_error::{EMPTY_POINT_CLOUD, bounding_operation, bounding_shrink},
    repr::{ReprLiteral, float_repr},
};
use pyo3::{exceptions::PyValueError, prelude::*};

use super::aabb2d::{PyAabb2d, intersects_2d};
use crate::{
    isometry2d::PyIsometry2d,
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
}

#[pymethods]
impl PyBoundingCircle {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(BoundingCircle::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    pub fn new(center: PyVec2, radius: f32) -> PyResult<Self> {
        let center_vec: Vec2 = center.try_into()?;
        let radius = check_for::<Self, _>("radius", radius, NON_NEGATIVE)?;
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

    /// True if this BoundingCircle intersects an Aabb2d or a BoundingCircle.
    pub fn intersects(&self, volume: &Bound<'_, PyAny>) -> PyResult<bool> {
        intersects_2d(&*self.as_ref()?, volume)
    }

    pub fn merge(&self, other: &PyBoundingCircle) -> PyResult<PyBoundingCircle> {
        let other_circle: BoundingCircle = other.try_into()?;
        let bounds = self.as_ref()?;
        let length = (bounds.center - other_circle.center).length();
        if !(bounds.radius() >= length + other_circle.radius()
            || other_circle.radius() >= length + bounds.radius())
        {
            let radius = (length + bounds.radius() + other_circle.radius()) / 2.0;
            if radius.is_nan() || radius < 0.0 {
                return Err(PyValueError::new_err(bounding_operation(
                    "BoundingCircle",
                    "merge",
                )));
            }
        }
        Ok(PyBoundingCircle::from_bounding_circle(
            bounds.merge(&other_circle),
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
        let bounds = self.as_ref()?;
        let amount = check_for::<Self, _>("amount", amount, NON_NEGATIVE)?;
        Ok(PyBoundingCircle::from_bounding_circle(bounds.grow(amount)))
    }

    pub fn shrink(&self, amount: f32) -> PyResult<PyBoundingCircle> {
        let bounds = self.as_ref()?;
        let amount = check_for::<Self, _>("amount", amount, NON_NEGATIVE)?;
        if bounds
            .radius()
            .partial_cmp(&amount)
            .is_none_or(|order| order.is_lt())
        {
            return Err(PyValueError::new_err(bounding_shrink("BoundingCircle")));
        }
        Ok(PyBoundingCircle::from_bounding_circle(
            bounds.shrink(amount),
        ))
    }

    pub fn scale_around_center(&self, scale: f32) -> PyResult<PyBoundingCircle> {
        let bounds = self.as_ref()?;
        let scale = check_for::<Self, _>("scale", scale, NON_NEGATIVE)?;
        Ok(PyBoundingCircle::from_bounding_circle(
            bounds.scale_around_center(scale),
        ))
    }

    pub fn visible_area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.visible_area())
    }

    pub fn aabb_2d(&self) -> PyResult<PyAabb2d> {
        Ok(PyAabb2d::from_aabb2d(self.as_ref()?.aabb_2d()))
    }

    #[staticmethod]
    pub fn from_point_cloud(
        isometry: PyIsometry2d,
        points: Vec<PyVec2>,
    ) -> PyResult<PyBoundingCircle> {
        if points.is_empty() {
            return Err(PyValueError::new_err(EMPTY_POINT_CLOUD));
        }
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
            "BoundingCircle(center={}, radius={})",
            circle.center.repr_literal(),
            float_repr(circle.radius())
        ))
    }

    pub fn translated_by(&self, translation: &Bound<'_, PyAny>) -> PyResult<PyBoundingCircle> {
        let translation = extract_vec2_from_any(translation)?;
        Ok(BoundingCircle::try_from(self)?
            .translated_by(translation)
            .into())
    }

    pub fn translate_by(&mut self, translation: &Bound<'_, PyAny>) -> PyResult<()> {
        let translation = extract_vec2_from_any(translation)?;
        self.as_mut()?.translate_by(translation);
        Ok(())
    }

    pub fn rotate_by(&mut self, rotation: &Bound<'_, PyAny>) -> PyResult<()> {
        let rotated = BoundingCircle::try_from(&self.rotated_by(rotation)?)?;
        *self.as_mut()? = rotated;
        Ok(())
    }

    pub fn transform_by(
        &mut self,
        translation: &Bound<'_, PyAny>,
        rotation: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let transformed = BoundingCircle::try_from(&self.transformed_by(translation, rotation)?)?;
        *self.as_mut()? = transformed;
        Ok(())
    }
}
