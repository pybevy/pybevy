use bevy::audio::SpatialScale;
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

#[pyclass(name = "SpatialScale", module = "pybevy.audio", frozen, from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PySpatialScale {
    pub(crate) inner: SpatialScale,
}

impl From<SpatialScale> for PySpatialScale {
    fn from(scale: SpatialScale) -> Self {
        Self { inner: scale }
    }
}

impl From<PySpatialScale> for SpatialScale {
    fn from(scale: PySpatialScale) -> Self {
        scale.inner
    }
}

#[pymethods]
impl PySpatialScale {
    #[new]
    #[pyo3(signature = (scale=1.0))]
    pub fn new(scale: f32) -> Self {
        Self {
            inner: SpatialScale::new(scale),
        }
    }

    #[staticmethod]
    pub fn new_2d(scale: f32) -> Self {
        Self {
            inner: SpatialScale::new_2d(scale),
        }
    }

    pub fn to_vec3(&self) -> PyVec3 {
        PyVec3::from_vec3(self.inner.0)
    }

    fn __repr__(&self) -> String {
        format!("SpatialScale({:?})", self.inner.0)
    }
}
