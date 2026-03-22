use bevy::{camera::primitives::Aabb, math::Vec3A};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pybevy_math::{affine3a::PyAffine3A, mat3a::PyMat3A, vec3::PyVec3, vec3a::PyVec3A};
use pyo3::prelude::*;

use crate::half_space::PyHalfSpace;

#[component_storage(Aabb)]
#[pyclass(name = "Aabb", extends = PyComponent)]
#[derive(Clone)]
pub struct PyAabb {
    pub(crate) storage: ComponentStorage<Aabb>,
}

#[pymethods]
impl PyAabb {
    #[new]
    #[pyo3(signature = (center=PyVec3A::vec3a(Vec3A::ZERO), half_extents=PyVec3A::vec3a(Vec3A::ZERO)))]
    pub fn new(center: PyVec3A, half_extents: PyVec3A) -> (Self, PyComponent) {
        (
            PyAabb {
                storage: ComponentStorage::owned(Aabb {
                    center: center.into(),
                    half_extents: half_extents.into(),
                }),
            },
            PyComponent,
        )
    }

    #[staticmethod]
    pub fn from_min_max(py: Python<'_>, minimum: &PyVec3, maximum: &PyVec3) -> PyResult<Py<Self>> {
        let aabb = Aabb::from_min_max(minimum.into(), maximum.into());
        Py::new(py, Self::from_owned(aabb))
    }

    #[getter]
    pub fn center(&self) -> PyResult<PyVec3A> {
        Ok(self.storage.borrow_field_as(|a| &a.center)?)
    }

    #[setter]
    pub fn set_center(&mut self, value: PyVec3A) -> PyResult<()> {
        self.as_mut()?.center = value.into();
        Ok(())
    }

    #[getter]
    pub fn half_extents(&self) -> PyResult<PyVec3A> {
        Ok(self.storage.borrow_field_as(|a| &a.half_extents)?)
    }

    #[setter]
    pub fn set_half_extents(&mut self, value: PyVec3A) -> PyResult<()> {
        self.as_mut()?.half_extents = value.into();
        Ok(())
    }

    pub fn min(&self) -> PyResult<PyVec3A> {
        Ok(self.as_ref()?.min().into())
    }

    pub fn max(&self) -> PyResult<PyVec3A> {
        Ok(self.as_ref()?.max().into())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let aabb = self.as_ref()?;
        Ok(format!(
            "Aabb(center={:?}, half_extents={:?})",
            aabb.center, aabb.half_extents
        ))
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        let a = self.as_ref()?;
        let b = other.as_ref()?;
        Ok(a.center == b.center && a.half_extents == b.half_extents)
    }

    pub fn relative_radius(&self, p_normal: &PyVec3A, world_from_local: &PyMat3A) -> PyResult<f32> {
        Ok(self
            .as_ref()?
            .relative_radius(&p_normal.into(), &world_from_local.get()))
    }

    pub fn is_in_half_space(
        &self,
        half_space: &PyHalfSpace,
        world_from_local: &PyAffine3A,
    ) -> PyResult<bool> {
        Ok(self
            .as_ref()?
            .is_in_half_space(&half_space.into(), &world_from_local.get()))
    }
}
