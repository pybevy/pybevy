use bevy::input::{ButtonInput, keyboard::KeyCode};
use pybevy_core::{PyResource, ResourceStorage};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

use crate::key_code::PyKeyCode;

#[pyresource(ButtonInput<KeyCode>, no_clone, bridge, "ButtonInput", no_mut, default_insert)]
#[pyclass(name = "ButtonInput", extends = PyResource, frozen)]
pub struct PyButtonInput {
    pub(crate) storage: ResourceStorage<ButtonInput<KeyCode>>,
}

#[pymethods]
impl PyButtonInput {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (
            Self {
                storage: ResourceStorage::owned(ButtonInput::default()),
            },
            PyResource,
        ).into()
    }

    pub fn just_pressed(&self, input: PyKeyCode) -> PyResult<bool> {
        Ok(self.as_ref()?.just_pressed(input.to_bevy()))
    }

    pub fn just_released(&self, input: PyKeyCode) -> PyResult<bool> {
        Ok(self.as_ref()?.just_released(input.to_bevy()))
    }

    pub fn pressed(&self, input: PyKeyCode) -> PyResult<bool> {
        Ok(self.as_ref()?.pressed(input.to_bevy()))
    }

    pub fn any_just_pressed(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(self.as_ref()?.any_just_pressed(bevy_keys))
    }

    pub fn any_pressed(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(self.as_ref()?.any_pressed(bevy_keys))
    }

    pub fn all_pressed(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let button_input = self.as_ref()?;
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(bevy_keys.iter().all(|k| button_input.pressed(*k)))
    }

    pub fn get_just_pressed(&self) -> PyResult<Vec<PyKeyCode>> {
        let input = self.as_ref()?;
        let pressed: Vec<PyKeyCode> = input
            .get_just_pressed()
            .filter_map(|k| PyKeyCode::from_bevy(*k))
            .collect();
        Ok(pressed)
    }

    pub fn get_pressed(&self) -> PyResult<Vec<PyKeyCode>> {
        let input = self.as_ref()?;
        let pressed: Vec<PyKeyCode> = input
            .get_pressed()
            .filter_map(|k| PyKeyCode::from_bevy(*k))
            .collect();
        Ok(pressed)
    }

    pub fn get_just_released(&self) -> PyResult<Vec<PyKeyCode>> {
        let input = self.as_ref()?;
        let released: Vec<PyKeyCode> = input
            .get_just_released()
            .filter_map(|k| PyKeyCode::from_bevy(*k))
            .collect();
        Ok(released)
    }

    pub fn any_just_released(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(self.as_ref()?.any_just_released(bevy_keys))
    }

    pub fn all_just_pressed(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(self.as_ref()?.all_just_pressed(bevy_keys))
    }

    pub fn all_just_released(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(self.as_ref()?.all_just_released(bevy_keys))
    }

    fn __repr__(&self) -> String {
        "ButtonInput(...)".to_string()
    }
}
