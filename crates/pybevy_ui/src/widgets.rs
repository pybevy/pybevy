use bevy::ui_widgets::{Button, TextInput};
use pybevy_core::PyComponent;
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(Button, unit, bridge)]
#[pyclass(name = "Button", module = "pybevy.ui_widgets", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyButton;

impl From<Button> for PyButton {
    fn from(_: Button) -> Self {
        PyButton
    }
}

impl From<PyButton> for Button {
    fn from(_: PyButton) -> Self {
        Button
    }
}

impl TryFrom<&Button> for PyButton {
    type Error = PyErr;

    fn try_from(_: &Button) -> PyResult<Self> {
        Ok(PyButton)
    }
}

#[pymethods]
impl PyButton {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyButton, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "Button()"
    }
}

#[pycomponent(TextInput, unit, bridge)]
#[pyclass(name = "TextInput", module = "pybevy.ui_widgets", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyTextInput;

impl From<TextInput> for PyTextInput {
    fn from(_: TextInput) -> Self {
        PyTextInput
    }
}

impl From<PyTextInput> for TextInput {
    fn from(_: PyTextInput) -> Self {
        TextInput
    }
}

impl TryFrom<&TextInput> for PyTextInput {
    type Error = PyErr;

    fn try_from(_: &TextInput) -> PyResult<Self> {
        Ok(PyTextInput)
    }
}

#[pymethods]
impl PyTextInput {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyTextInput, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "TextInput()"
    }
}
