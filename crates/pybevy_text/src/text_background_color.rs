use bevy::{color::Color, text::TextBackgroundColor};
use pybevy_color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

#[component_storage(TextBackgroundColor)]
#[pyclass(name = "TextBackgroundColor", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyTextBackgroundColor {
    pub(crate) storage: ComponentStorage<TextBackgroundColor>,
}

impl PyTextBackgroundColor {
    fn default_color() -> PyColor {
        TextBackgroundColor::default().0.into()
    }

    fn from_color(color: Color) -> PyResult<Py<Self>> {
        Python::attach(|py| Py::new(py, Self::from_owned(TextBackgroundColor(color))))
    }
}

#[pymethods]
impl PyTextBackgroundColor {
    #[new]
    #[pyo3(signature = (color = Self::default_color()))]
    pub fn new(color: PyColor) -> (Self, PyComponent) {
        Self::from_owned(TextBackgroundColor(color.into()))
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.0, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.0 = color.into();
        Ok(())
    }

    #[staticmethod]
    #[pyo3(name = "BLACK")]
    pub fn black() -> PyResult<Py<Self>> {
        Self::from_color(Color::BLACK)
    }

    #[staticmethod]
    #[pyo3(name = "WHITE")]
    pub fn white() -> PyResult<Py<Self>> {
        Self::from_color(Color::WHITE)
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
