use bevy::{
    ecs::entity::Entity,
    input::keyboard::{Key, KeyboardInput},
};
use pybevy_core::{PyEntity, PyMessage};
use pybevy_macros::pymessage;
use pyo3::prelude::*;

use crate::{
    button_state::PyButtonState,
    key::PyKey,
    key_code::{PyKeyCode, materialize_key_code},
};

#[pymessage(KeyboardInput, writable, materialize = materialize_keyboard_input)]
#[pyclass(name = "KeyboardInput", module = "pybevy.input", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyKeyboardInput {
    pub key_code: bevy::input::keyboard::KeyCode,
    pub logical_key: PyKey,
    pub state: PyButtonState,
    pub text: Option<String>,
    pub repeat: bool,
    pub window: PyEntity,
}

impl PyKeyboardInput {
    pub fn from_bevy_event(event: &KeyboardInput) -> PyResult<Self> {
        Ok(Self {
            key_code: event.key_code,
            logical_key: PyKey::try_from(&event.logical_key)?,
            state: event.state.into(),
            text: event.text.as_ref().map(ToString::to_string),
            repeat: event.repeat,
            window: event.window.into(),
        })
    }
}

impl TryFrom<&PyKeyboardInput> for KeyboardInput {
    type Error = PyErr;

    fn try_from(value: &PyKeyboardInput) -> PyResult<Self> {
        Ok(KeyboardInput {
            key_code: value.key_code,
            logical_key: Key::from(value.logical_key.clone()),
            state: value.state.into(),
            text: value.text.clone().map(Into::into),
            repeat: value.repeat,
            window: value.window.into(),
        })
    }
}

pub fn materialize_keyboard_input(py: Python<'_>, event: &KeyboardInput) -> PyResult<Py<PyAny>> {
    let py_event = PyKeyboardInput::from_bevy_event(event)?;
    Ok(Py::new(py, (py_event, PyMessage))?.into_any())
}

#[pymethods]
impl PyKeyboardInput {
    #[new]
    #[pyo3(signature = (*, key_code, logical_key, state, text=None, repeat=false, window=None))]
    fn new(
        key_code: &PyKeyCode,
        logical_key: PyKey,
        state: PyButtonState,
        text: Option<String>,
        repeat: bool,
        window: Option<PyEntity>,
    ) -> PyClassInitializer<Self> {
        (
            PyKeyboardInput {
                key_code: key_code.to_bevy(),
                logical_key,
                state,
                text,
                repeat,
                window: window.unwrap_or(Entity::PLACEHOLDER.into()),
            },
            PyMessage,
        )
            .into()
    }

    #[getter]
    fn key_code(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        materialize_key_code(py, self.key_code)
    }

    #[setter]
    fn set_key_code(&mut self, key_code: &PyKeyCode) {
        self.key_code = key_code.to_bevy();
    }

    #[getter]
    fn logical_key(&self) -> PyKey {
        self.logical_key.clone()
    }

    #[setter]
    fn set_logical_key(&mut self, logical_key: PyKey) {
        self.logical_key = logical_key;
    }

    #[getter]
    fn state(&self) -> PyButtonState {
        self.state
    }

    #[setter]
    fn set_state(&mut self, state: PyButtonState) {
        self.state = state;
    }

    #[getter]
    fn text(&self) -> Option<String> {
        self.text.clone()
    }

    #[setter]
    fn set_text(&mut self, text: Option<String>) {
        self.text = text;
    }

    #[getter]
    fn repeat(&self) -> bool {
        self.repeat
    }

    #[setter]
    fn set_repeat(&mut self, repeat: bool) {
        self.repeat = repeat;
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    #[setter]
    fn set_window(&mut self, window: PyEntity) {
        self.window = window;
    }

    fn __repr__(&self) -> String {
        format!(
            "KeyboardInput(key_code={:?}, state={:?})",
            self.key_code, self.state
        )
    }
}
