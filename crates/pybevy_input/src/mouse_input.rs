use bevy::input::{ButtonInput, mouse::MouseButton};
use pybevy_core::{PyResource, ResourceStorage};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

use crate::mouse_button::PyMouseButton;

#[pyresource(ButtonInput<MouseButton>, no_clone, bridge, "MouseInput", no_mut, default_insert)]
#[pyclass(name = "MouseInput", extends = PyResource, frozen)]
pub struct PyMouseInput {
    pub(crate) storage: ResourceStorage<ButtonInput<MouseButton>>,
}

#[pymethods]
impl PyMouseInput {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (
            Self {
                storage: ResourceStorage::owned(ButtonInput::default()),
            },
            PyResource,
        ).into()
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

    fn __repr__(&self) -> String {
        "MouseInput(...)".to_string()
    }
}
