use std::time::Duration;

use bevy::audio::{AudioSink, AudioSinkPlayback};
use pybevy_core::{ComponentStorage, PyComponent, public_error::SPEED_NON_FINITE};
use pybevy_macros::pycomponent;
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
};

use crate::volume::PyVolume;

/// Playback speed must be finite; negative (reverse) speed is allowed.
pub(crate) fn validate_sink_speed(speed: f32) -> PyResult<()> {
    if speed.is_finite() {
        Ok(())
    } else {
        Err(PyValueError::new_err(SPEED_NON_FINITE))
    }
}

#[pycomponent(AudioSink, no_clone, bridge, no_reflect)]
#[pyclass(name = "AudioSink", module = "pybevy.audio", extends = PyComponent)]
pub struct PyAudioSink {
    pub(crate) storage: ComponentStorage<AudioSink>,
}

#[pymethods]
impl PyAudioSink {
    pub fn play(&self) -> PyResult<()> {
        self.as_ref()?.play();
        Ok(())
    }

    pub fn pause(&self) -> PyResult<()> {
        self.as_ref()?.pause();
        Ok(())
    }

    pub fn stop(&self) -> PyResult<()> {
        self.as_ref()?.stop();
        Ok(())
    }

    pub fn volume(&self) -> PyResult<PyVolume> {
        Ok(self.as_ref()?.volume().into())
    }

    pub fn set_volume(&mut self, volume: &PyVolume) -> PyResult<()> {
        self.as_mut()?.set_volume((*volume).into());
        Ok(())
    }

    pub fn speed(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.speed())
    }

    pub fn set_speed(&self, speed: f32) -> PyResult<()> {
        validate_sink_speed(speed)?;
        self.as_ref()?.set_speed(speed);
        Ok(())
    }

    pub fn mute(&mut self) -> PyResult<()> {
        self.as_mut()?.mute();
        Ok(())
    }

    pub fn unmute(&mut self) -> PyResult<()> {
        self.as_mut()?.unmute();
        Ok(())
    }

    pub fn is_muted(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_muted())
    }

    pub fn is_paused(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_paused())
    }

    pub fn empty(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.empty())
    }

    pub fn position(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.position())
    }

    pub fn try_seek(&self, pos: Duration) -> PyResult<()> {
        self.as_ref()?
            .try_seek(pos)
            .map_err(|e| PyRuntimeError::new_err(format!("Seek error: {:?}", e)))?;
        Ok(())
    }

    pub fn toggle_playback(&self) -> PyResult<()> {
        self.as_ref()?.toggle_playback();
        Ok(())
    }

    pub fn toggle_mute(&mut self) -> PyResult<()> {
        self.as_mut()?.toggle_mute();
        Ok(())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(sink) => format!(
                "AudioSink(volume={}, speed={}, paused={}, muted={})",
                sink.volume().to_linear(),
                sink.speed(),
                if sink.is_paused() { "True" } else { "False" },
                if sink.is_muted() { "True" } else { "False" },
            ),
            Err(_) => "AudioSink(<invalid>)".to_string(),
        }
    }
}
