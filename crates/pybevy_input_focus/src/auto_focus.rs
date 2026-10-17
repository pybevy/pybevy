use bevy::input_focus::AutoFocus;
use pybevy_core::PyComponent;
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(AutoFocus, unit, bridge)]
#[pyclass(name = "AutoFocus", module = "pybevy.input_focus", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyAutoFocus;

impl From<AutoFocus> for PyAutoFocus {
    fn from(_: AutoFocus) -> Self {
        PyAutoFocus
    }
}

impl From<PyAutoFocus> for AutoFocus {
    fn from(_: PyAutoFocus) -> Self {
        AutoFocus
    }
}

impl TryFrom<&AutoFocus> for PyAutoFocus {
    type Error = PyErr;

    fn try_from(_: &AutoFocus) -> PyResult<Self> {
        Ok(PyAutoFocus)
    }
}

#[pymethods]
impl PyAutoFocus {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyAutoFocus, PyComponent).into()
    }

    pub fn __repr__(&self) -> &'static str {
        "AutoFocus()"
    }
}
