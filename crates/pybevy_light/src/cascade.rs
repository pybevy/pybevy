use bevy::light::cascade::Cascade;
use pybevy_core::repr::{float_repr, object_repr};
use pybevy_math::mat4::PyMat4;
use pyo3::prelude::*;

#[pyclass(name = "Cascade", module = "pybevy.light", frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyCascade {
    inner: Cascade,
}

impl From<Cascade> for PyCascade {
    fn from(cascade: Cascade) -> Self {
        PyCascade { inner: cascade }
    }
}

impl From<&Cascade> for PyCascade {
    fn from(cascade: &Cascade) -> Self {
        PyCascade {
            inner: cascade.clone(),
        }
    }
}

impl Default for PyCascade {
    fn default() -> Self {
        Self::new()
    }
}

#[pymethods]
impl PyCascade {
    #[new]
    pub fn new() -> Self {
        Cascade::default().into()
    }

    #[getter]
    pub fn world_from_cascade(&self) -> PyMat4 {
        self.inner.world_from_cascade.into()
    }

    #[getter]
    pub fn clip_from_cascade(&self) -> PyMat4 {
        self.inner.clip_from_cascade.into()
    }

    #[getter]
    pub fn clip_from_world(&self) -> PyMat4 {
        self.inner.clip_from_world.into()
    }

    #[getter]
    pub fn texel_size(&self) -> f32 {
        self.inner.texel_size
    }

    fn __repr__(&self) -> String {
        format!(
            "Cascade(texel_size={}, world_from_cascade={})",
            float_repr(self.inner.texel_size),
            object_repr(PyMat4::from(self.inner.world_from_cascade))
        )
    }
}
