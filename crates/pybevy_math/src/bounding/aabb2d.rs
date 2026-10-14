use bevy::math::{
    Isometry2d, Mat2, Vec2,
    bounding::{Aabb2d, BoundingCircle, BoundingVolume, IntersectsVolume},
};
use pybevy_core::{
    FromBorrowedStorage, StorageMut, StorageRef, ValueStorage,
    domain::{NON_NEGATIVE, check_for},
    public_error::{
        EMPTY_POINT_CLOUD, aabb_min_exceeds_max, bounding_operation, bounding_shrink,
        intersects_volume_type,
    },
    repr::ReprLiteral,
};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use super::bounding_circle::PyBoundingCircle;
use crate::{
    isometry2d::PyIsometry2d,
    rot2::extract_rot2_from_any,
    vec2::{PyVec2, extract_vec2_from_any},
};

#[pyclass(name = "Aabb2d", module = "pybevy.math", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAabb2d {
    storage: ValueStorage<Aabb2d>,
}

impl TryFrom<PyAabb2d> for Aabb2d {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_aabb: PyAabb2d) -> PyResult<Self> {
        Ok(py_aabb.storage.get()?)
    }
}

impl TryFrom<&PyAabb2d> for Aabb2d {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_aabb: &PyAabb2d) -> PyResult<Self> {
        Ok(py_aabb.storage.get()?)
    }
}

impl From<Aabb2d> for PyAabb2d {
    #[inline(always)]
    fn from(aabb: Aabb2d) -> Self {
        PyAabb2d::from_aabb2d(aabb)
    }
}

impl FromBorrowedStorage<ValueStorage<Aabb2d>> for PyAabb2d {
    fn from_borrowed(storage: ValueStorage<Aabb2d>) -> Self {
        PyAabb2d { storage }
    }
}

impl PyAabb2d {
    #[inline(always)]
    pub fn from_aabb2d(aabb: Aabb2d) -> Self {
        PyAabb2d {
            storage: ValueStorage::owned(aabb),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Aabb2d>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Aabb2d>> {
        Ok(self.storage.as_mut()?)
    }
}

fn check_min_max(min: Vec2, max: Vec2) -> PyResult<()> {
    if !max.cmpge(min).all() {
        return Err(PyValueError::new_err(aabb_min_exceeds_max("Aabb2d")));
    }
    Ok(())
}

#[pymethods]
impl PyAabb2d {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Aabb2d::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    #[pyo3(signature = (center, half_size))]
    pub fn new(center: PyVec2, half_size: PyVec2) -> PyResult<Self> {
        let center_vec: Vec2 = center.try_into()?;
        let half_size_vec: Vec2 = half_size.try_into()?;
        let half_size_vec = check_for::<Self, _>("half_size", half_size_vec, NON_NEGATIVE)?;
        Ok(PyAabb2d::from_aabb2d(Aabb2d::new(
            center_vec,
            half_size_vec,
        )))
    }

    #[getter]
    pub fn min(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|a| &a.min)?)
    }

    #[setter]
    pub fn set_min(&mut self, value: PyVec2) -> PyResult<()> {
        let min: Vec2 = value.try_into()?;
        check_min_max(min, self.as_ref()?.max)?;
        self.as_mut()?.min = min;
        Ok(())
    }

