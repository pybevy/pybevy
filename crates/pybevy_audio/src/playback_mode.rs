use bevy::audio::PlaybackMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(PlaybackMode)]
#[pyclass(name = "PlaybackMode", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyPlaybackMode {
    Once,
    Loop,
    Despawn,
    Remove,
}
