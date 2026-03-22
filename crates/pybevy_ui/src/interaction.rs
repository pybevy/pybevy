use bevy::ui::Interaction;
use pybevy_core::PyComponent;
use pybevy_macros::newtype_storage;
use pyo3::prelude::*;

#[newtype_storage(Interaction)]
#[pyclass(name = "Interaction", extends = PyComponent, frozen, eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyInteraction(pub(crate) Interaction);

#[pymethods]
impl PyInteraction {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        Self::from_owned(Interaction::None)
    }

    #[getter]
    pub fn is_none(&self) -> bool {
        matches!(self.0, Interaction::None)
    }

    #[getter]
    pub fn is_hovered(&self) -> bool {
        matches!(self.0, Interaction::Hovered)
    }

    #[getter]
    pub fn is_pressed(&self) -> bool {
        matches!(self.0, Interaction::Pressed)
    }

    #[getter]
    pub fn state(&self) -> String {
        match self.0 {
            Interaction::None => "none".to_string(),
            Interaction::Hovered => "hovered".to_string(),
            Interaction::Pressed => "pressed".to_string(),
        }
    }

    #[staticmethod]
    pub fn none(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Interaction::None))
    }

    #[staticmethod]
    pub fn hovered(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Interaction::Hovered))
    }

    #[staticmethod]
    pub fn pressed(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Interaction::Pressed))
    }

    pub fn __repr__(&self) -> String {
        format!("Interaction({})", self.state())
    }

    pub fn __str__(&self) -> String {
        self.state()
    }
}
