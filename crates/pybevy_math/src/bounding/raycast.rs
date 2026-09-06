use bevy::math::{
    Ray3d, Vec2, Vec3,
    bounding::{
        Aabb2d, Aabb3d, BoundingCircle, BoundingSphere, IntersectsVolume, RayCast2d, RayCast3d,
    },
};
use pybevy_core::{FieldStorage, FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

use super::{
    aabb2d::PyAabb2d, aabb3d::PyAabb3d, bounding_circle::PyBoundingCircle,
    bounding_sphere::PyBoundingSphere,
};
use crate::{
    dir2::PyDir2,
    dir3::PyDir3,
    ray::{PyRay2d, PyRay3d},
    vec2::PyVec2,
    vec3::PyVec3,
};

#[pyfield]
#[pyclass(name = "RayCast2d", module = "pybevy.math", skip_from_py_object)]
#[derive(Debug)]
pub struct PyRayCast2d {
    storage: FieldStorage<RayCast2d>,
}

#[pymethods]
impl PyRayCast2d {
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

    pub fn intersects_aabb(&self, aabb: &PyAabb2d) -> PyResult<bool> {
        let aabb_2d: Aabb2d = aabb.try_into()?;
        Ok(self.as_ref()?.intersects(&aabb_2d))
    }

    pub fn intersects_circle(&self, circle: &PyBoundingCircle) -> PyResult<bool> {
        let bounding_circle: BoundingCircle = circle.try_into()?;
        Ok(self.as_ref()?.intersects(&bounding_circle))
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
        Ok(format!("RayCast2d(ray={:?}, max={})", cast.ray, cast.max))
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
    pub fn ray(&self) -> PyResult<PyRay3d> {
        let cast = self.as_ref()?;
        let dir3: bevy::math::Dir3 = cast.direction.into();
        let ray = Ray3d::new(cast.origin.into(), dir3);
        // bevy's RayCast3d stores origin/direction primitives, not a ray, so
        // this getter synthesizes a fresh owned Ray3d. Return an enforced
        // read-only snapshot so writes fail loudly instead of silently
        // mutating a throwaway (W006).
        Ok(PyRay3d::from_borrowed(ValueStorage::read_only_snapshot(
            ray,
        )))
    }

    #[setter]
    pub fn set_ray(&mut self, ray: &PyRay3d) -> PyResult<()> {
        // Rebuild via from_ray so the cached direction reciprocal stays valid.
        let max = self.as_ref()?.max;
        self.as_mut()?
            .clone_from(&RayCast3d::from_ray(ray.to_ray3d()?, max));
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

    pub fn intersects_aabb(&self, aabb: &PyAabb3d) -> PyResult<bool> {
        let aabb_3d: Aabb3d = aabb.try_into()?;
        Ok(self.as_ref()?.intersects(&aabb_3d))
    }

    pub fn intersects_sphere(&self, sphere: &PyBoundingSphere) -> PyResult<bool> {
        let bounding_sphere: BoundingSphere = sphere.try_into()?;
        Ok(self.as_ref()?.intersects(&bounding_sphere))
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
            "RayCast3d(origin={:?}, direction={:?}, max={})",
            cast.origin, cast.direction, cast.max
        ))
    }
}
