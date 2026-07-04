use bevy::{
    asset::Handle,
    audio::{AudioPlayer, AudioSource},
};
use pybevy_core::{ComponentStorage, PyComponent, handle::PyHandle};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(AudioPlayer<AudioSource>, bridge)]
#[pyclass(name = "AudioPlayer", extends = PyComponent)]
pub struct PyAudioPlayer {
    pub(crate) storage: ComponentStorage<AudioPlayer<AudioSource>>,
}

#[pymethods]
impl PyAudioPlayer {
    #[new]
    pub fn new(handle: PyHandle) -> PyResult<PyClassInitializer<Self>> {
        let bevy_handle = Handle::<AudioSource>::try_from(&handle)?;
        let component = AudioPlayer(bevy_handle);
        Ok(Self::from_owned(component).into())
    }

    fn __repr__(&self) -> String {
        "AudioPlayer(...)".to_string()
    }
}
