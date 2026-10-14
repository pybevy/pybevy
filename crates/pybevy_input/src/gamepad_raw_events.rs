use bevy::input::gamepad::{RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent};
use pybevy_core::{
    PyEntity, PyMessage,
    domain::{NOT_NAN, check_for},
    repr::float_repr,
};
use pybevy_macros::pymessage;
use pyo3::prelude::*;

use crate::{gamepad_axis::PyGamepadAxis, gamepad_button::PyGamepadButton};

#[pymessage(RawGamepadButtonChangedEvent, writable)]
#[pyclass(name = "RawGamepadButtonChangedEvent", module = "pybevy.input", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyRawGamepadButtonChangedEvent {
    gamepad: PyEntity,
    button: PyGamepadButton,
    value: f32,
}

impl From<&RawGamepadButtonChangedEvent> for PyRawGamepadButtonChangedEvent {
    fn from(event: &RawGamepadButtonChangedEvent) -> Self {
        Self {
            gamepad: event.gamepad.into(),
            button: event.button.into(),
            value: event.value,
        }
    }
}

impl TryFrom<&PyRawGamepadButtonChangedEvent> for RawGamepadButtonChangedEvent {
    type Error = PyErr;

    fn try_from(event: &PyRawGamepadButtonChangedEvent) -> PyResult<Self> {
        Ok(Self {
            gamepad: event.gamepad.into(),
            button: event.button.into(),
            value: event.value,
        })
    }
}

#[pymethods]
impl PyRawGamepadButtonChangedEvent {
    #[new]
    fn new(
        gamepad: PyEntity,
        button: PyGamepadButton,
        value: f32,
    ) -> PyResult<PyClassInitializer<Self>> {
        let value = check_for::<Self, _>("value", value, NOT_NAN)?;
        Ok((
            Self {
                gamepad,
                button,
                value,
            },
            PyMessage,
        )
            .into())
    }

    #[getter]
    fn gamepad(&self) -> PyEntity {
        self.gamepad
    }

    #[setter]
    fn set_gamepad(&mut self, gamepad: PyEntity) {
        self.gamepad = gamepad;
    }

    #[getter]
    fn button(&self) -> PyGamepadButton {
        self.button
    }

    #[setter]
    fn set_button(&mut self, button: PyGamepadButton) {
        self.button = button;
    }

    #[getter]
    fn value(&self) -> f32 {
        self.value
    }

    #[setter]
    fn set_value(&mut self, value: f32) -> PyResult<()> {
        self.value = check_for::<Self, _>("value", value, NOT_NAN)?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "RawGamepadButtonChangedEvent(gamepad={}, button={}, value={})",
            self.gamepad.__repr__(),
            self.button.__repr__(),
            float_repr(self.value)
        )
    }
}

#[pymessage(RawGamepadAxisChangedEvent, writable)]
#[pyclass(name = "RawGamepadAxisChangedEvent", module = "pybevy.input", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyRawGamepadAxisChangedEvent {
    gamepad: PyEntity,
    axis: PyGamepadAxis,
    value: f32,
}

impl From<&RawGamepadAxisChangedEvent> for PyRawGamepadAxisChangedEvent {
    fn from(event: &RawGamepadAxisChangedEvent) -> Self {
        Self {
            gamepad: event.gamepad.into(),
            axis: event.axis.into(),
            value: event.value,
        }
    }
}

impl TryFrom<&PyRawGamepadAxisChangedEvent> for RawGamepadAxisChangedEvent {
    type Error = PyErr;

    fn try_from(event: &PyRawGamepadAxisChangedEvent) -> PyResult<Self> {
        Ok(Self {
            gamepad: event.gamepad.into(),
            axis: event.axis.into(),
            value: event.value,
        })
    }
}

#[pymethods]
impl PyRawGamepadAxisChangedEvent {
    #[new]
    fn new(
        gamepad: PyEntity,
        axis: PyGamepadAxis,
        value: f32,
    ) -> PyResult<PyClassInitializer<Self>> {
        // Bevy's axis filter reaches `unreachable!()` for NaN.
        let value = check_for::<Self, _>("value", value, NOT_NAN)?;
        Ok((
            Self {
                gamepad,
                axis,
                value,
            },
            PyMessage,
        )
            .into())
    }

    #[getter]
    fn gamepad(&self) -> PyEntity {
        self.gamepad
    }

    #[setter]
    fn set_gamepad(&mut self, gamepad: PyEntity) {
        self.gamepad = gamepad;
    }

    #[getter]
    fn axis(&self) -> PyGamepadAxis {
        self.axis
    }

    #[setter]
    fn set_axis(&mut self, axis: PyGamepadAxis) {
        self.axis = axis;
    }

    #[getter]
    fn value(&self) -> f32 {
        self.value
    }

    #[setter]
    fn set_value(&mut self, value: f32) -> PyResult<()> {
        self.value = check_for::<Self, _>("value", value, NOT_NAN)?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "RawGamepadAxisChangedEvent(gamepad={}, axis={}, value={})",
            self.gamepad.__repr__(),
            self.axis.__repr__(),
            float_repr(self.value)
        )
    }
}
