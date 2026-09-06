use std::time::Duration;

use bevy::{
    audio::{AudioSinkPlayback, SpatialAudioSink},
    transform::components::Transform,
};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::vec3::PyVec3;
use pybevy_transform::transform::PyTransform;
use pyo3::{exceptions::PyRuntimeError, prelude::*};

use crate::{audio_sink::validate_sink_speed, volume::PyVolume};

#[pycomponent(SpatialAudioSink, no_clone, bridge, no_reflect)]
#[pyclass(name = "SpatialAudioSink", module = "pybevy.audio", extends = PyComponent)]
pub struct PySpatialAudioSink {
    pub(crate) storage: ComponentStorage<SpatialAudioSink>,
}

#[pymethods]
impl PySpatialAudioSink {
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

    pub fn set_ears_position(&self, left_position: PyVec3, right_position: PyVec3) -> PyResult<()> {
        self.as_ref()?
            .set_ears_position(left_position.try_into()?, right_position.try_into()?);
        Ok(())
    }

    pub fn set_listener_position(&self, position: &PyTransform, gap: f32) -> PyResult<()> {
        let bevy_transform: Transform = *position.as_ref()?;
        self.as_ref()?.set_listener_position(bevy_transform, gap);
        Ok(())
    }

    pub fn set_emitter_position(&self, position: PyVec3) -> PyResult<()> {
        self.as_ref()?.set_emitter_position(position.try_into()?);
        Ok(())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(sink) => format!(
                "SpatialAudioSink(volume={}, speed={}, paused={}, muted={})",
                sink.volume().to_linear(),
                sink.speed(),
                if sink.is_paused() { "True" } else { "False" },
                if sink.is_muted() { "True" } else { "False" },
            ),
            Err(_) => "SpatialAudioSink(<invalid>)".to_string(),
        }
    }
}
