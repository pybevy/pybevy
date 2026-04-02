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

pub use audio_player::PyAudioPlayer;
pub use audio_sink::PyAudioSink;
pub use audio_source::PyAudioSource;
use bevy::{app::App, audio::AudioPlugin};
pub use default_spatial_scale::PyDefaultSpatialScale;
pub use global_volume::PyGlobalVolume;
pub use pitch::PyPitch;
pub use playback_mode::PyPlaybackMode;
pub use playback_settings::PyPlaybackSettings;
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::plugin_storage;
use pyo3::prelude::*;
pub use spatial_audio_sink::PySpatialAudioSink;
pub use spatial_listener::PySpatialListener;
pub use spatial_scale::PySpatialScale;
pub use volume::PyVolume;

#[plugin_storage(AudioPlugin)]
#[pyclass(name = "AudioPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyAudioPlugin;

#[pymethods]
impl PyAudioPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyAudioPlugin, PyPlugin)
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
    m.add_class::<PyAudioPlayer>()?;
    m.add_class::<PyAudioSink>()?;
    m.add_class::<PyAudioSource>()?;
    m.add_class::<PyPitch>()?;
    m.add_class::<PyPlaybackSettings>()?;
    m.add_class::<PySpatialListener>()?;
    m.add_class::<PySpatialAudioSink>()?;
    m.add_class::<PyGlobalVolume>()?;
    m.add_class::<PyDefaultSpatialScale>()?;
    m.add_class::<PyPlaybackMode>()?;
    m.add_class::<PyVolume>()?;
    m.add_class::<PySpatialScale>()?;
    parent.add_submodule(&m)
}
