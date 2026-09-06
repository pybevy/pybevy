use bevy::{
    camera::primitives::Aabb,
    math::{Vec3, Vec3A},
};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::{
    affine3a::PyAffine3A, mat3a::PyMat3A, primitives::half_space::PyHalfSpace, vec3::PyVec3,
    vec3a::PyVec3A,
};
use pyo3::prelude::*;

#[pycomponent(Aabb, bridge)]
#[pyclass(name = "Aabb", module = "pybevy.camera", extends = PyComponent)]
pub struct PyAabb {
    pub(crate) storage: ComponentStorage<Aabb>,
}

#[pymethods]
impl PyAabb {
    #[new]
    #[pyo3(signature = (center=PyVec3A::vec3a(Vec3A::ZERO), half_extents=PyVec3A::vec3a(Vec3A::ZERO)))]
    pub fn new(center: PyVec3A, half_extents: PyVec3A) -> PyResult<PyClassInitializer<Self>> {
        Ok((
            PyAabb {
                storage: ComponentStorage::owned(Aabb {
                    center: center.try_into()?,
                    half_extents: half_extents.try_into()?,
                }),
            },
            PyComponent,
        )
            .into())
    }

    #[staticmethod]
    pub fn from_min_max(py: Python<'_>, minimum: &PyVec3, maximum: &PyVec3) -> PyResult<Py<Self>> {
        let aabb = Aabb::from_min_max(minimum.try_into()?, maximum.try_into()?);
        Py::new(py, Self::from_owned(aabb))
    }

    #[staticmethod]
    pub fn enclosing(py: Python<'_>, iter: Vec<PyVec3>) -> PyResult<Option<Py<Self>>> {
        let iter: Vec<Vec3> = iter
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;
        Aabb::enclosing(iter)
            .map(|aabb| Py::new(py, Self::from_owned(aabb)))
            .transpose()
    }

    #[getter]
    pub fn center(&self) -> PyResult<PyVec3A> {
        Ok(self.storage.borrow_field_as(|a| &a.center)?)
    }

    #[setter]
    pub fn set_center(&mut self, value: PyVec3A) -> PyResult<()> {
        self.as_mut()?.center = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn half_extents(&self) -> PyResult<PyVec3A> {
        Ok(self.storage.borrow_field_as(|a| &a.half_extents)?)
    }

    #[setter]
    pub fn set_half_extents(&mut self, value: PyVec3A) -> PyResult<()> {
        self.as_mut()?.half_extents = value.try_into()?;
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
            .relative_radius(&p_normal.try_into()?, &world_from_local.try_get()?))
    }

    pub fn is_in_half_space(
        &self,
        half_space: &PyHalfSpace,
        world_from_local: &PyAffine3A,
    ) -> PyResult<bool> {
        Ok(self
            .as_ref()?
            .is_in_half_space(&half_space.into(), &world_from_local.try_get()?))
    }

    pub fn is_in_half_space_identity(&self, half_space: &PyHalfSpace) -> PyResult<bool> {
        Ok(self.as_ref()?.is_in_half_space_identity(&half_space.into()))
    }
}
