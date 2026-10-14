use std::time::Duration;

use bevy::audio::Pitch;
use pybevy_core::{AssetStorage, domain::DurationInput, repr::float_repr};
use pybevy_macros::pyasset;
use pyo3::prelude::*;

#[pyasset(Pitch, bridge, domains = [
    frequency => [positive],
    duration => [finite, non_negative],
])]
#[pyclass(name = "Pitch", module = "pybevy.audio", extends = pybevy_core::PyAsset, skip_from_py_object)]
#[derive(Debug)]
pub struct PyPitch {
    pub(crate) storage: AssetStorage<Pitch>,
}

#[pymethods]
impl PyPitch {
    #[new]
    #[pyo3(signature = (frequency, duration))]
    pub fn new(frequency: f32, duration: DurationInput) -> PyResult<PyClassInitializer<Self>> {
        let frequency = Self::check_frequency(frequency)?;
        let duration = Self::check_duration(duration)?.stored();
        let pitch = Pitch::new(frequency, duration);
        Ok(Self::from_owned(pitch).into())
    }

    #[getter]
    pub fn frequency(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.frequency)
    }

    #[setter]
    pub fn set_frequency(&mut self, frequency: f32) -> PyResult<()> {
        let frequency = Self::check_frequency(frequency)?;
        self.as_mut()?.frequency = frequency;
        Ok(())
    }

    #[getter]
    pub fn duration(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.duration)
    }

    #[setter]
    pub fn set_duration(&mut self, duration: DurationInput) -> PyResult<()> {
        let duration = Self::check_duration(duration)?.stored();
        self.as_mut()?.duration = duration;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let pitch = self.as_ref()?;
        Ok(format!(
            "Pitch(frequency={} Hz, duration={:.3} s)",
            float_repr(pitch.frequency),
            pitch.duration.as_secs_f64()
        ))
    }
}
