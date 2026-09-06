use bevy::input::{ButtonInput, mouse::MouseButton};
use pybevy_core::{PyResource, ResourceStorage, resource_initializer};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

use crate::mouse_button::PyMouseButton;

#[pyresource(ButtonInput<MouseButton>, no_clone, bridge, "MouseInput", default_insert)]
#[pyclass(name = "MouseInput", module = "pybevy.input", extends = PyResource)]
pub struct PyMouseInput {
    pub(crate) storage: ResourceStorage<ButtonInput<MouseButton>>,
}

#[pymethods]
impl PyMouseInput {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        resource_initializer(Self {
            storage: ResourceStorage::owned(ButtonInput::default()),
        })
    }

    pub fn just_pressed(&self, button: PyMouseButton) -> PyResult<bool> {
        Ok(self.as_ref()?.just_pressed(button.into()))
    }

    pub fn just_released(&self, button: PyMouseButton) -> PyResult<bool> {
        Ok(self.as_ref()?.just_released(button.into()))
    }

    pub fn pressed(&self, button: PyMouseButton) -> PyResult<bool> {
        Ok(self.as_ref()?.pressed(button.into()))
    }

    pub fn any_just_pressed(&self, buttons: Vec<PyMouseButton>) -> PyResult<bool> {
        let bevy_buttons: Vec<MouseButton> = buttons.into_iter().map(|b| b.into()).collect();
        Ok(self.as_ref()?.any_just_pressed(bevy_buttons))
    }

    pub fn any_pressed(&self, buttons: Vec<PyMouseButton>) -> PyResult<bool> {
        let bevy_buttons: Vec<MouseButton> = buttons.into_iter().map(|b| b.into()).collect();
        Ok(self.as_ref()?.any_pressed(bevy_buttons))
    }

    pub fn all_pressed(&self, buttons: Vec<PyMouseButton>) -> PyResult<bool> {
        let input = self.as_ref()?;
        let bevy_buttons: Vec<MouseButton> = buttons.into_iter().map(|b| b.into()).collect();
        Ok(bevy_buttons.iter().all(|b| input.pressed(*b)))
    }

    pub fn get_just_pressed(&self) -> PyResult<Vec<PyMouseButton>> {
        let input = self.as_ref()?;
        let pressed: Vec<PyMouseButton> = input.get_just_pressed().map(|b| (*b).into()).collect();
        Ok(pressed)
    }

    pub fn get_pressed(&self) -> PyResult<Vec<PyMouseButton>> {
        let input = self.as_ref()?;
        let pressed: Vec<PyMouseButton> = input.get_pressed().map(|b| (*b).into()).collect();
        Ok(pressed)
    }

    pub fn get_just_released(&self) -> PyResult<Vec<PyMouseButton>> {
        let input = self.as_ref()?;
        let released: Vec<PyMouseButton> = input.get_just_released().map(|b| (*b).into()).collect();
        Ok(released)
    }

    pub fn any_just_released(&self, buttons: Vec<PyMouseButton>) -> PyResult<bool> {
        let bevy_buttons: Vec<MouseButton> = buttons.into_iter().map(|b| b.into()).collect();
        Ok(self.as_ref()?.any_just_released(bevy_buttons))
    }

    pub fn all_just_pressed(&self, buttons: Vec<PyMouseButton>) -> PyResult<bool> {
        let bevy_buttons: Vec<MouseButton> = buttons.into_iter().map(|b| b.into()).collect();
        Ok(self.as_ref()?.all_just_pressed(bevy_buttons))
    }

    pub fn all_just_released(&self, buttons: Vec<PyMouseButton>) -> PyResult<bool> {
        let bevy_buttons: Vec<MouseButton> = buttons.into_iter().map(|b| b.into()).collect();
        Ok(self.as_ref()?.all_just_released(bevy_buttons))
    }

    pub fn press(&mut self, button: PyMouseButton) -> PyResult<()> {
        self.as_mut()?.press(button.into());
        Ok(())
    }

    pub fn release(&mut self, button: PyMouseButton) -> PyResult<()> {
        self.as_mut()?.release(button.into());
        Ok(())
    }

    pub fn release_all(&mut self) -> PyResult<()> {
        self.as_mut()?.release_all();
        Ok(())
    }

    pub fn clear_just_pressed(&mut self, button: PyMouseButton) -> PyResult<bool> {
        Ok(self.as_mut()?.clear_just_pressed(button.into()))
    }

    pub fn clear_just_released(&mut self, button: PyMouseButton) -> PyResult<bool> {
        Ok(self.as_mut()?.clear_just_released(button.into()))
    }

    pub fn reset(&mut self, button: PyMouseButton) -> PyResult<()> {
        self.as_mut()?.reset(button.into());
        Ok(())
    }

    pub fn reset_all(&mut self) -> PyResult<()> {
        self.as_mut()?.reset_all();
        Ok(())
    }

    pub fn clear(&mut self) -> PyResult<()> {
        self.as_mut()?.clear();
        Ok(())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(input) => {
                let pressed: Vec<String> = input
                    .get_pressed()
                    .map(|button| format!("{:?}", button))
                    .collect();
                format!("MouseInput(pressed=[{}])", pressed.join(", "))
            }
            Err(_) => "MouseInput(<invalid>)".to_string(),
        }
    }
}
