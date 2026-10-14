use bevy::audio::GlobalVolume;
use pybevy_core::{PyResource, ResourceStorage, repr::object_repr};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

use crate::volume::PyVolume;

#[pyresource(GlobalVolume, bridge)]
#[pyclass(name = "GlobalVolume", module = "pybevy.audio", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyGlobalVolume {
    pub(crate) storage: ResourceStorage<GlobalVolume>,
}

#[pymethods]
impl PyGlobalVolume {
    #[new]
    #[pyo3(signature = (volume = PyVolume::from(GlobalVolume::default().volume)))]
    pub fn new(volume: PyVolume) -> PyClassInitializer<Self> {
        Self::from_owned(GlobalVolume::new(volume.into()))
    }

    #[getter]
    pub fn volume(&self) -> PyResult<PyVolume> {
        Ok(self.as_ref()?.volume.into())
    }

    #[setter]
    pub fn set_volume(&mut self, volume: PyVolume) -> PyResult<()> {
        self.as_mut()?.volume = volume.into();
        Ok(())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(gv) => format!(
                "GlobalVolume(volume={})",
                object_repr(PyVolume::from(gv.volume))
            ),
            Err(_) => "GlobalVolume(<invalid>)".to_string(),
        }
    }
}
