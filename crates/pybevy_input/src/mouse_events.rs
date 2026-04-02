use bevy::{
    ecs::entity::Entity,
    input::mouse::{MouseButtonInput, MouseMotion, MouseWheel},
};
use pybevy_core::PyEntity;
pub use pybevy_core::PyMessage;
use pybevy_macros::message_storage;
use pybevy_math::PyVec2;
use pyo3::prelude::*;

use crate::{
    button_state::PyButtonState, mouse_button::PyMouseButton, mouse_scroll_unit::PyMouseScrollUnit,
};

#[message_storage(MouseButtonInput)]
#[pyclass(name = "MouseButtonInput", extends = PyMessage, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMouseButtonInput {
    pub button: PyMouseButton,
    pub state: PyButtonState,
    pub window: PyEntity,
}

impl PyMouseButtonInput {
    pub fn from_bevy(event: &MouseButtonInput) -> (Self, PyMessage) {
        (
            PyMouseButtonInput {
                button: event.button.into(),
                state: event.state.into(),
                window: event.window.into(),
            },
            PyMessage,
        )
    }
}

impl From<&MouseButtonInput> for PyMouseButtonInput {
    fn from(event: &MouseButtonInput) -> Self {
        PyMouseButtonInput {
            button: event.button.into(),
            state: event.state.into(),
            window: event.window.into(),
        }
    }
}

#[pymethods]
impl PyMouseButtonInput {
    #[new]
    #[pyo3(signature = (button, state, window=None))]
    fn new(
        button: PyMouseButton,
        state: PyButtonState,
        window: Option<PyEntity>,
    ) -> (Self, PyMessage) {
        (
            PyMouseButtonInput {
                button,
                state,
                window: window.unwrap_or(Entity::PLACEHOLDER.into()),
            },
            PyMessage,
        )
    }

    #[getter]
    fn button(&self) -> PyMouseButton {
        self.button
    }

    #[getter]
    fn state(&self) -> PyButtonState {
        self.state
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    fn __repr__(&self) -> String {
        format!(
            "MouseButtonInput(button={:?}, state={:?})",
            self.button, self.state
        )
    }
}

#[message_storage(MouseMotion)]
#[pyclass(name = "MouseMotion", extends = PyMessage, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMouseMotion {
    pub delta: PyVec2,
}

impl PyMouseMotion {
    pub fn from_bevy(event: &MouseMotion) -> (Self, PyMessage) {
        (
            PyMouseMotion {
                delta: event.delta.into(),
            },
            PyMessage,
        )
    }
}

impl From<&MouseMotion> for PyMouseMotion {
    fn from(event: &MouseMotion) -> Self {
        PyMouseMotion {
            delta: event.delta.into(),
        }
    }
}

#[pymethods]
impl PyMouseMotion {
    #[new]
    fn new(delta: PyVec2) -> (Self, PyMessage) {
        (PyMouseMotion { delta }, PyMessage)
    }

    #[getter]
    fn delta(&self) -> PyVec2 {
        self.delta.clone()
    }

    fn __repr__(&self) -> String {
        let d = self.delta.get();
        format!("MouseMotion(delta=Vec2({}, {}))", d.x, d.y)
    }
}

#[message_storage(MouseWheel)]
#[pyclass(name = "MouseWheel", extends = PyMessage, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMouseWheel {
    pub x: f32,
    pub y: f32,
    pub unit: PyMouseScrollUnit,
    pub window: PyEntity,
}

impl PyMouseWheel {
    pub fn from_bevy(event: &MouseWheel) -> (Self, PyMessage) {
        (
            PyMouseWheel {
                x: event.x,
                y: event.y,
                unit: event.unit.into(),
                window: event.window.into(),
            },
            PyMessage,
        )
    }
}

impl From<&MouseWheel> for PyMouseWheel {
    fn from(event: &MouseWheel) -> Self {
        PyMouseWheel {
            x: event.x,
            y: event.y,
            unit: event.unit.into(),
            window: event.window.into(),
        }
    }
}

#[pymethods]
impl PyMouseWheel {
    #[new]
    #[pyo3(signature = (x, y, unit = PyMouseScrollUnit::Line, window=None))]
    fn new(x: f32, y: f32, unit: PyMouseScrollUnit, window: Option<PyEntity>) -> (Self, PyMessage) {
        (
            PyMouseWheel {
                x,
                y,
                unit,
                window: window.unwrap_or(Entity::PLACEHOLDER.into()),
            },
            PyMessage,
        )
    }

    #[getter]
    fn x(&self) -> f32 {
        self.x
    }

    #[getter]
    fn y(&self) -> f32 {
        self.y
    }

    #[getter]
    fn unit(&self) -> PyMouseScrollUnit {
        self.unit
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    fn __repr__(&self) -> String {
        format!(
            "MouseWheel(x={}, y={}, unit={:?})",
            self.x, self.y, self.unit
        )
    }
}
