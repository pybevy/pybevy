use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton, GamepadInput};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::{
    gamepad_axis::PyGamepadAxis, gamepad_button::PyGamepadButton, gamepad_input::PyGamepadInput,
};

#[pycomponent(Gamepad, no_clone, bridge)]
#[pyclass(name = "Gamepad", module = "pybevy.input", extends = PyComponent, frozen)]
pub struct PyGamepad {
    pub(crate) storage: ComponentStorage<Gamepad>,
}

#[pymethods]
impl PyGamepad {
    pub fn just_pressed(&self, button_type: PyGamepadButton) -> PyResult<bool> {
        let gamepad = self.as_ref()?;
        Ok(gamepad.just_pressed(button_type.into()))
    }

    pub fn just_released(&self, button_type: PyGamepadButton) -> PyResult<bool> {
        let gamepad = self.as_ref()?;
        Ok(gamepad.just_released(button_type.into()))
    }

    pub fn pressed(&self, button_type: PyGamepadButton) -> PyResult<bool> {
        let gamepad = self.as_ref()?;
        Ok(gamepad.pressed(button_type.into()))
    }

    pub fn get_button(&self, button: PyGamepadButton) -> PyResult<Option<f32>> {
        let gamepad = self.as_ref()?;
        let bevy_button: GamepadButton = button.into();
        Ok(gamepad.get(bevy_button))
    }

    pub fn get_axis(&self, axis: PyGamepadAxis) -> PyResult<Option<f32>> {
        let gamepad = self.as_ref()?;
        let bevy_axis: GamepadAxis = axis.into();
        Ok(gamepad.get(bevy_axis))
    }

    pub fn get_button_unclamped(&self, button: PyGamepadButton) -> PyResult<Option<f32>> {
        let gamepad = self.as_ref()?;
        let bevy_button: GamepadButton = button.into();
        Ok(gamepad.get_unclamped(bevy_button))
    }

    pub fn get_axis_unclamped(&self, axis: PyGamepadAxis) -> PyResult<Option<f32>> {
        let gamepad = self.as_ref()?;
        let bevy_axis: GamepadAxis = axis.into();
        Ok(gamepad.get_unclamped(bevy_axis))
    }

    pub fn get(&self, input: PyGamepadInput) -> PyResult<Option<f32>> {
        let gamepad = self.as_ref()?;
        let bevy_input: GamepadInput = input.into();
        Ok(gamepad.get(bevy_input))
    }

    pub fn get_unclamped(&self, input: PyGamepadInput) -> PyResult<Option<f32>> {
        let gamepad = self.as_ref()?;
        let bevy_input: GamepadInput = input.into();
        Ok(gamepad.get_unclamped(bevy_input))
    }

    pub fn get_analog_axes(&self) -> PyResult<Vec<PyGamepadInput>> {
        let gamepad = self.as_ref()?;
        let axes: Vec<PyGamepadInput> = gamepad.get_analog_axes().map(|a| (*a).into()).collect();
        Ok(axes)
    }

    pub fn get_pressed(&self) -> PyResult<Vec<PyGamepadButton>> {
        let gamepad = self.as_ref()?;
        let pressed: Vec<PyGamepadButton> = gamepad.get_pressed().map(|b| (*b).into()).collect();
        Ok(pressed)
    }

    pub fn get_just_pressed(&self) -> PyResult<Vec<PyGamepadButton>> {
        let gamepad = self.as_ref()?;
        let pressed: Vec<PyGamepadButton> =
            gamepad.get_just_pressed().map(|b| (*b).into()).collect();
        Ok(pressed)
    }

    pub fn get_just_released(&self) -> PyResult<Vec<PyGamepadButton>> {
        let gamepad = self.as_ref()?;
        let released: Vec<PyGamepadButton> =
            gamepad.get_just_released().map(|b| (*b).into()).collect();
        Ok(released)
    }

    pub fn left_stick(&self) -> PyResult<PyVec2> {
        let gamepad = self.as_ref()?;
        Ok(gamepad.left_stick().into())
    }

    pub fn right_stick(&self) -> PyResult<PyVec2> {
        let gamepad = self.as_ref()?;
        Ok(gamepad.right_stick().into())
    }

    pub fn dpad(&self) -> PyResult<PyVec2> {
        let gamepad = self.as_ref()?;
        Ok(gamepad.dpad().into())
    }

    pub fn any_pressed(&self, button_inputs: Vec<PyGamepadButton>) -> PyResult<bool> {
        let gamepad = self.as_ref()?;
        let bevy_buttons: Vec<GamepadButton> = button_inputs.iter().map(|b| (*b).into()).collect();
        Ok(gamepad.any_pressed(bevy_buttons))
    }

    pub fn all_pressed(&self, button_inputs: Vec<PyGamepadButton>) -> PyResult<bool> {
        let gamepad = self.as_ref()?;
        let bevy_buttons: Vec<GamepadButton> = button_inputs.iter().map(|b| (*b).into()).collect();
        Ok(gamepad.all_pressed(bevy_buttons))
    }

    pub fn any_just_pressed(&self, button_inputs: Vec<PyGamepadButton>) -> PyResult<bool> {
        let gamepad = self.as_ref()?;
        let bevy_buttons: Vec<GamepadButton> = button_inputs.iter().map(|b| (*b).into()).collect();
        Ok(gamepad.any_just_pressed(bevy_buttons))
    }

    pub fn all_just_pressed(&self, button_inputs: Vec<PyGamepadButton>) -> PyResult<bool> {
        let gamepad = self.as_ref()?;
        let bevy_buttons: Vec<GamepadButton> = button_inputs.iter().map(|b| (*b).into()).collect();
        Ok(gamepad.all_just_pressed(bevy_buttons))
    }

    pub fn any_just_released(&self, button_inputs: Vec<PyGamepadButton>) -> PyResult<bool> {
        let gamepad = self.as_ref()?;
        let bevy_buttons: Vec<GamepadButton> = button_inputs.iter().map(|b| (*b).into()).collect();
        Ok(gamepad.any_just_released(bevy_buttons))
    }

    pub fn all_just_released(&self, button_inputs: Vec<PyGamepadButton>) -> PyResult<bool> {
        let gamepad = self.as_ref()?;
        let bevy_buttons: Vec<GamepadButton> = button_inputs.iter().map(|b| (*b).into()).collect();
        Ok(gamepad.all_just_released(bevy_buttons))
    }

    pub fn vendor_id(&self) -> PyResult<Option<u16>> {
        let gamepad = self.as_ref()?;
        Ok(gamepad.vendor_id())
    }

    pub fn product_id(&self) -> PyResult<Option<u16>> {
        let gamepad = self.as_ref()?;
        Ok(gamepad.product_id())
    }

    fn __repr__(&self) -> &'static str {
        "Gamepad"
    }
}
