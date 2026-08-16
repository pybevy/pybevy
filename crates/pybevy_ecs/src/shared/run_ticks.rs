//! The change-detection tick window shared by every per-run construct.

use bevy::ecs::change_detection::Tick;

/// The change-detection window for one system run: `last_run` opens it and
/// `this_run` is the freshly advanced world tick every query, view, and
/// write-back observes.
#[derive(Clone, Copy, Debug)]
pub struct RunTicks {
    pub last_run: Tick,
    pub this_run: Tick,
}
