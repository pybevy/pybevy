use bevy::ui::Interaction;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;

#[pywrap(Interaction, bridge)]
#[pyclass(name = "Interaction", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyInteraction(pub(crate) Interaction);

#[pymethods]
impl PyInteraction {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(Interaction::None).into()
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
    #[pyo3(name = "None_")]
    pub fn none(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Interaction::None))
    }

    #[staticmethod]
    #[pyo3(name = "Hovered")]
    pub fn hovered(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Interaction::Hovered))
    }

    #[staticmethod]
    #[pyo3(name = "Pressed")]
    pub fn pressed(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Interaction::Pressed))
    }

    pub fn __repr__(&self) -> String {
        match self.0 {
            Interaction::None => "Interaction.None_".to_string(),
            Interaction::Hovered => "Interaction.Hovered".to_string(),
            Interaction::Pressed => "Interaction.Pressed".to_string(),
        }
    }

    pub fn __str__(&self) -> String {
        self.state()
    }
}
