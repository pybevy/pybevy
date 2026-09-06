use bevy::camera::primitives::Frustum;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::{
    affine3a::PyAffine3A,
    primitives::{half_space::PyHalfSpace, view_frustum::PyViewFrustum},
};
use pyo3::prelude::*;

use crate::sphere::PySphere;

#[pycomponent(Frustum, bridge)]
#[pyclass(name = "Frustum", module = "pybevy.camera", extends = PyComponent)]
pub struct PyFrustum {
    pub(crate) storage: ComponentStorage<Frustum>,
}

#[pymethods]
impl PyFrustum {
    #[new]
    #[pyo3(signature = (view_frustum = None))]
    pub fn new(view_frustum: Option<PyViewFrustum>) -> PyResult<PyClassInitializer<Self>> {
        let frustum = match view_frustum {
            Some(view_frustum) => Frustum(view_frustum.try_into()?),
            None => Frustum::default(),
        };
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
    pub fn set_value(&mut self, view_frustum: PyViewFrustum) -> PyResult<()> {
        self.as_mut()?.0 = view_frustum.try_into()?;
        Ok(())
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
        aabb: &crate::aabb::PyAabb,
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

    pub fn contains_aabb(
        &self,
        aabb: &crate::aabb::PyAabb,
        world_from_local: &PyAffine3A,
    ) -> PyResult<bool> {
        Ok(self
            .as_ref()?
            .contains_aabb(aabb.as_ref()?.reborrow(), &world_from_local.try_get()?))
    }
}
