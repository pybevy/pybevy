use bevy::{
    math::Dir3,
    shape::{AabbCast3d, IntersectsVolume},
};
use pybevy_core::{
    FieldStorage, FromBorrowedStorage,
    repr::{ReprLiteral, float_repr, object_repr},
};
use pybevy_macros::pyfield;
use pybevy_math::{dir3::PyDir3, vec3a::extract_vec3a_from_any};
use pyo3::prelude::*;

use super::{aabb3d::PyAabb3d, raycast::PyRayCast3d};
use crate::ray::PyRay3d;

#[pyfield]
#[pyclass(name = "AabbCast3d", module = "pybevy.shape", from_py_object)]
#[derive(Debug)]
pub struct PyAabbCast3d {
    storage: FieldStorage<AabbCast3d>,
}

#[pymethods]
impl PyAabbCast3d {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(AabbCast3d::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    pub fn new(
        aabb: &PyAabb3d,
        origin: &Bound<'_, PyAny>,
        direction: PyDir3,
        max: f32,
    ) -> PyResult<Self> {
        Ok(Self::from_owned(AabbCast3d::new(
            aabb.try_into()?,
            extract_vec3a_from_any(origin)?,
            Dir3::try_from(direction)?,
            max,
        )))
    }

    #[staticmethod]
    pub fn from_ray(aabb: &PyAabb3d, ray: &PyRay3d, max: f32) -> PyResult<Self> {
        Ok(Self::from_owned(AabbCast3d::from_ray(
            aabb.try_into()?,
            ray.try_into()?,
            max,
        )))
    }

    pub fn aabb_collision_at(&self, aabb: &PyAabb3d) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.aabb_collision_at(aabb.try_into()?))
    }

    pub fn intersects(&self, volume: &PyAabb3d) -> PyResult<bool> {
        Ok(self.as_ref()?.intersects(&volume.try_into()?))
    }

    #[getter]
    pub fn aabb(&self) -> PyResult<PyAabb3d> {
        Ok(self.storage.borrow_field_as(|cast| &cast.aabb)?)
    }

    #[setter]
    pub fn set_aabb(&mut self, aabb: &PyAabb3d) -> PyResult<()> {
        let aabb = aabb.try_into()?;
        self.storage.update(|cast| cast.aabb = aabb)?;
        Ok(())
    }

    #[getter]
    pub fn ray(&self) -> PyResult<PyRayCast3d> {
        Ok(self.storage.borrow_field_as(|cast| &cast.ray)?)
    }

    #[setter]
    pub fn set_ray(&mut self, ray: &PyRayCast3d) -> PyResult<()> {
        let ray = ray.try_into()?;
        self.storage.update(|cast| cast.ray.clone_from(&ray))?;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let cast = self.as_ref()?;
        Ok(format!(
            "AabbCast3d(aabb={}, origin={}, direction={}, max={})",
            object_repr(PyAabb3d::from(cast.aabb)),
            cast.ray.origin.repr_literal(),
            PyDir3::from(Dir3::from(cast.ray.direction)).__repr__()?,
            float_repr(cast.ray.max)
        ))
    }
}
