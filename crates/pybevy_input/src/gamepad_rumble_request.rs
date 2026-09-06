use bevy::ecs::entity::Entity;
use pybevy_core::{PyEntity, PyMessage};
use pyo3::prelude::*;

#[pyclass(name = "GamepadRumbleRequest", module = "pybevy.input", extends = PyMessage, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyGamepadRumbleRequest {
    pub duration_secs: f32,
    pub strong_motor: f32,
    pub weak_motor: f32,
    pub gamepad_entity: PyEntity,
}

#[pymethods]
impl PyGamepadRumbleRequest {
    #[new]
    #[pyo3(signature = (duration_secs, strong_motor=1.0, weak_motor=1.0, gamepad=PyEntity::from(Entity::PLACEHOLDER)))]
    fn new(
        duration_secs: f32,
        strong_motor: f32,
        weak_motor: f32,
        gamepad: PyEntity,
    ) -> PyClassInitializer<Self> {
        (
            PyGamepadRumbleRequest {
                duration_secs,
                strong_motor,
                weak_motor,
                gamepad_entity: gamepad,
            },
            PyMessage,
        )
            .into()
    }

    #[getter]
    fn duration_secs(&self) -> f32 {
        self.duration_secs
    }

    #[getter]
    fn strong_motor(&self) -> f32 {
        self.strong_motor
    }

    #[getter]
    fn weak_motor(&self) -> f32 {
        self.weak_motor
    }

    fn gamepad(&self) -> PyEntity {
        self.gamepad_entity
    }

    fn __repr__(&self) -> String {
        format!(
            "GamepadRumbleRequest(duration_secs={}, strong_motor={}, weak_motor={})",
            self.duration_secs, self.strong_motor, self.weak_motor
        )
    }
}
