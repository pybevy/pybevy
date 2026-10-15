use bevy::{ecs::entity::Entity, input::touch::TouchInput, math::Vec2};
use pybevy_core::PyEntity;
pub use pybevy_core::PyMessage;
use pybevy_macros::pymessage;
use pybevy_math::snapshot::Snapshot;
use pyo3::prelude::*;

use crate::{force_touch::PyForceTouch, touch_phase::PyTouchPhase};

#[pymessage(TouchInput, writable)]
#[pyclass(name = "TouchInput", module = "pybevy.input", extends = PyMessage, frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyTouchInput {
    #[pyo3(get)]
    pub phase: PyTouchPhase,

    pub position: Vec2,

    #[pyo3(get)]
    pub id: u64,

    #[pyo3(get)]
    pub force: Option<PyForceTouch>,

    #[pyo3(get)]
    pub window: PyEntity,
}

impl PyTouchInput {
    pub fn from_bevy(event: &TouchInput) -> (Self, PyMessage) {
        (Self::from(event), PyMessage)
    }
}

impl From<&TouchInput> for PyTouchInput {
    fn from(event: &TouchInput) -> Self {
        PyTouchInput {
            phase: event.phase.into(),
            position: event.position,
            id: event.id,
            force: event.force.map(Into::into),
            window: event.window.into(),
        }
    }
}

impl TryFrom<&PyTouchInput> for TouchInput {
    type Error = PyErr;

    fn try_from(value: &PyTouchInput) -> PyResult<Self> {
        Ok(Self {
            phase: value.phase.into(),
            position: value.position,
            window: value.window.into(),
            force: value.force.map(Into::into),
            id: value.id,
        })
    }
}

#[pymethods]
impl PyTouchInput {
    #[new]
    #[pyo3(signature = (*, phase, position, window = PyEntity::from(Entity::PLACEHOLDER), force = None, id))]
    pub fn new(
        phase: PyTouchPhase,
        position: Snapshot<Vec2>,
        window: PyEntity,
        force: Option<PyForceTouch>,
        id: u64,
    ) -> PyClassInitializer<Self> {
        (
            PyTouchInput {
                phase,
                position: position.0,
                id,
                force,
                window,
            },
            PyMessage,
        )
            .into()
    }

    #[getter]
    fn position(&self) -> Snapshot<Vec2> {
        Snapshot(self.position)
    }

    pub fn __repr__(&self) -> String {
        format!(
            "TouchInput(phase={}, position=Vec2(x={:.2}, y={:.2}), id={}, force={})",
            self.phase.__repr__(),
            self.position.x,
            self.position.y,
            self.id,
            self.force
                .map_or_else(|| "None".to_string(), |force| force.__repr__())
        )
    }

    pub fn __eq__(&self, other: &Self) -> bool {
        self.phase == other.phase
            && self.position == other.position
            && self.id == other.id
            && self.force == other.force
            && self.window == other.window
    }
}
