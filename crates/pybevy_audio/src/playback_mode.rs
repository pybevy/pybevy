use bevy::audio::PlaybackMode;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(PlaybackMode)]
#[pyclass(name = "PlaybackMode", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyPlaybackMode {
    Once,
    Loop,
    Despawn,
    Remove,
}
