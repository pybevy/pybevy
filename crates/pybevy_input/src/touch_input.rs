use bevy::{ecs::entity::Entity, input::touch::TouchInput};
use pybevy_core::PyEntity;
pub use pybevy_core::PyMessage;
use pybevy_macros::pymessage;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::touch_phase::PyTouchPhase;

#[pymessage(TouchInput)]
#[pyclass(name = "TouchInput", extends = PyMessage, frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyTouchInput {
    #[pyo3(get)]
    pub phase: PyTouchPhase,

    #[pyo3(get)]
    pub position: PyVec2,

    #[pyo3(get)]
    pub id: u64,

    #[pyo3(get)]
    pub force: Option<f64>,

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
        let force = event.force.map(|f| match f {
            bevy::input::touch::ForceTouch::Calibrated {
                force,
                max_possible_force,
                altitude_angle: _,
            } => force / max_possible_force,
            bevy::input::touch::ForceTouch::Normalized(normalized) => normalized,
        });

        PyTouchInput {
            phase: event.phase.into(),
            position: event.position.into(),
            id: event.id,
            force,
            window: event.window.into(),
        }
    }
}

#[pymethods]
impl PyTouchInput {
    #[new]
    #[pyo3(signature = (phase, position, id, force = None, window = PyEntity::from(Entity::PLACEHOLDER)))]
    pub fn new(
        phase: PyTouchPhase,
        position: PyVec2,
        id: u64,
        force: Option<f64>,
        window: PyEntity,
    ) -> PyClassInitializer<Self> {
        (
            PyTouchInput {
                phase,
                position,
                id,
                force,
                window,
            },
            PyMessage,
        ).into()
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "TouchInput(phase={}, position=Vec2(x={:.2}, y={:.2}), id={}, force={:?})",
            self.phase.__repr__(),
            self.position.x()?,
            self.position.y()?,
            self.id,
            self.force
        ))
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.phase == other.phase
            && self.position.x()? == other.position.x()?
            && self.position.y()? == other.position.y()?
            && self.id == other.id
            && self.force == other.force
            && self.window == other.window)
    }
}
