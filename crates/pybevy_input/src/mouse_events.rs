use bevy::{
    ecs::entity::Entity,
    input::mouse::{MouseButtonInput, MouseMotion, MouseWheel},
};
use pybevy_core::PyEntity;
pub use pybevy_core::PyMessage;
use pybevy_macros::pymessage;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::{
    button_state::PyButtonState, mouse_button::PyMouseButton, mouse_scroll_unit::PyMouseScrollUnit,
    touch_phase::PyTouchPhase,
};

#[pymessage(MouseButtonInput, writable)]
#[pyclass(name = "MouseButtonInput", module = "pybevy.input", extends = PyMessage, eq, skip_from_py_object)]
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

impl TryFrom<&PyMouseButtonInput> for MouseButtonInput {
    type Error = PyErr;

    fn try_from(value: &PyMouseButtonInput) -> PyResult<Self> {
        Ok(MouseButtonInput {
            button: value.button.into(),
            state: value.state.into(),
            window: value.window.into(),
        })
    }
}

#[pymethods]
impl PyMouseButtonInput {
    #[new]
    #[pyo3(signature = (*, button, state, window=None))]
    fn new(
        button: PyMouseButton,
        state: PyButtonState,
        window: Option<PyEntity>,
    ) -> PyClassInitializer<Self> {
        (
            PyMouseButtonInput {
                button,
                state,
                window: window.unwrap_or(Entity::PLACEHOLDER.into()),
            },
            PyMessage,
        )
            .into()
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

#[pymessage(MouseMotion, writable)]
#[pyclass(name = "MouseMotion", module = "pybevy.input", extends = PyMessage, eq, skip_from_py_object)]
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

impl TryFrom<&PyMouseMotion> for MouseMotion {
    type Error = PyErr;

    fn try_from(value: &PyMouseMotion) -> PyResult<Self> {
        Ok(MouseMotion {
            delta: value.delta.try_get()?,
        })
    }
}

#[pymethods]
impl PyMouseMotion {
    #[new]
    #[pyo3(signature = (*, delta))]
    fn new(delta: PyVec2) -> PyClassInitializer<Self> {
        (PyMouseMotion { delta }, PyMessage).into()
    }

    #[getter]
    fn delta(&self) -> PyVec2 {
        self.delta.clone()
    }

    fn __repr__(&self) -> PyResult<String> {
        let d = self.delta.try_get()?;
        Ok(format!("MouseMotion(delta=Vec2({}, {}))", d.x, d.y))
    }
}

#[pymessage(MouseWheel, writable)]
#[pyclass(name = "MouseWheel", module = "pybevy.input", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMouseWheel {
    pub x: f32,
    pub y: f32,
    pub unit: PyMouseScrollUnit,
    pub window: PyEntity,
    pub phase: PyTouchPhase,
}

impl PyMouseWheel {
    pub fn from_bevy(event: &MouseWheel) -> (Self, PyMessage) {
        (Self::from(event), PyMessage)
    }
}

impl From<&MouseWheel> for PyMouseWheel {
    fn from(event: &MouseWheel) -> Self {
        PyMouseWheel {
            x: event.x,
            y: event.y,
            unit: event.unit.into(),
            window: event.window.into(),
            phase: event.phase.into(),
        }
    }
}

impl TryFrom<&PyMouseWheel> for MouseWheel {
    type Error = PyErr;

    fn try_from(value: &PyMouseWheel) -> PyResult<Self> {
        Ok(MouseWheel {
            unit: value.unit.into(),
            x: value.x,
            y: value.y,
            window: value.window.into(),
            phase: value.phase.into(),
        })
    }
}

#[pymethods]
impl PyMouseWheel {
    #[new]
    #[pyo3(signature = (*, unit = PyMouseScrollUnit::Line, x, y, window=None, phase = PyTouchPhase::Moved))]
    fn new(
        unit: PyMouseScrollUnit,
        x: f32,
        y: f32,
        window: Option<PyEntity>,
        phase: PyTouchPhase,
    ) -> PyClassInitializer<Self> {
        (
            PyMouseWheel {
                x,
                y,
                unit,
                window: window.unwrap_or(Entity::PLACEHOLDER.into()),
                phase,
            },
            PyMessage,
        )
            .into()
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

    #[getter]
    fn phase(&self) -> PyTouchPhase {
        self.phase
    }

    fn __repr__(&self) -> String {
        format!(
            "MouseWheel(x={}, y={}, unit={:?}, phase={:?})",
            self.x, self.y, self.unit, self.phase
        )
    }
}
