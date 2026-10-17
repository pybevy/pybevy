use bevy::{camera::primitives::Frustum, shape::ViewFrustum};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::affine3a::PyAffine3A;
use pybevy_shape::primitives::{
    half_space::PyHalfSpace, half_spaces::PyHalfSpaces, view_frustum::PyViewFrustum,
};
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{aabb::PyAabb, sphere::PySphere};

#[pycomponent(Frustum, bridge)]
#[pyclass(name = "Frustum", module = "pybevy.camera", extends = PyComponent)]
pub struct PyFrustum {
    pub(crate) storage: ComponentStorage<Frustum>,
}

#[pymethods]
impl PyFrustum {
    #[new]
    #[pyo3(signature = (value = PyViewFrustum::from(ViewFrustum::default())))]
    pub fn new(value: PyViewFrustum) -> PyResult<PyClassInitializer<Self>> {
        let frustum = Frustum(value.try_into()?);
        Ok((
            PyFrustum {
                storage: ComponentStorage::owned(frustum),
            },
            PyComponent,
        )
            .into())
    }

    #[getter]
    pub fn value(&self) -> PyResult<PyViewFrustum> {
        Ok(self.storage.borrow_field_as(|f| &f.0)?)
    }

    #[setter]
    pub fn set_value(&mut self, value: PyViewFrustum) -> PyResult<()> {
        self.as_mut()?.0 = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn half_spaces(&self) -> PyResult<PyHalfSpaces> {
        Ok(self
            .storage
            .borrow_field_as(|frustum| &frustum.0.half_spaces)?)
    }

    #[setter]
    pub fn set_half_spaces(&mut self, half_spaces: Vec<PyHalfSpace>) -> PyResult<()> {
        if half_spaces.len() != 6 {
            return Err(PyValueError::new_err(
                "Frustum requires exactly 6 half-spaces",
            ));
        }
        let mut frustum = self.as_mut()?;
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
            .intersects_sphere(&sphere.try_into()?, intersect_far))
    }

    pub fn intersects_obb(
        &self,
        aabb: &PyAabb,
        world_from_local: &PyAffine3A,
        intersect_near: bool,
        intersect_far: bool,
    ) -> PyResult<bool> {
        Ok(self.as_ref()?.intersects_obb(
            aabb.as_ref()?.reborrow(),
            &world_from_local.try_get()?,
            intersect_near,
            intersect_far,
        ))
    }

    pub fn contains_aabb(&self, aabb: &PyAabb, world_from_local: &PyAffine3A) -> PyResult<bool> {
        Ok(self
            .as_ref()?
            .contains_aabb(aabb.as_ref()?.reborrow(), &world_from_local.try_get()?))
    }
}
