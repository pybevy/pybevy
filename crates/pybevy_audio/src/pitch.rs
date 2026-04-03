use std::time::Duration;

use bevy::audio::Pitch;
use pybevy_core::AssetStorage;
use pybevy_macros::pyasset;
use pyo3::prelude::*;

#[pyasset(Pitch, bridge)]
#[pyclass(name = "Pitch", extends = pybevy_core::PyAsset)]
#[derive(Debug)]
pub struct PyPitch {
    pub(crate) storage: AssetStorage<Pitch>,
}

#[pymethods]
impl PyPitch {
    #[new]
    #[pyo3(signature = (frequency, duration))]
    pub fn new(frequency: f32, duration: f64) -> (Self, pybevy_core::PyAsset) {
        let pitch = Pitch::new(frequency, Duration::from_secs_f64(duration));
        Self::from_owned(pitch)
    }

    #[getter]
    pub fn frequency(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.frequency)
    }

    #[setter]
    pub fn set_frequency(&mut self, frequency: f32) -> PyResult<()> {
        self.as_mut()?.frequency = frequency;
        Ok(())
    }

    #[getter]
    pub fn duration(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.duration.as_secs_f64())
    }

    #[setter]
    pub fn set_duration(&mut self, duration: f64) -> PyResult<()> {
        self.as_mut()?.duration = Duration::from_secs_f64(duration);
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let pitch = self.as_ref()?;
        Ok(format!(
            "Pitch(frequency={} Hz, duration={:.3} s)",
            pitch.frequency,
            pitch.duration.as_secs_f64()
        ))
    }
}
