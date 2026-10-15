use bevy::math::bounding::{BoundingCircleCast, IntersectsVolume};
use pybevy_core::{
    FieldStorage, FromBorrowedStorage,
    repr::{ReprLiteral, float_repr, object_repr},
};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

use super::{bounding_circle::PyBoundingCircle, raycast::PyRayCast2d};
use crate::{dir2::PyDir2, ray::PyRay2d, vec2::PyVec2};

#[pyfield]
#[pyclass(name = "BoundingCircleCast", module = "pybevy.math", from_py_object)]
#[derive(Debug)]
pub struct PyBoundingCircleCast {
    storage: FieldStorage<BoundingCircleCast>,
}

#[pymethods]
impl PyBoundingCircleCast {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(BoundingCircleCast::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    pub fn new(
        circle: &PyBoundingCircle,
        origin: PyVec2,
        direction: PyDir2,
        max: f32,
    ) -> PyResult<Self> {
        Ok(Self::from_owned(BoundingCircleCast::new(
            circle.try_into()?,
            origin.try_into()?,
            direction.try_into()?,
            max,
        )))
    }

    #[staticmethod]
    pub fn from_ray(circle: &PyBoundingCircle, ray: &PyRay2d, max: f32) -> PyResult<Self> {
        Ok(Self::from_owned(BoundingCircleCast::from_ray(
            circle.try_into()?,
            ray.try_into()?,
            max,
        )))
    }

    pub fn circle_collision_at(&self, circle: &PyBoundingCircle) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.circle_collision_at(circle.try_into()?))
    }

    pub fn intersects(&self, volume: &PyBoundingCircle) -> PyResult<bool> {
        Ok(self.as_ref()?.intersects(&volume.try_into()?))
    }

    #[getter]
    pub fn circle(&self) -> PyResult<PyBoundingCircle> {
        Ok(self.storage.borrow_field_as(|cast| &cast.circle)?)
    }

    #[setter]
    pub fn set_circle(&mut self, circle: &PyBoundingCircle) -> PyResult<()> {
        self.as_mut()?.circle = circle.try_into()?;
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
            "BoundingCircleCast(circle={}, origin={}, direction={}, max={})",
            object_repr(PyBoundingCircle::from(cast.circle)),
            cast.ray.ray.origin.repr_literal(),
            PyDir2::from(cast.ray.ray.direction).__repr__()?,
            float_repr(cast.ray.max)
        ))
    }
}
