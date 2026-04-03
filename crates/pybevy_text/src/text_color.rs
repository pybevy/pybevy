use bevy::{color::Color, text::TextColor};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(TextColor, bridge, batch_only_fields = [0 as color])]
#[pyclass(name = "TextColor", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyTextColor {
    pub(crate) storage: ComponentStorage<TextColor>,
}

impl PyTextColor {
    fn default_color() -> PyColor {
        TextColor::default().0.into()
    }
}

#[pymethods]
impl PyTextColor {
    #[new]
    #[pyo3(signature = (color = Self::default_color()))]
    pub fn new(color: PyColor) -> (Self, PyComponent) {
        Self::from_owned(TextColor(color.into()))
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
        Python::attach(|py| Py::new(py, Self::from_owned(TextColor(Color::BLACK))))
    }

    #[staticmethod]
    #[pyo3(name = "WHITE")]
    pub fn white() -> PyResult<Py<Self>> {
        Python::attach(|py| Py::new(py, Self::from_owned(TextColor(Color::WHITE))))
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
