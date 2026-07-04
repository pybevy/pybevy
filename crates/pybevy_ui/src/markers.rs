use bevy::ui::{
    Checked, InteractionDisabled, IsDefaultUiCamera, Pressed,
    widget::{Button, Label},
};
use pybevy_core::PyComponent;
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(Label, unit, bridge)]
#[pyclass(name = "Label", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyLabel;

impl From<Label> for PyLabel {
    fn from(_: Label) -> Self {
        PyLabel
    }
}

impl From<PyLabel> for Label {
    fn from(_: PyLabel) -> Self {
        Label
    }
}

impl TryFrom<&Label> for PyLabel {
    type Error = PyErr;

    fn try_from(_: &Label) -> PyResult<Self> {
        Ok(PyLabel)
    }
}

#[pymethods]
impl PyLabel {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyLabel, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "Label()"
    }

    pub fn __str__(&self) -> &'static str {
        "Label()"
    }
}

#[pycomponent(Checked, unit, bridge)]
#[pyclass(name = "Checked", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyChecked;

impl From<Checked> for PyChecked {
    fn from(_: Checked) -> Self {
        PyChecked
    }
}

impl From<PyChecked> for Checked {
    fn from(_: PyChecked) -> Self {
        Checked
    }
}

impl TryFrom<&Checked> for PyChecked {
    type Error = PyErr;

    fn try_from(_: &Checked) -> PyResult<Self> {
        Ok(PyChecked)
    }
}

#[pymethods]
impl PyChecked {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyChecked, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "Checked()"
    }

    pub fn __str__(&self) -> &'static str {
        "Checked()"
    }
}

#[pycomponent(Pressed, unit, bridge)]
#[pyclass(name = "Pressed", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyPressed;

impl From<Pressed> for PyPressed {
    fn from(_: Pressed) -> Self {
        PyPressed
    }
}

impl From<PyPressed> for Pressed {
    fn from(_: PyPressed) -> Self {
        Pressed
    }
}

impl TryFrom<&Pressed> for PyPressed {
    type Error = PyErr;

    fn try_from(_: &Pressed) -> PyResult<Self> {
        Ok(PyPressed)
    }
}

#[pymethods]
impl PyPressed {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyPressed, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "Pressed()"
    }

    pub fn __str__(&self) -> &'static str {
        "Pressed()"
    }
}

#[pycomponent(InteractionDisabled, unit, bridge)]
#[pyclass(name = "InteractionDisabled", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyInteractionDisabled;

impl From<InteractionDisabled> for PyInteractionDisabled {
    fn from(_: InteractionDisabled) -> Self {
        PyInteractionDisabled
    }
}

impl From<PyInteractionDisabled> for InteractionDisabled {
    fn from(_: PyInteractionDisabled) -> Self {
        InteractionDisabled
    }
}

impl TryFrom<&InteractionDisabled> for PyInteractionDisabled {
    type Error = PyErr;

    fn try_from(_: &InteractionDisabled) -> PyResult<Self> {
        Ok(PyInteractionDisabled)
    }
}

#[pymethods]
impl PyInteractionDisabled {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyInteractionDisabled, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "InteractionDisabled()"
    }

    pub fn __str__(&self) -> &'static str {
        "InteractionDisabled()"
    }
}

#[pycomponent(IsDefaultUiCamera, unit, bridge)]
#[pyclass(name = "IsDefaultUiCamera", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyIsDefaultUiCamera;

impl From<IsDefaultUiCamera> for PyIsDefaultUiCamera {
    fn from(_: IsDefaultUiCamera) -> Self {
        PyIsDefaultUiCamera
    }
}

impl From<PyIsDefaultUiCamera> for IsDefaultUiCamera {
    fn from(_: PyIsDefaultUiCamera) -> Self {
        IsDefaultUiCamera
    }
}

impl TryFrom<&IsDefaultUiCamera> for PyIsDefaultUiCamera {
    type Error = PyErr;

    fn try_from(_: &IsDefaultUiCamera) -> PyResult<Self> {
        Ok(PyIsDefaultUiCamera)
    }
}

#[pymethods]
impl PyIsDefaultUiCamera {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyIsDefaultUiCamera, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "IsDefaultUiCamera"
    }
}

#[pycomponent(Button, unit, bridge)]
#[pyclass(name = "Button", extends = PyComponent, frozen, eq, skip_from_py_object)]
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
