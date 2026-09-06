use bevy::math::bounding::BoundingCircleCast;
use pybevy_core::{FieldStorage, FromBorrowedStorage};
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
            "BoundingCircleCast(circle={:?}, ray=({:?}, max={}))",
            cast.circle, cast.ray.ray, cast.ray.max
        ))
    }
}
