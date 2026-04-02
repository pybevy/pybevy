use bevy::input::{
    ButtonInput,
    keyboard::{Key, KeyCode, KeyboardInput},
};
use pybevy_core::PyMessage;

use crate::{button_state::PyButtonState, key_code::PyKeyCode, keyboard_input::PyKeyboardInput};
pub trait PyKeyboardInputExt {
    fn from_bevy(
        event: &KeyboardInput,
        keyboard: &ButtonInput<KeyCode>,
    ) -> Option<(PyKeyboardInput, PyMessage)>;
}

impl PyKeyboardInputExt for PyKeyboardInput {
    fn from_bevy(
        event: &KeyboardInput,
        keyboard: &ButtonInput<KeyCode>,
    ) -> Option<(PyKeyboardInput, PyMessage)> {
        let key_code = PyKeyCode::from_bevy(event.key_code)?;
        let state: PyButtonState = event.state.into();

        let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
        let ctrl =
            keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
        let alt = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
        let super_key =
            keyboard.pressed(KeyCode::SuperLeft) || keyboard.pressed(KeyCode::SuperRight);

        let logical_key = match &event.logical_key {
            Key::Character(s) => Some(s.to_string()),
            _ => None,
        };

        let text = event.text.as_ref().map(|s| s.to_string());

        Some((
            PyKeyboardInput {
                key_code,
                state,
                logical_key,
                shift,
                ctrl,
                alt,
                super_key,
                repeat: event.repeat,
                text,
                window: event.window.into(),
            },
            PyMessage,
        ))
    }
}
