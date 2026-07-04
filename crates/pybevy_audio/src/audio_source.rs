use bevy::audio::AudioSource;
use pybevy_core::AssetStorage;
use pybevy_macros::pyasset;
use pyo3::{prelude::*, types::PyBytes};

#[pyasset(AudioSource, bridge)]
#[pyclass(name = "AudioSource", extends = pybevy_core::PyAsset, skip_from_py_object)]
#[derive(Debug)]
pub struct PyAudioSource {
    pub(crate) storage: AssetStorage<AudioSource>,
}

#[pymethods]
impl PyAudioSource {
    #[getter]
    pub fn bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let audio_source = self.as_ref()?;
        Ok(PyBytes::new(py, &audio_source.bytes))
    }

    fn __repr__(&self) -> String {
        match self.storage.as_ref() {
            Ok(source) => format!("AudioSource({} bytes)", source.bytes.len()),
            Err(_) => "AudioSource(<invalid>)".to_string(),
        }
    }
}
