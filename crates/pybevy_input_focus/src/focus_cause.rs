use bevy::input_focus::FocusCause;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(FocusCause)]
#[pyclass(
    name = "FocusCause",
    module = "pybevy.input_focus",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyFocusCause {
    Navigated,
    Pressed,
    Auto,
}
