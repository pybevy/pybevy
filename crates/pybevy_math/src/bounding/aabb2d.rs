use bevy::math::{
    Isometry2d, Vec2,
    bounding::{Aabb2d, BoundingVolume, IntersectsVolume},
};
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pyo3::prelude::*;

use super::bounding_circle::PyBoundingCircle;
use crate::vec2::PyVec2;

#[pyclass(name = "Aabb2d", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAabb2d {
    storage: ValueStorage<Aabb2d>,
}

impl From<PyAabb2d> for Aabb2d {
    #[inline(always)]
    fn from(py_aabb: PyAabb2d) -> Self {
        match py_aabb.storage.get() {
            Ok(val) => val,
            Err(_) => Aabb2d::new(Vec2::ZERO, Vec2::ZERO),
        }
    }
}

impl From<&PyAabb2d> for Aabb2d {
    #[inline(always)]
    fn from(py_aabb: &PyAabb2d) -> Self {
        match py_aabb.storage.get() {
            Ok(val) => val,
            Err(_) => Aabb2d::new(Vec2::ZERO, Vec2::ZERO),
        }
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

#[pymethods]
impl PyAabb2d {
    #[new]
    #[pyo3(signature = (center, half_size))]
    pub fn new(center: PyVec2, half_size: PyVec2) -> PyResult<Self> {
        let center_vec: Vec2 = center.try_into()?;
        let half_size_vec: Vec2 = half_size.try_into()?;
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
        self.as_mut()?.min = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn max(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|a| &a.max)?)
    }

    #[setter]
    pub fn set_max(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.max = value.try_into()?;
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
        let other_aabb: Aabb2d = other.into();
        Ok(self.as_ref()?.contains(&other_aabb))
    }

    pub fn merge(&self, other: &PyAabb2d) -> PyResult<PyAabb2d> {
        let other_aabb: Aabb2d = other.into();
        Ok(PyAabb2d::from_aabb2d(self.as_ref()?.merge(&other_aabb)))
    }

    pub fn grow(&self, amount: PyVec2) -> PyResult<PyAabb2d> {
        let amount_vec: Vec2 = amount.try_into()?;
        Ok(PyAabb2d::from_aabb2d(self.as_ref()?.grow(amount_vec)))
    }

    pub fn shrink(&self, amount: PyVec2) -> PyResult<PyAabb2d> {
        let amount_vec: Vec2 = amount.try_into()?;
        Ok(PyAabb2d::from_aabb2d(self.as_ref()?.shrink(amount_vec)))
    }

    pub fn scale_around_center(&self, scale: PyVec2) -> PyResult<PyAabb2d> {
        let scale_vec: Vec2 = scale.try_into()?;
        Ok(PyAabb2d::from_aabb2d(
            self.as_ref()?.scale_around_center(scale_vec),
        ))
    }

    pub fn visible_area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.visible_area())
    }

    pub fn bounding_circle(&self) -> PyResult<PyBoundingCircle> {
        Ok(PyBoundingCircle::from_bounding_circle(
            self.as_ref()?.bounding_circle(),
        ))
    }

    pub fn intersects_aabb(&self, other: &PyAabb2d) -> PyResult<bool> {
        let other_aabb: Aabb2d = other.into();
        Ok(self.as_ref()?.intersects(&other_aabb))
    }

    pub fn intersects_circle(&self, circle: &PyBoundingCircle) -> PyResult<bool> {
        let bounding_circle = circle.to_bounding_circle();
        Ok(self.as_ref()?.intersects(&bounding_circle))
    }

    #[staticmethod]
    pub fn from_point_cloud(isometry: PyIsometry2d, points: Vec<PyVec2>) -> PyResult<PyAabb2d> {
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
        Ok(format!("Aabb2d(min={:?}, max={:?})", aabb.min, aabb.max))
    }
}

#[pyclass(name = "Isometry2d", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyIsometry2d {
    inner: Isometry2d,
}

impl From<PyIsometry2d> for Isometry2d {
    fn from(iso: PyIsometry2d) -> Self {
        iso.inner
    }
}

impl From<Isometry2d> for PyIsometry2d {
    fn from(iso: Isometry2d) -> Self {
        PyIsometry2d { inner: iso }
    }
}

#[pymethods]
impl PyIsometry2d {
    #[classattr]
    const IDENTITY: PyIsometry2d = PyIsometry2d {
        inner: Isometry2d::IDENTITY,
    };

    #[new]
    #[pyo3(signature = (translation = PyVec2::ZERO, rotation = None))]
    pub fn new(translation: PyVec2, rotation: Option<&crate::rot2::PyRot2>) -> PyResult<Self> {
        let rot = rotation
            .map(|rotation| rotation.inner())
            .transpose()?
            .unwrap_or(bevy::math::Rot2::IDENTITY);
        Ok(PyIsometry2d {
            inner: Isometry2d::new(translation.try_into()?, rot),
        })
    }

    #[staticmethod]
    pub fn from_rotation(rotation: &crate::rot2::PyRot2) -> PyResult<Self> {
        Ok(PyIsometry2d {
            inner: Isometry2d::from_rotation(rotation.inner()?),
        })
    }

    #[staticmethod]
    pub fn from_translation(translation: PyVec2) -> PyResult<Self> {
        Ok(PyIsometry2d {
            inner: Isometry2d::from_translation(translation.try_into()?),
        })
    }

    #[staticmethod]
    pub fn from_xy(x: f32, y: f32) -> Self {
        PyIsometry2d {
            inner: Isometry2d::from_xy(x, y),
        }
    }

    #[getter]
    pub fn rotation(&self) -> crate::rot2::PyRot2 {
        self.inner.rotation.into()
    }

    #[getter]
    pub fn translation(&self) -> PyVec2 {
        PyVec2::from_vec2(self.inner.translation)
    }

    pub fn inverse(&self) -> Self {
        PyIsometry2d {
            inner: self.inner.inverse(),
        }
    }

    pub fn inverse_mul(&self, rhs: &PyIsometry2d) -> Self {
        PyIsometry2d {
            inner: self.inner.inverse_mul(rhs.inner),
        }
    }

    pub fn transform_point(&self, point: PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.inner.transform_point(point.try_into()?),
        ))
    }

    pub fn inverse_transform_point(&self, point: PyVec2) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(
            self.inner.inverse_transform_point(point.try_into()?),
        ))
    }

    fn __repr__(&self) -> String {
        format!(
            "Isometry2d(translation=Vec2({}, {}), rotation=Rot2({}))",
            self.inner.translation.x,
            self.inner.translation.y,
            self.inner.rotation.as_radians()
        )
    }
}
