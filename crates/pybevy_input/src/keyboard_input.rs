use bevy::{ecs::entity::Entity, input::keyboard::KeyboardInput};
use pybevy_core::{PyEntity, PyMessage};
use pyo3::prelude::*;

use crate::{
    button_state::PyButtonState,
    key::PyKey,
    key_code::{PyKeyCode, materialize_key_code},
};

#[pyclass(name = "KeyboardInput", extends = PyMessage, eq, skip_from_py_object)]
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

#[pymethods]
impl PyKeyboardInput {
    #[new]
    #[pyo3(signature = (key_code, logical_key, state, text=None, repeat=false, window=None))]
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

    #[getter]
    fn logical_key(&self) -> PyKey {
        self.logical_key.clone()
    }

    #[getter]
    fn state(&self) -> PyButtonState {
        self.state
    }

    #[getter]
    fn text(&self) -> Option<String> {
        self.text.clone()
    }

    #[getter]
    fn repeat(&self) -> bool {
        self.repeat
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    fn __repr__(&self) -> String {
        format!(
            "KeyboardInput(key_code={:?}, state={:?})",
            self.key_code, self.state
        )
    }
}
