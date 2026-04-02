use bevy::input::gamepad::{GamepadConnection, GamepadEvent};
use pyo3::prelude::*;

use crate::{gamepad_axis::PyGamepadAxis, gamepad_button::PyGamepadButton};

#[pyclass(name = "GamepadEvent")]
#[derive(Debug, Clone)]
pub enum PyGamepadEvent {
    Connection {
        connected: bool,
        name: Option<String>,
        vendor_id: Option<u16>,
        product_id: Option<u16>,
    },
    Button {
        button: PyGamepadButton,
        value: f32,
    },
    Axis {
        axis: PyGamepadAxis,
        value: f32,
    },
}

impl PyGamepadEvent {
    pub fn from_bevy(event: &GamepadEvent) -> Self {
        match event {
            GamepadEvent::Connection(conn) => {
                let (connected, name, vendor_id, product_id) = match &conn.connection {
                    GamepadConnection::Connected {
                        name,
                        vendor_id,
                        product_id,
                    } => (true, Some(name.clone()), *vendor_id, *product_id),
                    GamepadConnection::Disconnected => (false, None, None, None),
                };
                PyGamepadEvent::Connection {
                    connected,
                    name,
                    vendor_id,
                    product_id,
                }
            }
            GamepadEvent::Button(btn) => PyGamepadEvent::Button {
                button: btn.button.into(),
                value: btn.value,
            },
            GamepadEvent::Axis(axis) => PyGamepadEvent::Axis {
                axis: axis.axis.into(),
                value: axis.value,
            },
        }
    }
}

#[pymethods]
impl PyGamepadEvent {
    fn __repr__(&self) -> String {
        match self {
            PyGamepadEvent::Connection {
                connected,
                name,
                vendor_id,
                product_id,
            } => {
                if *connected {
                    let name_str = match name {
                        Some(n) => format!("\"{}\"", n),
                        None => "None".to_string(),
                    };
                    let vendor_str = match vendor_id {
                        Some(v) => format!("0x{:04X}", v),
                        None => "None".to_string(),
                    };
                    let product_str = match product_id {
                        Some(p) => format!("0x{:04X}", p),
                        None => "None".to_string(),
                    };
                    format!(
                        "GamepadEvent.Connection(connected=True, name={}, vendor_id={}, product_id={})",
                        name_str, vendor_str, product_str
                    )
                } else {
                    "GamepadEvent.Connection(connected=False)".to_string()
                }
            }
            PyGamepadEvent::Button { button, value } => {
                format!("GamepadEvent.Button(button={:?}, value={})", button, value)
            }
            PyGamepadEvent::Axis { axis, value } => {
                format!("GamepadEvent.Axis(axis={:?}, value={})", axis, value)
            }
        }
    }
}
