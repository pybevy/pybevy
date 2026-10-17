use bevy::picking::hover::Hovered;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;

#[pywrap(Hovered, bridge, copy)]
#[pyclass(name = "Hovered", module = "pybevy.picking", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyHovered(pub(crate) Hovered);

#[pymethods]
impl PyHovered {
    #[new]
    #[pyo3(signature = (value = false))]
    pub fn new(value: bool) -> PyClassInitializer<Self> {
        Self::from_owned(Hovered(value)).into()
    }

    #[getter]
    pub fn value(&self) -> bool {
        self.0.0
    }

    pub fn get(&self) -> bool {
        self.0.get()
    }

    pub fn __repr__(&self) -> String {
        format!("Hovered({})", if self.0.0 { "True" } else { "False" })
    }
}
