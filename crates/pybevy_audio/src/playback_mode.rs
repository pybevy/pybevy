use bevy::audio::PlaybackMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(PlaybackMode)]
#[pyclass(
    name = "PlaybackMode",
    module = "pybevy.audio",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyPlaybackMode {
    Once,
    Loop,
    Despawn,
    Remove,
}
