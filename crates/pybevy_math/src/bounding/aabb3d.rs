use bevy::math::{
    Isometry3d, Mat3, Vec3, Vec3A,
    bounding::{Aabb3d, BoundingSphere, BoundingVolume, IntersectsVolume},
};
use pybevy_core::{
    FromBorrowedStorage, ValueStorage,
    public_error::{
        AABB3D_MIN_MAX, EMPTY_POINT_CLOUD, bounding_half_size, bounding_operation, bounding_shrink,
        intersects_volume_type,
    },
    repr::ReprLiteral,
};
use pybevy_macros::pyvalue;
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use super::bounding_sphere::PyBoundingSphere;
use crate::{
    isometry3d::PyIsometry3d,
    quat::extract_quat_from_any,
    vec3::PyVec3,
    vec3a::{PyVec3A, extract_vec3a_from_any},
};

#[pyvalue]
#[pyclass(name = "Aabb3d", module = "pybevy.math", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAabb3d {
    storage: ValueStorage<Aabb3d>,
}

impl TryFrom<PyAabb3d> for Aabb3d {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_aabb: PyAabb3d) -> PyResult<Self> {
        Ok(py_aabb.storage.get()?)
    }
}

impl TryFrom<&PyAabb3d> for Aabb3d {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_aabb: &PyAabb3d) -> PyResult<Self> {
        Ok(py_aabb.storage.get()?)
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
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(Aabb3d::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    #[pyo3(signature = (center, half_size))]
    pub fn new(center: &Bound<'_, PyAny>, half_size: &Bound<'_, PyAny>) -> PyResult<Self> {
        let half_size = extract_vec3a_from_any(half_size)?;
        if !half_size.cmpge(Vec3A::ZERO).all() {
            return Err(PyValueError::new_err(bounding_half_size(
                "Aabb3d",
                &[half_size.x, half_size.y, half_size.z],
            )));
        }
        Ok(PyAabb3d::from_owned(Aabb3d::new(
            extract_vec3a_from_any(center)?,
            half_size,
        )))
    }

    #[staticmethod]
    pub fn from_min_max(min: &Bound<'_, PyAny>, max: &Bound<'_, PyAny>) -> PyResult<Self> {
        let min = extract_vec3a_from_any(min)?;
        let max = extract_vec3a_from_any(max)?;
        if !max.cmpge(min).all() {
            return Err(PyValueError::new_err(AABB3D_MIN_MAX));
        }
        Ok(PyAabb3d::from_owned(Aabb3d::from_min_max(min, max)))
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

    pub fn center(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.center()))
    }

    pub fn half_size(&self) -> PyResult<PyVec3A> {
        Ok(PyVec3A::from_vec3a(self.as_ref()?.half_size()))
    }

    pub fn closest_point(&self, point: &Bound<'_, PyAny>) -> PyResult<PyVec3A> {
        let point = extract_vec3a_from_any(point)?;
        Ok(PyVec3A::from_vec3a(self.as_ref()?.closest_point(point)))
    }

    pub fn contains(&self, other: &PyAabb3d) -> PyResult<bool> {
        let other_aabb: Aabb3d = other.try_into()?;
        Ok(self.as_ref()?.contains(&other_aabb))
    }

    /// True if this Aabb3d intersects an Aabb3d or a BoundingSphere.
    pub fn intersects(&self, volume: &Bound<'_, PyAny>) -> PyResult<bool> {
        intersects_3d(&*self.as_ref()?, volume)
    }

    pub fn merge(&self, other: &PyAabb3d) -> PyResult<PyAabb3d> {
        let other_aabb: Aabb3d = other.try_into()?;
        Ok(PyAabb3d::from_owned(self.as_ref()?.merge(&other_aabb)))
    }

    pub fn rotated_by(&self, rotation: &Bound<'_, PyAny>) -> PyResult<PyAabb3d> {
        let rotation = extract_quat_from_any(rotation)?;
        let bounds = self.as_ref()?;
        let half_size = Mat3::from_quat(rotation).abs() * bounds.half_size();
        if !half_size.cmpge(Vec3A::ZERO).all() {
            return Err(PyValueError::new_err(bounding_operation(
                "Aabb3d",
                "rotated_by",
            )));
        }
        Ok(PyAabb3d::from_owned((*bounds).rotated_by(rotation)))
    }

