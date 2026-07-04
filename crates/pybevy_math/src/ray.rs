use bevy::math::{Ray2d, Ray3d, Vec2, Vec3};
use pyo3::prelude::*;

use crate::{
    dir2::PyDir2,
    dir3::PyDir3,
    primitives::{infinite_plane3d::PyInfinitePlane3d, plane2d::PyPlane2d},
    vec2::PyVec2,
    vec3::PyVec3,
};

#[pyclass(name = "Ray2d", eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyRay2d(pub(crate) Ray2d);

impl From<PyRay2d> for Ray2d {
    #[inline(always)]
    fn from(py_ray: PyRay2d) -> Self {
        py_ray.0
    }
}

impl From<&PyRay2d> for Ray2d {
    #[inline(always)]
    fn from(py_ray: &PyRay2d) -> Self {
        py_ray.0
    }
}

impl From<Ray2d> for PyRay2d {
    #[inline(always)]
    fn from(ray: Ray2d) -> Self {
        PyRay2d(ray)
    }
}

impl PyRay2d {
    pub fn from_ray2d(ray: Ray2d) -> Self {
        PyRay2d(ray)
    }

    pub fn to_ray2d(&self) -> Ray2d {
        self.0
    }
}

#[pymethods]
impl PyRay2d {
    #[new]
    pub fn new(origin: PyVec2, direction: PyDir2) -> Self {
        let origin_vec: Vec2 = origin.into();
        let dir = direction.get();
        PyRay2d(Ray2d::new(origin_vec, dir))
    }

    #[getter]
    pub fn origin(&self) -> PyVec2 {
        PyVec2::from_vec2(self.0.origin)
    }

    #[getter]
    pub fn direction(&self) -> PyDir2 {
        PyDir2::from_dir2(self.0.direction)
    }

    pub fn get_point(&self, distance: f32) -> PyVec2 {
        PyVec2::from_vec2(self.0.get_point(distance))
    }

    pub fn intersect_plane(&self, plane_origin: PyVec2, plane: &PyPlane2d) -> Option<f32> {
        self.0.intersect_plane(plane_origin.into(), plane.inner)
    }

    pub fn plane_intersection_point(
        &self,
        plane_origin: PyVec2,
        plane: &PyPlane2d,
    ) -> Option<PyVec2> {
        self.0
            .plane_intersection_point(plane_origin.into(), plane.inner)
            .map(PyVec2::from_vec2)
    }

    fn __repr__(&self) -> String {
        format!(
            "Ray2d(origin={:?}, direction={:?})",
            self.0.origin, self.0.direction
        )
    }
}

#[pyclass(name = "Ray3d", eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyRay3d(pub(crate) Ray3d);

impl From<PyRay3d> for Ray3d {
    #[inline(always)]
    fn from(py_ray: PyRay3d) -> Self {
        py_ray.0
    }
}

impl From<&PyRay3d> for Ray3d {
    #[inline(always)]
    fn from(py_ray: &PyRay3d) -> Self {
        py_ray.0
    }
}

impl From<Ray3d> for PyRay3d {
    #[inline(always)]
    fn from(ray: Ray3d) -> Self {
        PyRay3d(ray)
    }
}

impl PyRay3d {
    pub fn from_ray3d(ray: Ray3d) -> Self {
        PyRay3d(ray)
    }

    pub fn to_ray3d(&self) -> Ray3d {
        self.0
    }
}

#[pymethods]
impl PyRay3d {
    #[new]
    pub fn new(origin: PyVec3, direction: PyDir3) -> Self {
        let origin_vec: Vec3 = origin.into();
        let dir = direction.get();
        PyRay3d(Ray3d::new(origin_vec, dir))
    }

    #[getter]
    pub fn origin(&self) -> PyVec3 {
        PyVec3::from_vec3(self.0.origin)
    }

    #[getter]
    pub fn direction(&self) -> PyDir3 {
        PyDir3::from_dir3(self.0.direction)
    }

    pub fn get_point(&self, distance: f32) -> PyVec3 {
        PyVec3::from_vec3(self.0.get_point(distance))
    }

    pub fn intersect_plane(&self, plane_origin: PyVec3, plane: &PyInfinitePlane3d) -> Option<f32> {
        self.0.intersect_plane(plane_origin.into(), plane.inner)
    }

    pub fn plane_intersection_point(
        &self,
        plane_origin: PyVec3,
        plane: &PyInfinitePlane3d,
    ) -> Option<PyVec3> {
        self.0
            .plane_intersection_point(plane_origin.into(), plane.inner)
            .map(PyVec3::from_vec3)
    }

    fn __repr__(&self) -> String {
        format!(
            "Ray3d(origin={:?}, direction={:?})",
            self.0.origin, self.0.direction
        )
    }
}
