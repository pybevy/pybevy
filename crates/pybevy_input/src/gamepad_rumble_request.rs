use std::time::Duration;

use bevy::{
    ecs::entity::Entity,
    input::gamepad::{GamepadRumbleIntensity, GamepadRumbleRequest},
};
use pybevy_core::PyEntity;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::gamepad_rumble_intensity::PyGamepadRumbleIntensity;

/// Mirrors Bevy's `GamepadRumbleRequest` message enum. The base is
/// non-constructible; both variants travel through the native
/// `Messages<GamepadRumbleRequest>` channel.
#[pyenum(GamepadRumbleRequest, message, writable, no_eq, no_debug)]
#[pyclass(module = "pybevy.input", name = "GamepadRumbleRequest")]
pub enum PyGamepadRumbleRequest {
    Add {
        duration: Duration,
        #[py_type(PyGamepadRumbleIntensity)]
        #[py_try_into]
        intensity: GamepadRumbleIntensity,
        #[py_type(PyEntity)]
        gamepad: Entity,
    },
    Stop {
        #[py_type(PyEntity)]
        gamepad: Entity,
    },
}

impl TryFrom<&PyGamepadRumbleRequest> for GamepadRumbleRequest {
    type Error = pyo3::PyErr;

    fn try_from(value: &PyGamepadRumbleRequest) -> pyo3::PyResult<Self> {
        Ok(value.inner.clone())
    }
}