    #[getter]
    pub fn max(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|a| &a.max)?)
    }

    #[setter]
    pub fn set_max(&mut self, value: PyVec2) -> PyResult<()> {
        let max: Vec2 = value.try_into()?;
        check_min_max(self.as_ref()?.min, max)?;
        self.as_mut()?.max = max;
        Ok(())
    }

    pub fn center(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.center()))
    }

    pub fn half_size(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.half_size()))
    }

    pub fn closest_point(&self, point: PyVec2) -> PyResult<PyVec2> {
        let point_vec: Vec2 = point.try_into()?;
        Ok(PyVec2::from_vec2(self.as_ref()?.closest_point(point_vec)))
    }

    pub fn contains(&self, other: &PyAabb2d) -> PyResult<bool> {
        let other_aabb: Aabb2d = other.try_into()?;
        Ok(self.as_ref()?.contains(&other_aabb))
    }

    /// True if this Aabb2d intersects an Aabb2d or a BoundingCircle.
    pub fn intersects(&self, volume: &Bound<'_, PyAny>) -> PyResult<bool> {
        intersects_2d(&*self.as_ref()?, volume)
    }

    pub fn merge(&self, other: &PyAabb2d) -> PyResult<PyAabb2d> {
        let other_aabb: Aabb2d = other.try_into()?;
        Ok(PyAabb2d::from_aabb2d(self.as_ref()?.merge(&other_aabb)))
    }

    pub fn rotated_by(&self, rotation: &Bound<'_, PyAny>) -> PyResult<PyAabb2d> {
        let rotation = extract_rot2_from_any(rotation)?;
        let bounds = self.as_ref()?;
        let half_size = Mat2::from(rotation).abs() * bounds.half_size();
        if !half_size.cmpge(Vec2::ZERO).all() {
            return Err(PyValueError::new_err(bounding_operation(
                "Aabb2d",
                "rotated_by",
            )));
        }
        Ok((*bounds).rotated_by(rotation).into())
    }

    pub fn transformed_by(
        &self,
        translation: &Bound<'_, PyAny>,
        rotation: &Bound<'_, PyAny>,
    ) -> PyResult<PyAabb2d> {
        let translation = extract_vec2_from_any(translation)?;
        let rotation = extract_rot2_from_any(rotation)?;
        let bounds = self.as_ref()?;
        let half_size = Mat2::from(rotation).abs() * bounds.half_size();
        if !half_size.cmpge(Vec2::ZERO).all() {
            return Err(PyValueError::new_err(bounding_operation(
                "Aabb2d",
                "transformed_by",
            )));
        }
        Ok((*bounds).transformed_by(translation, rotation).into())
    }

    pub fn grow(&self, amount: PyVec2) -> PyResult<PyAabb2d> {
        let amount: Vec2 = amount.try_into()?;
        let bounds = self.as_ref()?;
        if !(bounds.max + amount).cmpge(bounds.min - amount).all() {
            return Err(PyValueError::new_err(bounding_operation("Aabb2d", "grow")));
        }
        Ok(PyAabb2d::from_aabb2d(bounds.grow(amount)))
    }

    pub fn shrink(&self, amount: PyVec2) -> PyResult<PyAabb2d> {
        let amount: Vec2 = amount.try_into()?;
        let bounds = self.as_ref()?;
        if !(bounds.max - amount).cmpge(bounds.min + amount).all() {
            return Err(PyValueError::new_err(bounding_shrink("Aabb2d")));
        }
        Ok(PyAabb2d::from_aabb2d(bounds.shrink(amount)))
    }

    pub fn scale_around_center(&self, scale: PyVec2) -> PyResult<PyAabb2d> {
        let scale: Vec2 = scale.try_into()?;
        let bounds = self.as_ref()?;
        let half_size = bounds.half_size() * scale;
        let center = bounds.center();
        if !(center + half_size).cmpge(center - half_size).all() {
            return Err(PyValueError::new_err(bounding_operation(
                "Aabb2d",
                "scale_around_center",
            )));
        }
        Ok(PyAabb2d::from_aabb2d(bounds.scale_around_center(scale)))
    }

    pub fn visible_area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.visible_area())
    }

    pub fn bounding_circle(&self) -> PyResult<PyBoundingCircle> {
        let bounds = self.as_ref()?;
        let radius = bounds.min.distance(bounds.max) / 2.0;
        if radius.is_nan() || radius < 0.0 {
            return Err(PyValueError::new_err(bounding_operation(
                "Aabb2d",
                "bounding_circle",
            )));
        }
        Ok(PyBoundingCircle::from_bounding_circle(
            bounds.bounding_circle(),
        ))
    }

    #[staticmethod]
    pub fn from_point_cloud(isometry: PyIsometry2d, points: Vec<PyVec2>) -> PyResult<PyAabb2d> {
        if points.is_empty() {
            return Err(PyValueError::new_err(EMPTY_POINT_CLOUD));
        }
        let iso: Isometry2d = isometry.into();
        let point_refs: Vec<Vec2> = points
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;

        Ok(PyAabb2d::from_aabb2d(Aabb2d::from_point_cloud(
            iso,
            &point_refs,
        )))
    }

    pub fn __eq__(&self, other: &PyAabb2d) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    fn __repr__(&self) -> PyResult<String> {
        let aabb = self.as_ref()?;
        Ok(format!(
            "Aabb2d(center={}, half_size={})",
            aabb.center().repr_literal(),
            aabb.half_size().repr_literal()
        ))
    }

    pub fn translated_by(&self, translation: &Bound<'_, PyAny>) -> PyResult<PyAabb2d> {
        let translation = extract_vec2_from_any(translation)?;
        Ok(Aabb2d::try_from(self)?.translated_by(translation).into())
    }

    pub fn translate_by(&mut self, translation: &Bound<'_, PyAny>) -> PyResult<()> {
        let translation = extract_vec2_from_any(translation)?;
        self.as_mut()?.translate_by(translation);
        Ok(())
    }

    pub fn rotate_by(&mut self, rotation: &Bound<'_, PyAny>) -> PyResult<()> {
        let rotated = Aabb2d::try_from(&self.rotated_by(rotation)?)?;
        *self.as_mut()? = rotated;
        Ok(())
    }

    pub fn transform_by(
        &mut self,
        translation: &Bound<'_, PyAny>,
        rotation: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let transformed = Aabb2d::try_from(&self.transformed_by(translation, rotation)?)?;
        *self.as_mut()? = transformed;
        Ok(())
    }
}

pub(crate) fn intersects_2d<V>(value: &V, volume: &Bound<'_, PyAny>) -> PyResult<bool>
where
    V: IntersectsVolume<Aabb2d> + IntersectsVolume<BoundingCircle>,
{
    if let Ok(aabb) = volume.cast::<PyAabb2d>() {
        let aabb = Aabb2d::try_from(&*aabb.try_borrow()?)?;
        return Ok(value.intersects(&aabb));
    }
    if let Ok(circle) = volume.cast::<PyBoundingCircle>() {
        let circle = BoundingCircle::try_from(&*circle.try_borrow()?)?;
        return Ok(value.intersects(&circle));
    }
    Err(PyTypeError::new_err(intersects_volume_type(
        "Aabb2d or BoundingCircle",
    )))
}
