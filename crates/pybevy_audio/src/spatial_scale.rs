use bevy::audio::SpatialScale;
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

#[pyclass(name = "SpatialScale", module = "pybevy.audio", frozen, from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PySpatialScale {
    pub inner: SpatialScale,
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
    #[pyo3(signature = (scale=None))]
    pub fn new(scale: Option<f32>) -> PyResult<Self> {
        let inner = match scale {
            None => SpatialScale::new(1.0),
            Some(value) => SpatialScale::new(value),
        };
        Ok(Self { inner })
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
