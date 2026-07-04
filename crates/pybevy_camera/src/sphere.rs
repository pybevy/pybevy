use bevy::{camera::primitives::Sphere, math::Vec3A};
use pybevy_math::vec3a::PyVec3A;
use pyo3::prelude::*;

#[pyclass(name = "Sphere", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PySphere {
    pub(crate) inner: Sphere,
}

impl From<Sphere> for PySphere {
    fn from(sphere: Sphere) -> Self {
        PySphere { inner: sphere }
    }
}

impl From<&PySphere> for Sphere {
    fn from(py_sphere: &PySphere) -> Self {
        py_sphere.inner.clone()
    }
}

impl From<PySphere> for Sphere {
    fn from(py_sphere: PySphere) -> Self {
        py_sphere.inner
    }
}

#[pymethods]
impl PySphere {
    #[new]
    #[pyo3(signature = (center=PyVec3A::vec3a(Vec3A::ZERO), radius=0.0))]
    pub fn new(center: PyVec3A, radius: f32) -> Self {
        PySphere {
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
            "Sphere(center={:?}, radius={})",
            self.inner.center, self.inner.radius
        )
    }

    pub fn __eq__(&self, other: &Self) -> bool {
        self.inner.center == other.inner.center && self.inner.radius == other.inner.radius
    }
}
