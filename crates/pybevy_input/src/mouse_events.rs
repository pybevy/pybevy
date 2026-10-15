use bevy::{
    ecs::entity::Entity,
    input::mouse::{MouseButtonInput, MouseMotion, MouseWheel},
    math::Vec2,
};
pub use pybevy_core::PyMessage;
use pybevy_core::{
    PyEntity,
    repr::{ReprLiteral, float_repr},
};
use pybevy_macros::pymessage;
use pybevy_math::snapshot::Snapshot;
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
    #[pyo3(signature = (*, button, state, window=PyEntity::from(Entity::PLACEHOLDER)))]
    fn new(
        button: PyMouseButton,
        state: PyButtonState,
        window: PyEntity,
    ) -> PyClassInitializer<Self> {
        (
            PyMouseButtonInput {
                button,
                state,
                window,
            },
            PyMessage,
        )
            .into()
    }

    #[getter]
    fn button(&self) -> PyMouseButton {
        self.button
    }

    #[setter]
    fn set_button(&mut self, button: PyMouseButton) {
        self.button = button;
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
    fn window(&self) -> PyEntity {
        self.window
    }

    #[setter]
    fn set_window(&mut self, window: PyEntity) {
        self.window = window;
    }

    fn __repr__(&self) -> String {
        format!(
            "MouseButtonInput(button={}, state={}, window={})",
            self.button.__repr__(),
            self.state.__repr__(),
            self.window.__repr__()
        )
    }
}

#[pymessage(MouseMotion, writable)]
#[pyclass(name = "MouseMotion", module = "pybevy.input", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMouseMotion {
    pub delta: Vec2,
}

impl PyMouseMotion {
    pub fn from_bevy(event: &MouseMotion) -> (Self, PyMessage) {
        (PyMouseMotion::from(event), PyMessage)
    }
}

impl From<&MouseMotion> for PyMouseMotion {
    fn from(event: &MouseMotion) -> Self {
        PyMouseMotion { delta: event.delta }
    }
}

impl TryFrom<&PyMouseMotion> for MouseMotion {
    type Error = PyErr;

    fn try_from(value: &PyMouseMotion) -> PyResult<Self> {
        Ok(MouseMotion { delta: value.delta })
    }
}

#[pymethods]
impl PyMouseMotion {
    #[new]
    #[pyo3(signature = (*, delta))]
    fn new(delta: Snapshot<Vec2>) -> PyClassInitializer<Self> {
        (PyMouseMotion { delta: delta.0 }, PyMessage).into()
    }

    #[getter]
    fn delta(&self) -> Snapshot<Vec2> {
        Snapshot(self.delta)
    }

    #[setter]
    fn set_delta(&mut self, delta: Snapshot<Vec2>) {
        self.delta = delta.0;
    }

    fn __repr__(&self) -> String {
        format!("MouseMotion(delta={})", self.delta.repr_literal())
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
    #[pyo3(signature = (*, unit = PyMouseScrollUnit::Line, x, y, window=PyEntity::from(Entity::PLACEHOLDER), phase = PyTouchPhase::Moved))]
    fn new(
        unit: PyMouseScrollUnit,
        x: f32,
        y: f32,
        window: PyEntity,
        phase: PyTouchPhase,
    ) -> PyClassInitializer<Self> {
        (
            PyMouseWheel {
                x,
                y,
                unit,
                window,
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

    #[setter]
    fn set_x(&mut self, x: f32) {
        self.x = x;
    }

    #[getter]
    fn y(&self) -> f32 {
        self.y
    }

    #[setter]
    fn set_y(&mut self, y: f32) {
        self.y = y;
    }

    #[getter]
    fn unit(&self) -> PyMouseScrollUnit {
        self.unit
    }

    #[setter]
    fn set_unit(&mut self, unit: PyMouseScrollUnit) {
        self.unit = unit;
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    #[setter]
    fn set_window(&mut self, window: PyEntity) {
        self.window = window;
    }

    #[getter]
    fn phase(&self) -> PyTouchPhase {
        self.phase
    }

    #[setter]
    fn set_phase(&mut self, phase: PyTouchPhase) {
        self.phase = phase;
    }

    fn __repr__(&self) -> String {
        format!(
            "MouseWheel(x={}, y={}, unit={}, window={}, phase={})",
            float_repr(self.x),
            float_repr(self.y),
            self.unit.__repr__(),
            self.window.__repr__(),
            self.phase.__repr__()
        )
    }
}
