use bevy::math::{
    Dir3, Vec2, Vec3,
    bounding::{Aabb2d, Aabb3d, BoundingCircle, BoundingSphere, RayCast2d, RayCast3d},
};
use pybevy_core::{
    FieldStorage, FromBorrowedStorage, ValueStorage,
    repr::{ReprLiteral, float_repr, object_repr},
};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

use super::{
    aabb2d::{PyAabb2d, intersects_2d},
    aabb3d::{PyAabb3d, intersects_3d},
    bounding_circle::PyBoundingCircle,
    bounding_sphere::PyBoundingSphere,
};
use crate::{
    dir2::PyDir2,
    dir3::PyDir3,
    ray::{PyRay2d, PyRay3d},
    vec2::PyVec2,
    vec3::PyVec3,
    vec3a::{PyVec3A, extract_vec3a_from_any},
};

#[pyfield]
#[pyclass(name = "RayCast2d", module = "pybevy.math", skip_from_py_object)]
#[derive(Debug)]
pub struct PyRayCast2d {
    storage: FieldStorage<RayCast2d>,
}

#[pymethods]
impl PyRayCast2d {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(RayCast2d::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    pub fn new(origin: PyVec2, direction: PyDir2, max: f32) -> PyResult<Self> {
        let origin_vec: Vec2 = origin.try_into()?;
        let dir = direction.get()?;
        Ok(Self::from_owned(RayCast2d::new(origin_vec, dir, max)))
    }

    #[staticmethod]
    pub fn from_ray(ray: &PyRay2d, max: f32) -> PyResult<Self> {
        Ok(Self::from_owned(RayCast2d::from_ray(ray.to_ray2d()?, max)))
    }

    #[getter]
    pub fn ray(&self) -> PyResult<PyRay2d> {
        Ok(self.storage.borrow_field_as(|cast| &cast.ray)?)
    }

    #[setter]
    pub fn set_ray(&mut self, ray: &PyRay2d) -> PyResult<()> {
        // Rebuild via from_ray so the cached direction reciprocal stays valid.
        let max = self.as_ref()?.max;
        self.as_mut()?
            .clone_from(&RayCast2d::from_ray(ray.to_ray2d()?, max));
        Ok(())
    }

    #[getter]
    pub fn max(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max)
    }

    #[setter]
    pub fn set_max(&mut self, max: f32) -> PyResult<()> {
        self.as_mut()?.max = max;
        Ok(())
    }

    /// True if the ray hits an Aabb2d or a BoundingCircle within `max`.
    pub fn intersects(&self, volume: &Bound<'_, PyAny>) -> PyResult<bool> {
        intersects_2d(&*self.as_ref()?, volume)
    }

    pub fn aabb_intersection_at(&self, aabb: &PyAabb2d) -> PyResult<Option<f32>> {
        let aabb_2d: Aabb2d = aabb.try_into()?;
        Ok(self.as_ref()?.aabb_intersection_at(&aabb_2d))
    }

    pub fn circle_intersection_at(&self, circle: &PyBoundingCircle) -> PyResult<Option<f32>> {
        let bounding_circle: BoundingCircle = circle.try_into()?;
        Ok(self.as_ref()?.circle_intersection_at(&bounding_circle))
    }

    pub fn direction_recip(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.direction_recip()))
    }

    fn __repr__(&self) -> PyResult<String> {
        let cast = self.as_ref()?;
        Ok(format!(
            "RayCast2d(ray={}, max={})",
            object_repr(PyRay2d::from(cast.ray)),
            float_repr(cast.max)
        ))
    }
}

#[pyfield]
#[pyclass(name = "RayCast3d", module = "pybevy.math", skip_from_py_object)]
#[derive(Debug)]
pub struct PyRayCast3d {
    storage: FieldStorage<RayCast3d>,
}

#[pymethods]
impl PyRayCast3d {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from(RayCast3d::try_from(self)?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    pub fn new(origin: PyVec3, direction: PyDir3, max: f32) -> PyResult<Self> {
        let origin_vec: Vec3 = origin.try_into()?;
        let dir = direction.get()?;
        Ok(Self::from_owned(RayCast3d::new(origin_vec, dir, max)))
    }

    #[staticmethod]
    pub fn from_ray(ray: &PyRay3d, max: f32) -> PyResult<Self> {
        Ok(Self::from_owned(RayCast3d::from_ray(ray.to_ray3d()?, max)))
    }

    #[getter]
    pub fn origin(&self) -> PyResult<PyVec3A> {
        Ok(self.storage.borrow_field_as(|cast| &cast.origin)?)
    }

    #[setter]
    pub fn set_origin(&mut self, origin: &Bound<'_, PyAny>) -> PyResult<()> {
        let origin = extract_vec3a_from_any(origin)?;
        let cast = self.as_ref()?.clone();
        self.as_mut()?
            .clone_from(&RayCast3d::new(origin, cast.direction, cast.max));
        Ok(())
    }

    #[getter]
    pub fn direction(&self) -> PyResult<PyDir3> {
        let direction: Dir3 = self.as_ref()?.direction.into();
        Ok(PyDir3::from_borrowed(ValueStorage::read_only_snapshot(
            direction,
        )))
    }

    #[setter]
    pub fn set_direction(&mut self, direction: &PyDir3) -> PyResult<()> {
        let direction = direction.get()?;
        let cast = self.as_ref()?.clone();
        self.as_mut()?
            .clone_from(&RayCast3d::new(cast.origin, direction, cast.max));
        Ok(())
    }

    #[getter]
    pub fn max(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max)
    }

    #[setter]
    pub fn set_max(&mut self, max: f32) -> PyResult<()> {
        self.as_mut()?.max = max;
        Ok(())
    }

    /// True if the ray hits an Aabb3d or a BoundingSphere within `max`.
    pub fn intersects(&self, volume: &Bound<'_, PyAny>) -> PyResult<bool> {
        intersects_3d(&*self.as_ref()?, volume)
    }

    pub fn aabb_intersection_at(&self, aabb: &PyAabb3d) -> PyResult<Option<f32>> {
        let aabb_3d: Aabb3d = aabb.try_into()?;
        Ok(self.as_ref()?.aabb_intersection_at(&aabb_3d))
    }

    pub fn sphere_intersection_at(&self, sphere: &PyBoundingSphere) -> PyResult<Option<f32>> {
        let bounding_sphere: BoundingSphere = sphere.try_into()?;
        Ok(self.as_ref()?.sphere_intersection_at(&bounding_sphere))
    }

    pub fn direction_recip(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.direction_recip().into()))
    }

    fn __repr__(&self) -> PyResult<String> {
        let cast = self.as_ref()?;
        Ok(format!(
            "RayCast3d(origin={}, direction=Dir3({}, {}, {}), max={})",
            cast.origin.repr_literal(),
            float_repr(cast.direction.x),
            float_repr(cast.direction.y),
            float_repr(cast.direction.z),
            float_repr(cast.max)
        ))
    }
}
