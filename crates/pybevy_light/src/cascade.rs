use bevy::light::cascade::Cascade;
use pybevy_math::PyMat4;
use pyo3::prelude::*;

#[pyclass(name = "Cascade", frozen)]
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

#[pymethods]
impl PyCascade {
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
            "Cascade(texel_size={:.4}, world_from_cascade={:?})",
            self.inner.texel_size, self.inner.world_from_cascade
        )
    }
}
