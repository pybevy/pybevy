use bevy::{color::Color, ui::BackgroundColor};
use pybevy_color::color::{IntoColorValue, PyColor};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(BackgroundColor, bridge)]
#[pyclass(name = "BackgroundColor", module = "pybevy.ui", extends = PyComponent, eq)]
#[derive(Debug, PartialEq)]
pub struct PyBackgroundColor {
    pub(crate) storage: ComponentStorage<BackgroundColor>,
}

#[pymethods]
impl PyBackgroundColor {
    #[new]
    #[pyo3(signature = (color = IntoColorValue(Color::NONE)))]
    pub fn new(color: IntoColorValue) -> PyResult<PyClassInitializer<Self>> {
        let color = color.0;
        Ok(Self::from_owned(BackgroundColor(color)).into())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |color| &color.0, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: IntoColorValue) -> PyResult<()> {
        self.as_mut()?.0 = color.0;
        Ok(())
    }

    pub fn __repr__(&self, py: Python) -> PyResult<String> {
        let color_py = PyColor::from_color(self.as_ref()?.0, py)?;
        let color_repr = color_py.bind(py).repr()?.to_string();
        Ok(format!("BackgroundColor({})", color_repr))
    }
}
