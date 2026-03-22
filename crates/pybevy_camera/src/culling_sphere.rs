use bevy::{camera::primitives::Sphere, math::Vec3A};
use pybevy_math::PyVec3A;
use pyo3::prelude::*;

#[pyclass(name = "CullingSphere")]
#[derive(Debug, Clone)]
pub struct PyCullingSphere {
    pub(crate) inner: Sphere,
}

impl From<Sphere> for PyCullingSphere {
    fn from(sphere: Sphere) -> Self {
        PyCullingSphere { inner: sphere }
    }
}

impl From<&PyCullingSphere> for Sphere {
    fn from(py_sphere: &PyCullingSphere) -> Self {
        py_sphere.inner.clone()
    }
}

impl From<PyCullingSphere> for Sphere {
    fn from(py_sphere: PyCullingSphere) -> Self {
        py_sphere.inner
    }
}

#[pymethods]
impl PyCullingSphere {
    #[new]
    #[pyo3(signature = (center=PyVec3A::vec3a(Vec3A::ZERO), radius=0.0))]
    pub fn new(center: PyVec3A, radius: f32) -> Self {
        PyCullingSphere {
            inner: Sphere {
                center: center.into(),
                radius,
            },
        }
    }

    #[getter]
    pub fn center(&self) -> PyVec3A {
        self.inner.center.into()
    }

    #[setter]
    pub fn set_center(&mut self, value: PyVec3A) {
        self.inner.center = value.into();
    }

    #[getter]
    pub fn radius(&self) -> f32 {
        self.inner.radius
    }

    #[setter]
    pub fn set_radius(&mut self, value: f32) {
        self.inner.radius = value;
    }

    pub fn __repr__(&self) -> String {
        format!(
            "CullingSphere(center={:?}, radius={})",
            self.inner.center, self.inner.radius
        )
    }

    pub fn __eq__(&self, other: &Self) -> bool {
        self.inner.center == other.inner.center && self.inner.radius == other.inner.radius
    }
}
