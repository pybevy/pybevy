use bevy::{camera::primitives::Sphere, math::Vec3A};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pybevy_math::vec3a::PyVec3A;
use pyo3::prelude::*;

#[pyvalue]
#[pyclass(name = "Sphere", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PySphere {
    pub(crate) storage: ValueStorage<Sphere>,
}

impl From<Sphere> for PySphere {
    fn from(sphere: Sphere) -> Self {
        Self::from_owned(sphere)
    }
}

impl TryFrom<&PySphere> for Sphere {
    type Error = PyErr;

    fn try_from(py_sphere: &PySphere) -> PyResult<Self> {
        py_sphere.to_bevy()
    }
}

impl TryFrom<PySphere> for Sphere {
    type Error = PyErr;

    fn try_from(py_sphere: PySphere) -> PyResult<Self> {
        py_sphere.to_bevy()
    }
}

#[pymethods]
impl PySphere {
    #[new]
    #[pyo3(signature = (center=PyVec3A::vec3a(Vec3A::ZERO), radius=0.0))]
    pub fn new(center: PyVec3A, radius: f32) -> PyResult<Self> {
        Ok(Self::from_owned(Sphere {
            center: center.try_into()?,
            radius,
        }))
    }

    #[getter]
    pub fn center(&self) -> PyResult<PyVec3A> {
        Ok(self.storage.borrow_field_as(|s| &s.center)?)
    }

    #[setter]
    pub fn set_center(&mut self, value: PyVec3A) -> PyResult<()> {
        self.as_mut()?.center = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.radius)
    }

    #[setter]
    pub fn set_radius(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.radius = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Sphere(center={:?}, radius={})",
            self.as_ref()?.center,
            self.as_ref()?.radius
        ))
    }
}
