use std::time::Duration;

use bevy::audio::{AudioSink, AudioSinkPlayback};
use pybevy_core::{
    ComponentStorage, PyComponent,
    domain::{DurationInput, FINITE_NON_NEGATIVE, POSITIVE, check_for},
    public_error::{audio_seek_beyond_speed_range, audio_seek_failed},
    repr::float_repr,
};
use pybevy_macros::pycomponent;
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
};

use crate::volume::PyVolume;

/// rodio scales a seek position by the speed with a panicking `Duration::mul_f32`.
pub(crate) fn check_seek(position: Duration, speed: f32) -> PyResult<()> {
    if Duration::try_from_secs_f64(position.as_secs_f64() * f64::from(speed)).is_err() {
        return Err(PyValueError::new_err(audio_seek_beyond_speed_range(
            position, speed,
        )));
    }
    Ok(())
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
        // rodio's seek and duration math panics for a zero or negative speed.
        let speed = check_for::<Self, _>("speed", speed, POSITIVE)?;
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

    pub fn try_seek(&self, pos: DurationInput) -> PyResult<()> {
        let pos = check_for::<Self, _>("pos", pos, FINITE_NON_NEGATIVE)?.stored();
        let sink = self.as_ref()?;
        check_seek(pos, sink.speed())?;
        sink.try_seek(pos)
            .map_err(|e| PyRuntimeError::new_err(audio_seek_failed(e)))?;
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
                float_repr(sink.volume().to_linear()),
                float_repr(sink.speed()),
                if sink.is_paused() { "True" } else { "False" },
                if sink.is_muted() { "True" } else { "False" },
            ),
            Err(_) => "AudioSink(<invalid>)".to_string(),
        }
    }
}
