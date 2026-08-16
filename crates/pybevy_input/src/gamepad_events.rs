use bevy::{
    ecs::entity::Entity,
    input::gamepad::{
        GamepadAxisChangedEvent, GamepadButtonChangedEvent, GamepadButtonStateChangedEvent,
        GamepadConnection, GamepadConnectionEvent,
    },
};
use pybevy_core::PyEntity;
pub use pybevy_core::PyMessage;
use pybevy_macros::{pyenum, pymessage};
use pyo3::prelude::*;

use crate::{
    button_state::PyButtonState, gamepad_axis::PyGamepadAxis, gamepad_button::PyGamepadButton,
};

#[pymessage(GamepadButtonChangedEvent)]
#[pyclass(name = "GamepadButtonChangedEvent", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyGamepadButtonChangedEvent {
    pub entity: PyEntity,
    pub button: PyGamepadButton,
    pub state: PyButtonState,
    pub value: f32,
}

impl PyGamepadButtonChangedEvent {
    pub fn from_bevy(event: &GamepadButtonChangedEvent) -> (Self, PyMessage) {
        (
            PyGamepadButtonChangedEvent {
                entity: event.entity.into(),
                button: event.button.into(),
                state: event.state.into(),
                value: event.value,
            },
            PyMessage,
        )
    }
}

impl From<&GamepadButtonChangedEvent> for PyGamepadButtonChangedEvent {
    fn from(event: &GamepadButtonChangedEvent) -> Self {
        PyGamepadButtonChangedEvent {
            entity: event.entity.into(),
            button: event.button.into(),
            state: event.state.into(),
            value: event.value,
        }
    }
}

#[pymethods]
impl PyGamepadButtonChangedEvent {
    #[new]
    #[pyo3(signature = (button, value, *, state = PyButtonState::Released(), entity = None))]
    fn new(
        button: PyGamepadButton,
        value: f32,
        state: PyButtonState,
        entity: Option<PyEntity>,
    ) -> PyClassInitializer<Self> {
        (
            PyGamepadButtonChangedEvent {
                entity: entity.unwrap_or(Entity::PLACEHOLDER.into()),
                button,
                state,
                value,
            },
            PyMessage,
        )
            .into()
    }

    #[getter]
    fn entity(&self) -> PyEntity {
        self.entity
    }

    #[getter]
    fn button(&self) -> PyGamepadButton {
        self.button
    }

    #[getter]
    fn state(&self) -> PyButtonState {
        self.state
    }

    #[getter]
    fn value(&self) -> f32 {
        self.value
    }

    fn __repr__(&self) -> String {
        format!(
            "GamepadButtonChangedEvent(button={:?}, value={})",
            self.button, self.value
        )
    }
}

#[pymessage(GamepadAxisChangedEvent)]
#[pyclass(name = "GamepadAxisChangedEvent", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyGamepadAxisChangedEvent {
    pub entity: PyEntity,
    pub axis: PyGamepadAxis,
    pub value: f32,
}

impl PyGamepadAxisChangedEvent {
    pub fn from_bevy(event: &GamepadAxisChangedEvent) -> (Self, PyMessage) {
        (
            PyGamepadAxisChangedEvent {
                entity: event.entity.into(),
                axis: event.axis.into(),
                value: event.value,
            },
            PyMessage,
        )
    }
}

impl From<&GamepadAxisChangedEvent> for PyGamepadAxisChangedEvent {
    fn from(event: &GamepadAxisChangedEvent) -> Self {
        PyGamepadAxisChangedEvent {
            entity: event.entity.into(),
            axis: event.axis.into(),
            value: event.value,
        }
    }
}

#[pymethods]
impl PyGamepadAxisChangedEvent {
    #[new]
    #[pyo3(signature = (axis, value, *, entity = None))]
    fn new(axis: PyGamepadAxis, value: f32, entity: Option<PyEntity>) -> PyClassInitializer<Self> {
        (
            PyGamepadAxisChangedEvent {
                entity: entity.unwrap_or(Entity::PLACEHOLDER.into()),
                axis,
                value,
            },
            PyMessage,
        )
            .into()
    }

    #[getter]
    fn entity(&self) -> PyEntity {
        self.entity
    }

    #[getter]
    fn axis(&self) -> PyGamepadAxis {
        self.axis
    }

    #[getter]
    fn value(&self) -> f32 {
        self.value
    }

    fn __repr__(&self) -> String {
        format!(
            "GamepadAxisChangedEvent(axis={:?}, value={})",
            self.axis, self.value
        )
    }
}

