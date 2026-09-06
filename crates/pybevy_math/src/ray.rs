use bevy::math::{Ray2d, Ray3d, Vec2, Vec3};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

use crate::{
    dir2::PyDir2,
    dir3::PyDir3,
    primitives::{infinite_plane3d::PyInfinitePlane3d, plane2d::PyPlane2d},
    vec2::PyVec2,
    vec3::PyVec3,
};

#[pyclass(name = "Ray2d", module = "pybevy.math", eq, from_py_object)]
#[pyvalue]
#[derive(Debug, Clone)]
pub struct PyRay2d {
    pub(crate) storage: ValueStorage<Ray2d>,
}

impl PartialEq for PyRay2d {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl TryFrom<PyRay2d> for Ray2d {
    type Error = PyErr;

    fn try_from(value: PyRay2d) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyRay2d> for Ray2d {
    type Error = PyErr;

    fn try_from(value: &PyRay2d) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl From<Ray2d> for PyRay2d {
    fn from(value: Ray2d) -> Self {
        Self::from_owned(value)
    }
}

impl PyRay2d {
    pub fn from_ray2d(ray: Ray2d) -> Self {
        Self::from_owned(ray)
    }

    pub fn to_ray2d(&self) -> PyResult<Ray2d> {
        self.to_bevy()
    }
}

#[pymethods]
impl PyRay2d {
    #[new]
    pub fn new(origin: PyVec2, direction: PyDir2) -> PyResult<Self> {
        let origin_vec: Vec2 = origin.try_into()?;
        let dir = direction.get()?;
        Ok(PyRay2d::from_owned(Ray2d::new(origin_vec, dir)))
    }

    #[getter]
    pub fn origin(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|r| &r.origin)?)
    }

    #[setter]
    pub fn set_origin(&mut self, origin: PyVec2) -> PyResult<()> {
        self.as_mut()?.origin = origin.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn direction(&self) -> PyResult<PyDir2> {
        Ok(self.storage.borrow_field_as(|ray| &ray.direction)?)
    }

    #[setter]
    pub fn set_direction(&mut self, direction: PyDir2) -> PyResult<()> {
        self.as_mut()?.direction = direction.into_dir2()?;
        Ok(())
    }

    pub fn get_point(&self, distance: f32) -> PyResult<PyVec2> {
        Ok(PyVec2::from_vec2(self.as_ref()?.get_point(distance)))
    }

    pub fn intersect_plane(
        &self,
        plane_origin: PyVec2,
        plane: &PyPlane2d,
    ) -> PyResult<Option<f32>> {
        Ok(self
            .as_ref()?
            .intersect_plane(plane_origin.try_into()?, plane.inner))
    }

    pub fn plane_intersection_point(
        &self,
        plane_origin: PyVec2,
        plane: &PyPlane2d,
    ) -> PyResult<Option<PyVec2>> {
        Ok(self
            .as_ref()?
            .plane_intersection_point(plane_origin.try_into()?, plane.inner)
            .map(PyVec2::from_vec2))
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Ray2d(origin={:?}, direction={:?})",
            self.as_ref()?.origin,
            self.as_ref()?.direction
        ))
    }
}

#[pyclass(name = "Ray3d", module = "pybevy.math", eq, skip_from_py_object)]
#[pyvalue]
#[derive(Debug, Clone)]
pub struct PyRay3d {
    pub(crate) storage: ValueStorage<Ray3d>,
}

impl PartialEq for PyRay3d {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl TryFrom<PyRay3d> for Ray3d {
    type Error = PyErr;

    fn try_from(value: PyRay3d) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyRay3d> for Ray3d {
    type Error = PyErr;

    fn try_from(value: &PyRay3d) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl From<Ray3d> for PyRay3d {
    fn from(value: Ray3d) -> Self {
        Self::from_owned(value)
    }
}

impl PyRay3d {
    pub fn from_ray3d(ray: Ray3d) -> Self {
        Self::from_owned(ray)
    }

    pub fn to_ray3d(&self) -> PyResult<Ray3d> {
        self.to_bevy()
    }
}

#[pymethods]
impl PyRay3d {
    #[new]
    pub fn new(origin: PyVec3, direction: PyDir3) -> PyResult<Self> {
        let origin_vec: Vec3 = origin.try_into()?;
        let dir = direction.get()?;
        Ok(PyRay3d::from_owned(Ray3d::new(origin_vec, dir)))
    }

    #[getter]
    pub fn origin(&self) -> PyResult<PyVec3> {
        Ok(self.storage.borrow_field_as(|r| &r.origin)?)
    }

    #[setter]
    pub fn set_origin(&mut self, origin: PyVec3) -> PyResult<()> {
        self.as_mut()?.origin = origin.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn direction(&self) -> PyResult<PyDir3> {
        Ok(self.storage.borrow_field_as(|ray| &ray.direction)?)
    }

    #[setter]
    pub fn set_direction(&mut self, direction: PyDir3) -> PyResult<()> {
        self.as_mut()?.direction = direction.into_dir3()?;
        Ok(())
    }

    pub fn get_point(&self, distance: f32) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.get_point(distance)))
    }

    pub fn intersect_plane(
        &self,
        plane_origin: PyVec3,
        plane: &PyInfinitePlane3d,
    ) -> PyResult<Option<f32>> {
        Ok(self
            .as_ref()?
            .intersect_plane(plane_origin.try_into()?, plane.inner))
    }

    pub fn plane_intersection_point(
        &self,
        plane_origin: PyVec3,
        plane: &PyInfinitePlane3d,
    ) -> PyResult<Option<PyVec3>> {
        Ok(self
            .as_ref()?
            .plane_intersection_point(plane_origin.try_into()?, plane.inner)
            .map(PyVec3::from_vec3))
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Ray3d(origin={:?}, direction={:?})",
            self.as_ref()?.origin,
            self.as_ref()?.direction
        ))
    }
}
