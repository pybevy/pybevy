use bevy::{
    camera::primitives::Frustum,
    math::{Mat4, Vec3},
};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::{affine3a::PyAffine3A, mat4::PyMat4, vec3::PyVec3};
use pyo3::prelude::*;

use crate::{half_space::PyHalfSpace, sphere::PySphere};

#[pycomponent(Frustum, bridge)]
#[pyclass(name = "Frustum", extends = PyComponent)]
#[derive(Clone)]
pub struct PyFrustum {
    pub(crate) storage: ComponentStorage<Frustum>,
}

#[pymethods]
impl PyFrustum {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (
            PyFrustum {
                storage: ComponentStorage::owned(Frustum::default()),
            },
            PyComponent,
        )
    }

    #[staticmethod]
    pub fn from_clip_from_world(py: Python<'_>, clip_from_world: &PyMat4) -> PyResult<Py<Self>> {
        let mat: Mat4 = clip_from_world.into();
        Py::new(py, Self::from_owned(Frustum::from_clip_from_world(&mat)))
    }

    #[staticmethod]
    pub fn from_clip_from_world_custom_far(
        py: Python<'_>,
        clip_from_world: &PyMat4,
        view_translation: PyVec3,
        view_backward: PyVec3,
        far: f32,
    ) -> PyResult<Py<Self>> {
        let mat: Mat4 = clip_from_world.into();
        let translation: Vec3 = view_translation.into();
        let backward: Vec3 = view_backward.into();
        Py::new(
            py,
            Self::from_owned(Frustum::from_clip_from_world_custom_far(
                &mat,
                &translation,
                &backward,
                far,
            )),
        )
    }

    #[getter]
    pub fn half_spaces(&self) -> PyResult<Vec<PyHalfSpace>> {
        let frustum = self.as_ref()?;
        Ok(frustum.half_spaces.iter().map(|&hs| hs.into()).collect())
    }

    #[setter]
    pub fn set_half_spaces(&mut self, half_spaces: Vec<PyHalfSpace>) -> PyResult<()> {
        use pyo3::exceptions::PyValueError;

        if half_spaces.len() != 6 {
            return Err(PyValueError::new_err(
                "Frustum requires exactly 6 half-spaces",
            ));
        }
        let frustum = self.as_mut()?;
        for (i, hs) in half_spaces.into_iter().enumerate() {
            frustum.half_spaces[i] = hs.into();
        }
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok("Frustum(...)".to_string())
    }

    pub fn intersects_sphere(&self, sphere: &PySphere, intersect_far: bool) -> PyResult<bool> {
        Ok(self
            .as_ref()?
            .intersects_sphere(&sphere.into(), intersect_far))
    }

    pub fn intersects_obb(
        &self,
        aabb: &crate::aabb::PyAabb,
        world_from_local: &PyAffine3A,
        intersect_near: bool,
        intersect_far: bool,
    ) -> PyResult<bool> {
        Ok(self.as_ref()?.intersects_obb(
            aabb.as_ref()?,
            &world_from_local.get(),
            intersect_near,
            intersect_far,
        ))
    }

    pub fn contains_aabb(
        &self,
        aabb: &crate::aabb::PyAabb,
        world_from_local: &PyAffine3A,
    ) -> PyResult<bool> {
        Ok(self
            .as_ref()?
            .contains_aabb(aabb.as_ref()?, &world_from_local.get()))
    }
}
