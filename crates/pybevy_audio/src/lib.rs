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
use bevy::{
    app::{App, Plugin},
    audio::{
        AudioPlayer, AudioSink, AudioSource, DefaultSpatialScale, GlobalVolume, Pitch,
        PlaybackSettings, SpatialAudioSink, SpatialListener,
    },
};
pub use default_spatial_scale::PyDefaultSpatialScale;
pub use global_volume::PyGlobalVolume;
pub use pitch::PyPitch;
pub use playback_mode::PyPlaybackMode;
pub use playback_settings::PyPlaybackSettings;
use pybevy_core::{
    DynamicAssetRegistry, DynamicComponentRegistry, DynamicResourceRegistry, PyPlugin,
    plugin::plugin_registry, registry::global_registry,
};
use pybevy_macros::{asset_bridge, component_bridge, plugin_bridge, resource_bridge};
use pyo3::prelude::*;
pub use spatial_audio_sink::PySpatialAudioSink;
pub use spatial_listener::PySpatialListener;
pub use spatial_scale::PySpatialScale;
pub use volume::PyVolume;

component_bridge!(AudioPlayer<AudioSource>, PyAudioPlayer, "AudioPlayer");
component_bridge!(AudioSink, PyAudioSink, no_insert);
component_bridge!(
    PlaybackSettings,
    PyPlaybackSettings,
    view_fields = [speed, paused, muted, spatial]
);
component_bridge!(SpatialListener, PySpatialListener);
component_bridge!(SpatialAudioSink, PySpatialAudioSink, no_insert);

resource_bridge!(GlobalVolume, PyGlobalVolume);
resource_bridge!(DefaultSpatialScale, PyDefaultSpatialScale);

asset_bridge!(AudioSource, PyAudioSource);
asset_bridge!(Pitch, PyPitch);

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

plugin_bridge!(PyAudioPlugin, bevy::audio::AudioPlugin);

pub struct PyBevyAudioPlugin;

impl Plugin for PyBevyAudioPlugin {
    fn build(&self, app: &mut App) {
        global_registry::register_component_bridge(AudioPlayerBridge);
        global_registry::register_component_bridge(AudioSinkBridge);
        global_registry::register_component_bridge(PlaybackSettingsBridge);
        global_registry::register_component_bridge(SpatialListenerBridge);
        global_registry::register_component_bridge(SpatialAudioSinkBridge);
        global_registry::register_resource_bridge(GlobalVolumeBridge);
        global_registry::register_resource_bridge(DefaultSpatialScaleBridge);
        global_registry::register_asset_bridge(AudioSourceBridge);
        global_registry::register_asset_bridge(PitchBridge);

        if let Some(mut registry) = app
            .world_mut()
            .get_resource_mut::<DynamicComponentRegistry>()
        {
            registry.register(AudioPlayerBridge);
            registry.register(AudioSinkBridge);
            registry.register(PlaybackSettingsBridge);
            registry.register(SpatialListenerBridge);
            registry.register(SpatialAudioSinkBridge);
        }

        if let Some(mut registry) = app
            .world_mut()
            .get_resource_mut::<DynamicResourceRegistry>()
        {
            registry.register(GlobalVolumeBridge);
            registry.register(DefaultSpatialScaleBridge);
        }

        if let Some(mut registry) = app.world_mut().get_resource_mut::<DynamicAssetRegistry>() {
            registry.register(AudioSourceBridge);
            registry.register(PitchBridge);
        }
    }
}

pub fn register_audio_bridges() {
    global_registry::register_component_bridge(AudioPlayerBridge);
    global_registry::register_component_bridge(AudioSinkBridge);
    global_registry::register_component_bridge(PlaybackSettingsBridge);
    global_registry::register_component_bridge(SpatialListenerBridge);
    global_registry::register_component_bridge(SpatialAudioSinkBridge);
    register_playback_settings_batch();

    global_registry::register_resource_bridge(GlobalVolumeBridge);
    global_registry::register_resource_bridge(DefaultSpatialScaleBridge);

    global_registry::register_asset_bridge(AudioSourceBridge);
    global_registry::register_asset_bridge(PitchBridge);

    plugin_registry::register_plugin_bridge(AudioPluginBridge);
}

pub fn add_audio_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_audio_bridges();

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

    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "audio")?;
    add_audio_classes(&m)?;
    parent.add_submodule(&m)
}
