use bevy::ui::UiScale;
use pybevy_core::PyResource;
use pyo3::prelude::*;

/// Note: This uses a simple value storage since UiScale doesn't impl Clone in Bevy.
#[pyclass(name = "UiScale", extends = PyResource, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyUiScale {
    pub value: f32,
}

impl PyUiScale {
    pub fn from_bevy(scale: &UiScale) -> Self {
        PyUiScale { value: scale.0 }
    }
}

impl From<UiScale> for PyUiScale {
    fn from(scale: UiScale) -> Self {
        PyUiScale { value: scale.0 }
    }
}

impl From<PyUiScale> for UiScale {
    fn from(py: PyUiScale) -> Self {
        UiScale(py.value)
    }
}

impl From<&PyUiScale> for UiScale {
    fn from(py: &PyUiScale) -> Self {
        UiScale(py.value)
    }
}

#[pymethods]
impl PyUiScale {
    #[new]
    #[pyo3(signature = (scale = 1.0))]
    pub fn new(scale: f32) -> (Self, PyResource) {
        (PyUiScale { value: scale }, PyResource)
    }

    #[getter]
    pub fn scale(&self) -> f32 {
        self.value
    }

    #[setter]
    pub fn set_scale(&mut self, value: f32) {
        self.value = value;
    }

    pub fn __repr__(&self) -> String {
        format!("UiScale({})", self.value)
    }

    pub fn __float__(&self) -> f64 {
        self.value as f64
    }
}
