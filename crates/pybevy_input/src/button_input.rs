use bevy::input::{ButtonInput, keyboard::KeyCode};
use pybevy_core::{PyResource, ResourceStorage};
use pyo3::prelude::*;

use crate::key_code::PyKeyCode;

#[pyclass(name = "ButtonInput", extends = PyResource, frozen)]
#[derive(Clone)]
pub struct PyButtonInput {
    storage: Option<ResourceStorage<ButtonInput<KeyCode>>>,
}

impl PyButtonInput {
    pub fn from_borrowed(storage: ResourceStorage<ButtonInput<KeyCode>>) -> (Self, PyResource) {
        (
            PyButtonInput {
                storage: Some(storage),
            },
            PyResource,
        )
    }

    fn get_input(&self) -> PyResult<&ButtonInput<KeyCode>> {
        match &self.storage {
            Some(storage) => Ok(storage.as_ref()?),
            None => {
                // Return empty ButtonInput for headless environments
                // This is safe because we use a static lazy-initialized instance
                static EMPTY_INPUT: std::sync::OnceLock<ButtonInput<KeyCode>> =
                    std::sync::OnceLock::new();
                Ok(EMPTY_INPUT.get_or_init(ButtonInput::default))
            }
        }
    }
}

#[pymethods]
impl PyButtonInput {
    #[new]
    pub fn new() -> (Self, PyResource) {
        (PyButtonInput { storage: None }, PyResource)
    }

    pub fn just_pressed(&self, input: PyKeyCode) -> PyResult<bool> {
        let button_input = self.get_input()?;
        Ok(button_input.just_pressed(input.to_bevy()))
    }

    pub fn just_released(&self, input: PyKeyCode) -> PyResult<bool> {
        let button_input = self.get_input()?;
        Ok(button_input.just_released(input.to_bevy()))
    }

    pub fn pressed(&self, input: PyKeyCode) -> PyResult<bool> {
        let button_input = self.get_input()?;
        Ok(button_input.pressed(input.to_bevy()))
    }

    pub fn any_just_pressed(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let button_input = self.get_input()?;
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(button_input.any_just_pressed(bevy_keys))
    }

    pub fn any_pressed(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let button_input = self.get_input()?;
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(button_input.any_pressed(bevy_keys))
    }

    pub fn all_pressed(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let button_input = self.get_input()?;
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(bevy_keys.iter().all(|k| button_input.pressed(*k)))
    }

    pub fn get_just_pressed(&self) -> PyResult<Vec<PyKeyCode>> {
        let input = self.get_input()?;
        let pressed: Vec<PyKeyCode> = input
            .get_just_pressed()
            .filter_map(|k| PyKeyCode::from_bevy(*k))
            .collect();
        Ok(pressed)
    }

    pub fn get_pressed(&self) -> PyResult<Vec<PyKeyCode>> {
        let input = self.get_input()?;
        let pressed: Vec<PyKeyCode> = input
            .get_pressed()
            .filter_map(|k| PyKeyCode::from_bevy(*k))
            .collect();
        Ok(pressed)
    }

    pub fn get_just_released(&self) -> PyResult<Vec<PyKeyCode>> {
        let input = self.get_input()?;
        let released: Vec<PyKeyCode> = input
            .get_just_released()
            .filter_map(|k| PyKeyCode::from_bevy(*k))
            .collect();
        Ok(released)
    }

    pub fn any_just_released(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let button_input = self.get_input()?;
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(button_input.any_just_released(bevy_keys))
    }

    pub fn all_just_pressed(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let button_input = self.get_input()?;
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(button_input.all_just_pressed(bevy_keys))
    }

    pub fn all_just_released(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let button_input = self.get_input()?;
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(button_input.all_just_released(bevy_keys))
    }

    fn __repr__(&self) -> String {
        if self.storage.is_some() {
            "ButtonInput(initialized)".to_string()
        } else {
            "ButtonInput(uninitialized)".to_string()
        }
    }
}
