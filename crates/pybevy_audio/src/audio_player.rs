use bevy::audio::{AudioPlayer, AudioSource};
use pybevy_core::{ComponentStorage, PyComponent, handle::PyHandle};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

#[component_storage(AudioPlayer<AudioSource>)]
#[pyclass(name = "AudioPlayer", extends = PyComponent)]
#[derive(Clone)]
pub struct PyAudioPlayer {
    pub(crate) storage: ComponentStorage<AudioPlayer<AudioSource>>,
}

#[pymethods]
impl PyAudioPlayer {
    #[new]
    pub fn new(handle: PyHandle) -> PyResult<(Self, PyComponent)> {
        let bevy_handle = bevy::asset::Handle::<AudioSource>::try_from(&handle)?;
        let component = AudioPlayer(bevy_handle);
        Ok(Self::from_owned(component))
    }

    fn __repr__(&self) -> String {
        "AudioPlayer(...)".to_string()
    }
}
