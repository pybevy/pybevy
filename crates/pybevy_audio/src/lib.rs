pub mod audio_player;
pub mod audio_sink;
pub mod audio_source;
pub mod default_spatial_scale;
pub mod global_volume;
pub mod pitch;
pub mod playback_mode;
pub mod playback_settings;
pub mod spatial_audio_sink;
pub mod spatial_listener;
pub mod spatial_scale;
pub mod volume;

use bevy::{app::App, audio::AudioPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        PyAudioPlugin, audio_player::PyAudioPlayer, audio_sink::PyAudioSink,
        audio_source::PyAudioSource, global_volume::PyGlobalVolume, pitch::PyPitch,
        playback_mode::PyPlaybackMode, playback_settings::PyPlaybackSettings,
        spatial_audio_sink::PySpatialAudioSink, spatial_listener::PySpatialListener,
        spatial_scale::PySpatialScale, volume::PyVolume,
    };
}

#[pyplugin(AudioPlugin)]
#[pyclass(name = "AudioPlugin", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyAudioPlugin;

#[pymethods]
impl PyAudioPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyAudioPlugin, PyPlugin).into()
    }
}

impl Default for PyAudioPlugin {
    fn default() -> Self {
        PyAudioPlugin
    }
}

impl PluginBuild for PyAudioPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(AudioPlugin::default());
        Ok(())
    }
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "audio")?;
    m.add_class::<PyAudioPlugin>()?;
    m.add_class::<audio_player::PyAudioPlayer>()?;
    m.add_class::<audio_sink::PyAudioSink>()?;
    m.add_class::<audio_source::PyAudioSource>()?;
    m.add_class::<pitch::PyPitch>()?;
    m.add_class::<playback_settings::PyPlaybackSettings>()?;
    m.add_class::<spatial_listener::PySpatialListener>()?;
    m.add_class::<spatial_audio_sink::PySpatialAudioSink>()?;
    m.add_class::<global_volume::PyGlobalVolume>()?;
    m.add_class::<default_spatial_scale::PyDefaultSpatialScale>()?;
    m.add_class::<playback_mode::PyPlaybackMode>()?;
    m.add_class::<volume::PyVolume>()?;
    m.add_class::<spatial_scale::PySpatialScale>()?;
    parent.add_submodule(&m)
}
