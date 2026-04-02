use bevy::input::gamepad::{
    GamepadAxisChangedEvent as GamepadAxisChanged,
    GamepadButtonChangedEvent as GamepadButtonChanged, GamepadButtonStateChangedEvent,
    GamepadConnection, GamepadConnectionEvent,
};
pub use pybevy_core::PyMessage;
use pybevy_macros::message_storage;
use pyo3::prelude::*;

use crate::{
    button_state::PyButtonState, gamepad_axis::PyGamepadAxis, gamepad_button::PyGamepadButton,
};

#[message_storage(GamepadButtonChanged)]
#[pyclass(name = "GamepadButtonChanged", extends = PyMessage)]
#[derive(Debug, Clone)]
pub struct PyGamepadButtonChanged {
    pub button: PyGamepadButton,
    pub value: f32,
}

impl PyGamepadButtonChanged {
    pub fn from_bevy(event: &GamepadButtonChanged) -> (Self, PyMessage) {
        (
            PyGamepadButtonChanged {
                button: event.button.into(),
                value: event.value,
            },
            PyMessage,
        )
    }
}

impl From<&GamepadButtonChanged> for PyGamepadButtonChanged {
    fn from(event: &GamepadButtonChanged) -> Self {
        PyGamepadButtonChanged {
            button: event.button.into(),
            value: event.value,
        }
    }
}

#[pymethods]
impl PyGamepadButtonChanged {
    #[new]
    fn new(button: PyGamepadButton, value: f32) -> (Self, PyMessage) {
        (PyGamepadButtonChanged { button, value }, PyMessage)
    }

    #[getter]
    fn button(&self) -> PyGamepadButton {
        self.button
    }

    #[getter]
    fn value(&self) -> f32 {
        self.value
    }

    fn __repr__(&self) -> String {
        format!(
            "GamepadButtonChanged(button={:?}, value={})",
            self.button, self.value
        )
    }
}

#[message_storage(GamepadAxisChanged)]
#[pyclass(name = "GamepadAxisChanged", extends = PyMessage)]
#[derive(Debug, Clone)]
pub struct PyGamepadAxisChanged {
    pub axis: PyGamepadAxis,
    pub value: f32,
}

impl PyGamepadAxisChanged {
    pub fn from_bevy(event: &GamepadAxisChanged) -> (Self, PyMessage) {
        (
            PyGamepadAxisChanged {
                axis: event.axis.into(),
                value: event.value,
            },
            PyMessage,
        )
    }
}

impl From<&GamepadAxisChanged> for PyGamepadAxisChanged {
    fn from(event: &GamepadAxisChanged) -> Self {
        PyGamepadAxisChanged {
            axis: event.axis.into(),
            value: event.value,
        }
    }
}

#[pymethods]
impl PyGamepadAxisChanged {
    #[new]
    fn new(axis: PyGamepadAxis, value: f32) -> (Self, PyMessage) {
        (PyGamepadAxisChanged { axis, value }, PyMessage)
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
            "GamepadAxisChanged(axis={:?}, value={})",
            self.axis, self.value
        )
    }
}

#[message_storage(GamepadConnectionEvent)]
#[pyclass(name = "GamepadConnection", extends = PyMessage, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyGamepadConnection {
    pub connected: bool,
    pub name: Option<String>,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
}

impl PyGamepadConnection {
    pub fn from_bevy(event: &GamepadConnectionEvent) -> (Self, PyMessage) {
        let (connected, name, vendor_id, product_id) = match &event.connection {
            GamepadConnection::Connected {
                name,
                vendor_id,
                product_id,
            } => (true, Some(name.clone()), *vendor_id, *product_id),
            GamepadConnection::Disconnected => (false, None, None, None),
        };

        (
            PyGamepadConnection {
                connected,
                name,
                vendor_id,
                product_id,
            },
            PyMessage,
        )
    }
}

impl From<&GamepadConnectionEvent> for PyGamepadConnection {
    fn from(event: &GamepadConnectionEvent) -> Self {
        let (connected, name, vendor_id, product_id) = match &event.connection {
            GamepadConnection::Connected {
                name,
                vendor_id,
                product_id,
            } => (true, Some(name.clone()), *vendor_id, *product_id),
            GamepadConnection::Disconnected => (false, None, None, None),
        };

        PyGamepadConnection {
            connected,
            name,
            vendor_id,
            product_id,
        }
    }
}

#[pymethods]
impl PyGamepadConnection {
    #[new]
    #[pyo3(signature = (connected, name=None, vendor_id=None, product_id=None))]
    fn new(
        connected: bool,
        name: Option<String>,
        vendor_id: Option<u16>,
        product_id: Option<u16>,
    ) -> (Self, PyMessage) {
        (
            PyGamepadConnection {
                connected,
                name,
                vendor_id,
                product_id,
            },
            PyMessage,
        )
    }

    #[getter]
    fn connected(&self) -> bool {
        self.connected
    }

    #[getter]
    fn name(&self) -> Option<String> {
        self.name.clone()
    }

    #[getter]
    fn vendor_id(&self) -> Option<u16> {
        self.vendor_id
    }

    #[getter]
    fn product_id(&self) -> Option<u16> {
        self.product_id
    }

    fn __repr__(&self) -> String {
        if self.connected {
            let name_str = match &self.name {
                Some(n) => n.as_str(),
                None => "Unknown",
            };
            let vendor_str = match self.vendor_id {
                Some(v) => format!("0x{:04X}", v),
                None => "None".to_string(),
            };
            let product_str = match self.product_id {
                Some(p) => format!("0x{:04X}", p),
                None => "None".to_string(),
            };
            format!(
                "GamepadConnection(connected=True, name=\"{}\", vendor_id={}, product_id={})",
                name_str, vendor_str, product_str
            )
        } else {
            "GamepadConnection(connected=False)".to_string()
        }
    }
}

#[message_storage(GamepadButtonStateChangedEvent)]
#[pyclass(name = "GamepadButtonStateChanged", extends = PyMessage, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyGamepadButtonStateChanged {
    pub button: PyGamepadButton,
    pub state: PyButtonState,
}

impl PyGamepadButtonStateChanged {
    pub fn from_bevy(event: &GamepadButtonStateChangedEvent) -> (Self, PyMessage) {
        (
            PyGamepadButtonStateChanged {
                button: event.button.into(),
                state: event.state.into(),
            },
            PyMessage,
        )
    }
}

impl From<&GamepadButtonStateChangedEvent> for PyGamepadButtonStateChanged {
    fn from(event: &GamepadButtonStateChangedEvent) -> Self {
        PyGamepadButtonStateChanged {
            button: event.button.into(),
            state: event.state.into(),
        }
    }
}

#[pymethods]
impl PyGamepadButtonStateChanged {
    #[new]
    fn new(button: PyGamepadButton, state: PyButtonState) -> (Self, PyMessage) {
        (PyGamepadButtonStateChanged { button, state }, PyMessage)
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
            "GamepadButtonStateChanged(button={:?}, state={:?})",
            self.button, self.state
        )
    }
}

// TODO: Review later. PyGamepadEvent remains in main crate (src/input/events.rs) as it uses
// PyGamepad, which has ComponentStorage that can't easily implement Debug/Clone in feature crate.
