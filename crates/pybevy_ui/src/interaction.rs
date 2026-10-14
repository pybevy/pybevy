use bevy::ui::Interaction;
use pybevy_core::PyComponent;
use pybevy_macros::{pyenum, pywrap};
use pyo3::prelude::*;

#[pyenum(Interaction, manual)]
#[pywrap(Interaction, bridge)]
#[pyclass(name = "Interaction", module = "pybevy.ui", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyInteraction(pub(crate) Interaction);

#[pymethods]
impl PyInteraction {
    #[classattr]
    fn __pybevy_variant_aliases__() -> Vec<(&'static str, &'static str)> {
        vec![("None", "None_")]
    }

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(Interaction::None).into()
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

    pub fn __str__(&self) -> &'static str {
        match self.0 {
            Interaction::None => "none",
            Interaction::Hovered => "hovered",
            Interaction::Pressed => "pressed",
        }
    }
}
