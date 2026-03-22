use bevy::input::{ButtonInput, mouse::MouseButton};
use pybevy_core::{PyResource, ResourceStorage};
use pyo3::prelude::*;

use crate::mouse_button::PyMouseButton;

#[pyclass(name = "MouseInput", extends = PyResource, frozen)]
#[derive(Clone)]
pub struct PyMouseInput {
    storage: Option<ResourceStorage<ButtonInput<MouseButton>>>,
}

impl PyMouseInput {
    pub fn from_borrowed(storage: ResourceStorage<ButtonInput<MouseButton>>) -> (Self, PyResource) {
        (
            PyMouseInput {
                storage: Some(storage),
            },
            PyResource,
        )
    }

    fn get_input(&self) -> PyResult<&ButtonInput<MouseButton>> {
        match &self.storage {
            Some(storage) => Ok(storage.as_ref()?),
            None => {
                static EMPTY_INPUT: std::sync::OnceLock<ButtonInput<MouseButton>> =
                    std::sync::OnceLock::new();
                Ok(EMPTY_INPUT.get_or_init(ButtonInput::default))
            }
        }
    }
}

#[pymethods]
impl PyMouseInput {
    #[new]
    pub fn new() -> (Self, PyResource) {
        (PyMouseInput { storage: None }, PyResource)
    }

    pub fn just_pressed(&self, button: PyMouseButton) -> PyResult<bool> {
        let input = self.get_input()?;
        Ok(input.just_pressed(button.into()))
    }

    pub fn just_released(&self, button: PyMouseButton) -> PyResult<bool> {
        let input = self.get_input()?;
        Ok(input.just_released(button.into()))
    }

    pub fn pressed(&self, button: PyMouseButton) -> PyResult<bool> {
        let input = self.get_input()?;
        Ok(input.pressed(button.into()))
    }

    pub fn any_just_pressed(&self, buttons: Vec<PyMouseButton>) -> PyResult<bool> {
        let input = self.get_input()?;
        let bevy_buttons: Vec<MouseButton> = buttons.into_iter().map(|b| b.into()).collect();
        Ok(input.any_just_pressed(bevy_buttons))
    }

    pub fn any_pressed(&self, buttons: Vec<PyMouseButton>) -> PyResult<bool> {
        let input = self.get_input()?;
        let bevy_buttons: Vec<MouseButton> = buttons.into_iter().map(|b| b.into()).collect();
        Ok(input.any_pressed(bevy_buttons))
    }

    pub fn all_pressed(&self, buttons: Vec<PyMouseButton>) -> PyResult<bool> {
        let input = self.get_input()?;
        let bevy_buttons: Vec<MouseButton> = buttons.into_iter().map(|b| b.into()).collect();
        Ok(bevy_buttons.iter().all(|b| input.pressed(*b)))
    }

    pub fn get_just_pressed(&self) -> PyResult<Vec<PyMouseButton>> {
        let input = self.get_input()?;
        let pressed: Vec<PyMouseButton> = input.get_just_pressed().map(|b| (*b).into()).collect();
        Ok(pressed)
    }

    pub fn get_pressed(&self) -> PyResult<Vec<PyMouseButton>> {
        let input = self.get_input()?;
        let pressed: Vec<PyMouseButton> = input.get_pressed().map(|b| (*b).into()).collect();
        Ok(pressed)
    }

    pub fn get_just_released(&self) -> PyResult<Vec<PyMouseButton>> {
        let input = self.get_input()?;
        let released: Vec<PyMouseButton> = input.get_just_released().map(|b| (*b).into()).collect();
        Ok(released)
    }

    fn __repr__(&self) -> String {
        if self.storage.is_some() {
            "MouseInput(initialized)".to_string()
        } else {
            "MouseInput(uninitialized)".to_string()
        }
    }
}