    pub fn transformed_by(
        &self,
        translation: &Bound<'_, PyAny>,
        rotation: &Bound<'_, PyAny>,
    ) -> PyResult<PyAabb3d> {
        let translation = extract_vec3a_from_any(translation)?;
        let rotation = extract_quat_from_any(rotation)?;
        let bounds = self.as_ref()?;
        let half_size = Mat3::from_quat(rotation).abs() * bounds.half_size();
        if !half_size.cmpge(Vec3A::ZERO).all() {
            return Err(PyValueError::new_err(bounding_operation(
                "Aabb3d",
                "transformed_by",
            )));
        }
        Ok(PyAabb3d::from_owned(
            (*bounds).transformed_by(translation, rotation),
        ))
    }

    pub fn grow(&self, amount: &Bound<'_, PyAny>) -> PyResult<PyAabb3d> {
        let amount = extract_vec3a_from_any(amount)?;
        let bounds = self.as_ref()?;
        if !(bounds.max + amount).cmpge(bounds.min - amount).all() {
            return Err(PyValueError::new_err(bounding_operation("Aabb3d", "grow")));
        }
        Ok(PyAabb3d::from_owned(bounds.grow(amount)))
    }

    pub fn shrink(&self, amount: &Bound<'_, PyAny>) -> PyResult<PyAabb3d> {
        let amount = extract_vec3a_from_any(amount)?;
        let bounds = self.as_ref()?;
        if !(bounds.max - amount).cmpge(bounds.min + amount).all() {
            return Err(PyValueError::new_err(bounding_shrink("Aabb3d")));
        }
        Ok(PyAabb3d::from_owned(bounds.shrink(amount)))
    }

    pub fn scale_around_center(&self, scale: &Bound<'_, PyAny>) -> PyResult<PyAabb3d> {
        let scale = extract_vec3a_from_any(scale)?;
        let bounds = self.as_ref()?;
        let half_size = bounds.half_size() * scale;
        let center = bounds.center();
        if !(center + half_size).cmpge(center - half_size).all() {
            return Err(PyValueError::new_err(bounding_operation(
                "Aabb3d",
                "scale_around_center",
            )));
        }
        Ok(PyAabb3d::from_owned(bounds.scale_around_center(scale)))
    }

    pub fn visible_area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.visible_area())
    }

    pub fn bounding_sphere(&self) -> PyResult<PyBoundingSphere> {
        let bounds = self.as_ref()?;
        let radius = bounds.min.distance(bounds.max) / 2.0;
        if radius.is_nan() || radius < 0.0 {
            return Err(PyValueError::new_err(bounding_operation(
                "Aabb3d",
                "bounding_sphere",
            )));
        }
        Ok(PyBoundingSphere::from_owned(bounds.bounding_sphere()))
    }

    #[staticmethod]
    pub fn from_point_cloud(isometry: PyIsometry3d, points: Vec<PyVec3>) -> PyResult<PyAabb3d> {
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
        Ok(format!(
            "Aabb3d.from_min_max({}, {})",
            aabb.min.repr_literal(),
            aabb.max.repr_literal()
        ))
    }

    pub fn translated_by(&self, translation: &Bound<'_, PyAny>) -> PyResult<PyAabb3d> {
        let translation = extract_vec3a_from_any(translation)?;
        Ok(Aabb3d::try_from(self)?.translated_by(translation).into())
    }

    pub fn translate_by(&mut self, translation: &Bound<'_, PyAny>) -> PyResult<()> {
        let translation = extract_vec3a_from_any(translation)?;
        self.as_mut()?.translate_by(translation);
        Ok(())
    }

    pub fn rotate_by(&mut self, rotation: &Bound<'_, PyAny>) -> PyResult<()> {
        let rotated = Aabb3d::try_from(&self.rotated_by(rotation)?)?;
        *self.as_mut()? = rotated;
        Ok(())
    }

    pub fn transform_by(
        &mut self,
        translation: &Bound<'_, PyAny>,
        rotation: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let transformed = Aabb3d::try_from(&self.transformed_by(translation, rotation)?)?;
        *self.as_mut()? = transformed;
        Ok(())
    }
}

pub(crate) fn intersects_3d<V>(value: &V, volume: &Bound<'_, PyAny>) -> PyResult<bool>
where
    V: IntersectsVolume<Aabb3d> + IntersectsVolume<BoundingSphere>,
{
    if let Ok(aabb) = volume.cast::<PyAabb3d>() {
        let aabb = Aabb3d::try_from(&*aabb.try_borrow()?)?;
        return Ok(value.intersects(&aabb));
    }
    if let Ok(sphere) = volume.cast::<PyBoundingSphere>() {
        let sphere = BoundingSphere::try_from(&*sphere.try_borrow()?)?;
        return Ok(value.intersects(&sphere));
    }
    Err(PyTypeError::new_err(intersects_volume_type(
        "Aabb3d or BoundingSphere",
    )))
}