#[pyenum(GamepadConnection, empty_tuple, no_repr)]
#[pyclass(
    name = "GamepadConnection",
    module = "pybevy.input",
    eq,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyGamepadConnection {
    Connected {
        name: String,
        vendor_id: Option<u16>,
        product_id: Option<u16>,
    },
    Disconnected(),
}

#[pymethods]
impl PyGamepadConnection {
    fn __repr__(&self) -> String {
        match self {
            PyGamepadConnection::Connected {
                name,
                vendor_id,
                product_id,
            } => format!(
                "GamepadConnection.Connected(name={name:?}, vendor_id={}, product_id={})",
                optional_id(*vendor_id),
                optional_id(*product_id)
            ),
            PyGamepadConnection::Disconnected() => "GamepadConnection.Disconnected()".to_string(),
        }
    }
}

fn optional_id(id: Option<u16>) -> String {
    id.map_or_else(|| "None".to_string(), |id| id.to_string())
}

#[pymessage(GamepadConnectionEvent)]
#[pyclass(name = "GamepadConnectionEvent", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyGamepadConnectionEvent {
    pub gamepad: PyEntity,
    pub connection: PyGamepadConnection,
}

impl PyGamepadConnectionEvent {
    pub fn from_bevy(event: &GamepadConnectionEvent) -> (Self, PyMessage) {
        (Self::from(event), PyMessage)
    }
}

impl From<&GamepadConnectionEvent> for PyGamepadConnectionEvent {
    fn from(event: &GamepadConnectionEvent) -> Self {
        PyGamepadConnectionEvent {
            gamepad: event.gamepad.into(),
            connection: event.connection.clone().into(),
        }
    }
}

#[pymethods]
impl PyGamepadConnectionEvent {
    #[new]
    #[pyo3(signature = (gamepad, connection))]
    fn new(gamepad: PyEntity, connection: PyGamepadConnection) -> PyClassInitializer<Self> {
        (
            PyGamepadConnectionEvent {
                gamepad,
                connection,
            },
            PyMessage,
        )
            .into()
    }

    #[getter]
    fn gamepad(&self) -> PyEntity {
        self.gamepad
    }

    #[getter]
    fn connection(&self) -> PyGamepadConnection {
        self.connection.clone()
    }

    fn connected(&self) -> bool {
        matches!(self.connection, PyGamepadConnection::Connected { .. })
    }

    fn disconnected(&self) -> bool {
        matches!(self.connection, PyGamepadConnection::Disconnected())
    }

    fn __repr__(&self) -> String {
        format!(
            "GamepadConnectionEvent(gamepad={:?}, connection={})",
            self.gamepad,
            self.connection.__repr__()
        )
    }
}

#[pymessage(GamepadButtonStateChangedEvent)]
#[pyclass(name = "GamepadButtonStateChangedEvent", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyGamepadButtonStateChangedEvent {
    pub entity: PyEntity,
    pub button: PyGamepadButton,
    pub state: PyButtonState,
}

impl PyGamepadButtonStateChangedEvent {
    pub fn from_bevy(event: &GamepadButtonStateChangedEvent) -> (Self, PyMessage) {
        (
            PyGamepadButtonStateChangedEvent {
                entity: event.entity.into(),
                button: event.button.into(),
                state: event.state.into(),
            },
            PyMessage,
        )
    }
}

impl From<&GamepadButtonStateChangedEvent> for PyGamepadButtonStateChangedEvent {
    fn from(event: &GamepadButtonStateChangedEvent) -> Self {
        PyGamepadButtonStateChangedEvent {
            entity: event.entity.into(),
            button: event.button.into(),
            state: event.state.into(),
        }
    }
}

#[pymethods]
impl PyGamepadButtonStateChangedEvent {
    #[new]
    #[pyo3(signature = (button, state, *, entity = None))]
    fn new(
        button: PyGamepadButton,
        state: PyButtonState,
        entity: Option<PyEntity>,
    ) -> PyClassInitializer<Self> {
        (
            PyGamepadButtonStateChangedEvent {
                entity: entity.unwrap_or(Entity::PLACEHOLDER.into()),
                button,
                state,
            },
            PyMessage,
        )
            .into()
    }

    #[getter]
    fn entity(&self) -> PyEntity {
        self.entity
    }

    #[getter]
    fn button(&self) -> PyGamepadButton {
        self.button
    }

    #[getter]
    fn state(&self) -> PyButtonState {
        self.state
    }

    fn __repr__(&self) -> String {
        format!(
            "GamepadButtonStateChangedEvent(button={:?}, state={:?})",
            self.button, self.state
        )
    }
}

// TODO: Review later. PyGamepadEvent remains in main crate (src/input/events.rs) as it uses
// PyGamepad, which has ComponentStorage that can't easily implement Debug/Clone in feature crate.
