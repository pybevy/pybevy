use bevy::{color::Color, text::TextBackgroundColor};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(TextBackgroundColor, bridge, batch_only_fields = [0 as color])]
#[pyclass(name = "TextBackgroundColor", extends = PyComponent)]
#[derive(Debug)]
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
    pub fn new(color: PyColor) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(TextBackgroundColor(color.try_into()?)).into())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |color| &color.0, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        self.as_mut()?.0 = color;
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
