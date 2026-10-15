use bevy::math::{
    Dir3,
    bounding::{BoundingSphereCast, IntersectsVolume},
};
use pybevy_core::{
    FieldStorage, FromBorrowedStorage,
    repr::{ReprLiteral, float_repr, object_repr},
};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

use super::{bounding_sphere::PyBoundingSphere, raycast::PyRayCast3d};
use crate::{dir3::PyDir3, ray::PyRay3d, vec3a::extract_vec3a_from_any};

#[pyfield]
#[pyclass(name = "BoundingSphereCast", module = "pybevy.math", from_py_object)]
#[derive(Debug)]
pub struct PyBoundingSphereCast {
    storage: FieldStorage<BoundingSphereCast>,
}

#[pymethods]
impl PyBoundingSphereCast {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(BoundingSphereCast::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    pub fn new(
        sphere: &PyBoundingSphere,
        origin: &Bound<'_, PyAny>,
        direction: PyDir3,
        max: f32,
    ) -> PyResult<Self> {
        Ok(Self::from_owned(BoundingSphereCast::new(
            sphere.try_into()?,
            extract_vec3a_from_any(origin)?,
            Dir3::try_from(direction)?,
            max,
        )))
    }

    #[staticmethod]
    pub fn from_ray(sphere: &PyBoundingSphere, ray: &PyRay3d, max: f32) -> PyResult<Self> {
        Ok(Self::from_owned(BoundingSphereCast::from_ray(
            sphere.try_into()?,
            ray.try_into()?,
            max,
        )))
    }

    pub fn sphere_collision_at(&self, sphere: &PyBoundingSphere) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.sphere_collision_at(sphere.try_into()?))
    }

    pub fn intersects(&self, volume: &PyBoundingSphere) -> PyResult<bool> {
        Ok(self.as_ref()?.intersects(&volume.try_into()?))
    }

    #[getter]
    pub fn sphere(&self) -> PyResult<PyBoundingSphere> {
        Ok(self.storage.borrow_field_as(|cast| &cast.sphere)?)
    }

    #[setter]
    pub fn set_sphere(&mut self, sphere: &PyBoundingSphere) -> PyResult<()> {
        self.as_mut()?.sphere = sphere.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn ray(&self) -> PyResult<PyRayCast3d> {
        Ok(self.storage.borrow_field_as(|cast| &cast.ray)?)
    }

    #[setter]
    pub fn set_ray(&mut self, ray: &PyRayCast3d) -> PyResult<()> {
        self.as_mut()?.ray = ray.try_into()?;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let cast = self.as_ref()?;
        Ok(format!(
            "BoundingSphereCast(sphere={}, origin={}, direction={}, max={})",
            object_repr(PyBoundingSphere::from(cast.sphere)),
            cast.ray.origin.repr_literal(),
            PyDir3::from(Dir3::from(cast.ray.direction)).__repr__()?,
            float_repr(cast.ray.max)
        ))
    }
}
