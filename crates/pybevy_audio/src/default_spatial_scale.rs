use bevy::audio::DefaultSpatialScale;
use pybevy_core::{PyResource, ResourceStorage};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

use crate::spatial_scale::PySpatialScale;

#[pyresource(DefaultSpatialScale, bridge)]
#[pyclass(name = "DefaultSpatialScale", module = "pybevy.audio", extends = PyResource, from_py_object)]
pub struct PyDefaultSpatialScale {
    pub(crate) storage: ResourceStorage<DefaultSpatialScale>,
}

#[pymethods]
impl PyDefaultSpatialScale {
    #[new]
    #[pyo3(signature = (scale=None))]
    pub fn new(scale: Option<PySpatialScale>) -> PyClassInitializer<Self> {
        let default_scale = match scale {
            Some(s) => DefaultSpatialScale(s.inner),
            None => DefaultSpatialScale::default(),
        };
        Self::from_owned(default_scale)
    }

    #[getter]
    pub fn scale(&self) -> PyResult<PySpatialScale> {
        Ok(self.as_ref()?.0.into())
    }

    #[setter]
    pub fn set_scale(&mut self, scale: PySpatialScale) -> PyResult<()> {
        self.as_mut()?.0 = scale.inner;
        Ok(())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(ds) => format!("DefaultSpatialScale({:?})", ds.0),
            Err(_) => "DefaultSpatialScale(<invalid>)".to_string(),
        }
    }
}
