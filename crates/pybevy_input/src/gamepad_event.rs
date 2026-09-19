use bevy::input::gamepad::{
    GamepadAxisChangedEvent, GamepadButtonChangedEvent, GamepadConnectionEvent, GamepadEvent,
};
use pybevy_core::PyMessage;
use pybevy_macros::pyenum;
use pyo3::{Borrowed, prelude::*};

use crate::gamepad_events::{
    PyGamepadAxisChangedEvent, PyGamepadButtonChangedEvent, PyGamepadConnectionEvent,
};

// Message subclasses need their complete Python initializer at the payload boundary.
macro_rules! message_payload {
    ($payload:ident, $native:ty, $wrapper:ty) => {
        pub struct $payload($native);

        impl From<$native> for $payload {
            fn from(value: $native) -> Self {
                Self(value)
            }
        }

        impl From<$payload> for $native {
            fn from(value: $payload) -> Self {
                value.0
            }
        }

        impl<'py> IntoPyObject<'py> for $payload {
            type Target = $wrapper;
            type Output = Bound<'py, $wrapper>;
            type Error = PyErr;

            fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
                Py::new(py, (<$wrapper>::from(&self.0), PyMessage))
                    .map(|value| value.into_bound(py))
            }
        }

        impl FromPyObject<'_, '_> for $payload {
            type Error = PyErr;

            fn extract(value: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
                let wrapper = value.extract::<PyRef<'_, $wrapper>>()?;
                <$native>::try_from(&*wrapper).map(Self)
            }
        }
    };
}

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
