use bevy::input::ButtonState;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ButtonState, empty_tuple)]
#[pyclass(
    name = "ButtonState",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object,
    hash
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyButtonState {
    Pressed(),
    Released(),
}

#[pymethods]
impl PyButtonState {
    pub fn is_pressed(&self) -> bool {
        matches!(self, PyButtonState::Pressed())
    }

    fn __str__(&self) -> String {
        match self {
            PyButtonState::Pressed() => "Pressed".to_string(),
            PyButtonState::Released() => "Released".to_string(),
        }
    }
}
