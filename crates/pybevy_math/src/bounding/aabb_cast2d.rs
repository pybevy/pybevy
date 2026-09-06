use bevy::math::bounding::AabbCast2d;
use pybevy_core::{FieldStorage, FromBorrowedStorage};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

use super::{aabb2d::PyAabb2d, raycast::PyRayCast2d};
use crate::{dir2::PyDir2, ray::PyRay2d, vec2::PyVec2};

#[pyfield]
#[pyclass(name = "AabbCast2d", module = "pybevy.math", from_py_object)]
#[derive(Debug)]
pub struct PyAabbCast2d {
    storage: FieldStorage<AabbCast2d>,
}

#[pymethods]
impl PyAabbCast2d {
    #[new]
    pub fn new(aabb: &PyAabb2d, origin: PyVec2, direction: PyDir2, max: f32) -> PyResult<Self> {
        Ok(Self::from_owned(AabbCast2d::new(
            aabb.try_into()?,
            origin.try_into()?,
            direction.try_into()?,
            max,
        )))
    }

    #[staticmethod]
    pub fn from_ray(aabb: &PyAabb2d, ray: &PyRay2d, max: f32) -> PyResult<Self> {
        Ok(Self::from_owned(AabbCast2d::from_ray(
            aabb.try_into()?,
            ray.try_into()?,
            max,
        )))
    }

    pub fn aabb_collision_at(&self, aabb: &PyAabb2d) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.aabb_collision_at(aabb.try_into()?))
    }

    #[getter]
    pub fn aabb(&self) -> PyResult<PyAabb2d> {
        Ok(self.storage.borrow_field_as(|cast| &cast.aabb)?)
    }

    #[setter]
    pub fn set_aabb(&mut self, aabb: &PyAabb2d) -> PyResult<()> {
        self.as_mut()?.aabb = aabb.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn ray(&self) -> PyResult<PyRayCast2d> {
        Ok(self.storage.borrow_field_as(|cast| &cast.ray)?)
    }

    #[setter]
    pub fn set_ray(&mut self, ray: &PyRayCast2d) -> PyResult<()> {
        self.as_mut()?.ray = ray.try_into()?;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let cast = self.as_ref()?;
        Ok(format!(
            "AabbCast2d(aabb={:?}, ray=({:?}, max={}))",
            cast.aabb, cast.ray.ray, cast.ray.max
        ))
    }
}
