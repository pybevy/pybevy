use bevy::{color::Color, ui::BackgroundColor};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(BackgroundColor, bridge)]
#[pyclass(name = "BackgroundColor", extends = PyComponent, eq)]
#[derive(Debug, PartialEq)]
pub struct PyBackgroundColor {
    pub(crate) storage: ComponentStorage<BackgroundColor>,
}

#[pymethods]
impl PyBackgroundColor {
    #[new]
    #[pyo3(signature = (color = None))]
    pub fn new(color: Option<PyColor>) -> PyResult<PyClassInitializer<Self>> {
        let color = color
            .map(Color::try_from)
            .transpose()?
            .unwrap_or(Color::NONE);
        Ok(Self::from_owned(BackgroundColor(color)).into())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |color| &color.0, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = Color::try_from(color)?;
        self.as_mut()?.0 = color;
        Ok(())
    }

    pub fn __repr__(&self, py: Python) -> PyResult<String> {
        let color_py = PyColor::from_color(self.as_ref()?.0, py)?;
        let color_repr = color_py.bind(py).repr()?.to_string();
        Ok(format!("BackgroundColor({})", color_repr))
    }
}
