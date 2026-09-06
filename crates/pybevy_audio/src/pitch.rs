use bevy::audio::Pitch;
use pybevy_core::{AssetStorage, duration_from_secs_f64, try_finite_positive_frequency};
use pybevy_macros::pyasset;
use pyo3::{exceptions::PyValueError, prelude::*};

fn validate_frequency(frequency: f32) -> PyResult<()> {
    try_finite_positive_frequency(frequency as f64)
        .map_err(|error| PyValueError::new_err(error.message()))?;
    Ok(())
}

#[pyasset(Pitch, bridge)]
#[pyclass(name = "Pitch", module = "pybevy.audio", extends = pybevy_core::PyAsset, skip_from_py_object)]
#[derive(Debug)]
pub struct PyPitch {
    pub(crate) storage: AssetStorage<Pitch>,
}

#[pymethods]
impl PyPitch {
    #[new]
    #[pyo3(signature = (frequency, duration))]
    pub fn new(frequency: f32, duration: f64) -> PyResult<PyClassInitializer<Self>> {
        validate_frequency(frequency)?;
        let pitch = Pitch::new(frequency, duration_from_secs_f64(duration)?);
        Ok(Self::from_owned(pitch).into())
    }

    #[getter]
    pub fn frequency(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.frequency)
    }

    #[setter]
    pub fn set_frequency(&mut self, frequency: f32) -> PyResult<()> {
        validate_frequency(frequency)?;
        self.as_mut()?.frequency = frequency;
        Ok(())
    }

    #[getter]
    pub fn duration(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.duration.as_secs_f64())
    }

    #[setter]
    pub fn set_duration(&mut self, duration: f64) -> PyResult<()> {
        self.as_mut()?.duration = duration_from_secs_f64(duration)?;
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
