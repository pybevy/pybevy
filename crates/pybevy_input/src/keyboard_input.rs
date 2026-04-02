use bevy::ecs::entity::Entity;
use pybevy_core::{PyEntity, PyMessage};
use pyo3::prelude::*;

use crate::{button_state::PyButtonState, key_code::PyKeyCode};

#[pyclass(name = "KeyboardInput", extends = PyMessage, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyKeyboardInput {
    pub key_code: PyKeyCode,
    pub state: PyButtonState,
    pub logical_key: Option<String>,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub super_key: bool,
    pub repeat: bool,
    pub text: Option<String>,
    pub window: PyEntity,
}

#[pymethods]
impl PyKeyboardInput {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (key_code, state, *, shift=false, ctrl=false, alt=false, super_key=false, repeat=false, logical_key=None, text=None, window=None))]
    fn new(
        key_code: PyKeyCode,
        state: PyButtonState,
        shift: bool,
        ctrl: bool,
        alt: bool,
        super_key: bool,
        repeat: bool,
        logical_key: Option<String>,
        text: Option<String>,
        window: Option<PyEntity>,
    ) -> (Self, PyMessage) {
        (
            PyKeyboardInput {
                key_code,
                state,
                logical_key,
                shift,
                ctrl,
                alt,
                super_key,
                repeat,
                text,
                window: window.unwrap_or(Entity::PLACEHOLDER.into()),
            },
            PyMessage,
        )
    }

    #[getter]
    fn key_code(&self) -> PyKeyCode {
        self.key_code
    }

    #[getter]
    fn state(&self) -> PyButtonState {
        self.state
    }

    #[getter]
    fn logical_key(&self) -> Option<String> {
        self.logical_key.clone()
    }

    #[getter]
    fn shift(&self) -> bool {
        self.shift
    }

    #[getter]
    fn ctrl(&self) -> bool {
        self.ctrl
    }

    #[getter]
    fn alt(&self) -> bool {
        self.alt
    }

    #[getter]
    fn super_key(&self) -> bool {
        self.super_key
    }

    #[getter]
    fn repeat(&self) -> bool {
        self.repeat
    }

    #[getter]
    fn text(&self) -> Option<String> {
        self.text.clone()
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
