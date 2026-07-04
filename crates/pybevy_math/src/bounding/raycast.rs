use bevy::math::{
    Ray3d, Vec2, Vec3,
    bounding::{IntersectsVolume, RayCast2d, RayCast3d},
};
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

#[pyclass(name = "RayCast2d", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyRayCast2d {
    ray_cast: RayCast2d,
}

impl From<PyRayCast2d> for RayCast2d {
    fn from(py_raycast: PyRayCast2d) -> Self {
        py_raycast.ray_cast
    }
}

impl From<&PyRayCast2d> for RayCast2d {
    fn from(py_raycast: &PyRayCast2d) -> Self {
        py_raycast.ray_cast.clone()
    }
}

impl From<RayCast2d> for PyRayCast2d {
    fn from(ray_cast: RayCast2d) -> Self {
        PyRayCast2d { ray_cast }
    }
}

#[pymethods]
impl PyRayCast2d {
    #[new]
    pub fn new(origin: PyVec2, direction: PyDir2, max: f32) -> Self {
        let origin_vec: Vec2 = origin.into();
        let dir = direction.get();
        PyRayCast2d {
            ray_cast: RayCast2d::new(origin_vec, dir, max),
        }
    }

    #[staticmethod]
    pub fn from_ray(ray: &PyRay2d, max: f32) -> Self {
        PyRayCast2d {
            ray_cast: RayCast2d::from_ray(ray.to_ray2d(), max),
        }
    }

    #[getter]
    pub fn ray(&self) -> PyRay2d {
        PyRay2d::from_ray2d(self.ray_cast.ray)
    }

    #[getter]
    pub fn max(&self) -> f32 {
        self.ray_cast.max
    }

    pub fn intersects_aabb(&self, aabb: &PyAabb2d) -> bool {
        let aabb_2d: bevy::math::bounding::Aabb2d = aabb.into();
        self.ray_cast.intersects(&aabb_2d)
    }

    pub fn intersects_circle(&self, circle: &PyBoundingCircle) -> bool {
        let bounding_circle: bevy::math::bounding::BoundingCircle = circle.into();
        self.ray_cast.intersects(&bounding_circle)
    }

    pub fn aabb_intersection_at(&self, aabb: &PyAabb2d) -> Option<f32> {
        let aabb_2d: bevy::math::bounding::Aabb2d = aabb.into();
        self.ray_cast.aabb_intersection_at(&aabb_2d)
    }

    pub fn circle_intersection_at(&self, circle: &PyBoundingCircle) -> Option<f32> {
        let bounding_circle: bevy::math::bounding::BoundingCircle = circle.into();
        self.ray_cast.circle_intersection_at(&bounding_circle)
    }

    pub fn direction_recip(&self) -> PyVec2 {
        PyVec2::from_vec2(self.ray_cast.direction_recip())
    }

    fn __repr__(&self) -> String {
        format!(
            "RayCast2d(ray={:?}, max={})",
            self.ray_cast.ray, self.ray_cast.max
        )
    }
}

#[pyclass(name = "RayCast3d", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyRayCast3d {
    ray_cast: RayCast3d,
}

impl From<PyRayCast3d> for RayCast3d {
    fn from(py_raycast: PyRayCast3d) -> Self {
        py_raycast.ray_cast
    }
}

impl From<&PyRayCast3d> for RayCast3d {
    fn from(py_raycast: &PyRayCast3d) -> Self {
        py_raycast.ray_cast.clone()
    }
}

impl From<RayCast3d> for PyRayCast3d {
    fn from(ray_cast: RayCast3d) -> Self {
        PyRayCast3d { ray_cast }
    }
}

#[pymethods]
impl PyRayCast3d {
    #[new]
    pub fn new(origin: PyVec3, direction: PyDir3, max: f32) -> Self {
        let origin_vec: Vec3 = origin.into();
        let dir = direction.get();
        PyRayCast3d {
            ray_cast: RayCast3d::new(origin_vec, dir, max),
        }
    }

    #[staticmethod]
    pub fn from_ray(ray: &PyRay3d, max: f32) -> Self {
        PyRayCast3d {
            ray_cast: RayCast3d::from_ray(ray.to_ray3d(), max),
        }
    }

    #[getter]
    pub fn ray(&self) -> PyRay3d {
        let dir3: bevy::math::Dir3 = self.ray_cast.direction.into();
        let ray = Ray3d::new(self.ray_cast.origin.into(), dir3);
        PyRay3d::from_ray3d(ray)
    }

    #[getter]
    pub fn max(&self) -> f32 {
        self.ray_cast.max
    }

    pub fn intersects_aabb(&self, aabb: &PyAabb3d) -> bool {
        let aabb_3d: bevy::math::bounding::Aabb3d = aabb.into();
        self.ray_cast.intersects(&aabb_3d)
    }

    pub fn intersects_sphere(&self, sphere: &PyBoundingSphere) -> bool {
        let bounding_sphere: bevy::math::bounding::BoundingSphere = sphere.into();
        self.ray_cast.intersects(&bounding_sphere)
    }

    pub fn aabb_intersection_at(&self, aabb: &PyAabb3d) -> Option<f32> {
        let aabb_3d: bevy::math::bounding::Aabb3d = aabb.into();
        self.ray_cast.aabb_intersection_at(&aabb_3d)
    }

    pub fn sphere_intersection_at(&self, sphere: &PyBoundingSphere) -> Option<f32> {
        let bounding_sphere: bevy::math::bounding::BoundingSphere = sphere.into();
        self.ray_cast.sphere_intersection_at(&bounding_sphere)
    }

    pub fn direction_recip(&self) -> PyVec3 {
        PyVec3::from_vec3(self.ray_cast.direction_recip().into())
    }

    fn __repr__(&self) -> String {
        format!(
            "RayCast3d(origin={:?}, direction={:?}, max={})",
            self.ray_cast.origin, self.ray_cast.direction, self.ray_cast.max
        )
    }
}
