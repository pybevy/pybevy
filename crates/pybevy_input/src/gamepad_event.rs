use bevy::input::gamepad::{
    GamepadAxisChangedEvent, GamepadButtonChangedEvent, GamepadConnectionEvent, GamepadEvent,
    RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent, RawGamepadEvent,
};
use pybevy_core::message_payload;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::{
    gamepad_events::{
        PyGamepadAxisChangedEvent, PyGamepadButtonChangedEvent, PyGamepadConnectionEvent,
    },
    gamepad_raw_events::{PyRawGamepadAxisChangedEvent, PyRawGamepadButtonChangedEvent},
};

message_payload!(
    ConnectionPayload,
    GamepadConnectionEvent,
    PyGamepadConnectionEvent
);
message_payload!(
    ButtonPayload,
    GamepadButtonChangedEvent,
    PyGamepadButtonChangedEvent
);
message_payload!(
    AxisPayload,
    GamepadAxisChangedEvent,
    PyGamepadAxisChangedEvent
);
message_payload!(
    RawButtonPayload,
    RawGamepadButtonChangedEvent,
    PyRawGamepadButtonChangedEvent
);
message_payload!(
    RawAxisPayload,
    RawGamepadAxisChangedEvent,
    PyRawGamepadAxisChangedEvent
);

#[pyenum(GamepadEvent, message, writable)]
#[pyclass(name = "GamepadEvent", module = "pybevy.input")]
pub enum PyGamepadEvent {
    #[py_bevy(tuple)]
    Connection {
        #[py_type(ConnectionPayload)]
        value: GamepadConnectionEvent,
    },
    #[py_bevy(tuple)]
    Button {
        #[py_type(ButtonPayload)]
        value: GamepadButtonChangedEvent,
    },
    #[py_bevy(tuple)]
    Axis {
        #[py_type(AxisPayload)]
        value: GamepadAxisChangedEvent,
    },
}

impl From<&PyGamepadEvent> for GamepadEvent {
    fn from(value: &PyGamepadEvent) -> Self {
        value.clone().into()
    }
}

#[pyenum(RawGamepadEvent, message, writable)]
#[pyclass(name = "RawGamepadEvent", module = "pybevy.input")]
pub enum PyRawGamepadEvent {
    #[py_bevy(tuple)]
    Connection {
        #[py_type(ConnectionPayload)]
        value: GamepadConnectionEvent,
    },
    #[py_bevy(tuple)]
    Button {
        #[py_type(RawButtonPayload)]
        value: RawGamepadButtonChangedEvent,
    },
    #[py_bevy(tuple)]
    Axis {
        #[py_type(RawAxisPayload)]
        value: RawGamepadAxisChangedEvent,
    },
}

impl From<&PyRawGamepadEvent> for RawGamepadEvent {
    fn from(value: &PyRawGamepadEvent) -> Self {
        value.clone().into()
    }
}
