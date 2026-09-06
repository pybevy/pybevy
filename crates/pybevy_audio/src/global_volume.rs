use bevy::audio::GlobalVolume;
use pybevy_core::{PyResource, ResourceStorage};
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
    #[pyo3(signature = (volume=None))]
    pub fn new(volume: Option<PyVolume>) -> PyClassInitializer<Self> {
        let global_volume = match volume {
            Some(v) => GlobalVolume::new(v.into()),
            None => GlobalVolume::default(),
        };
        Self::from_owned(global_volume)
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
            Ok(gv) => format!("GlobalVolume(volume={:?})", gv.volume),
            Err(_) => "GlobalVolume(<invalid>)".to_string(),
        }
    }
}
