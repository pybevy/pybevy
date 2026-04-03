use bevy::text::TextBounds;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(TextBounds, bridge)]
#[pyclass(name = "TextBounds", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyTextBounds {
    pub(crate) storage: ComponentStorage<TextBounds>,
}

impl PyTextBounds {
    fn create(width: Option<f32>, height: Option<f32>) -> PyResult<Py<Self>> {
        Python::attach(|py| Py::new(py, Self::from_owned(TextBounds { width, height })))
    }
}

#[pymethods]
impl PyTextBounds {
    #[staticmethod]
    #[pyo3(name = "UNBOUNDED")]
    pub fn unbounded() -> PyResult<Py<Self>> {
        Self::create(None, None)
    }

    #[new]
    #[pyo3(signature = (width = None, height = None))]
    pub fn new(width: Option<f32>, height: Option<f32>) -> (Self, PyComponent) {
        Self::from_owned(TextBounds { width, height })
    }

    #[getter]
    pub fn width(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.width)
    }

    #[setter]
    pub fn set_width(&mut self, width: Option<f32>) -> PyResult<()> {
        self.as_mut()?.width = width;
        Ok(())
    }

    #[getter]
    pub fn height(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.height)
    }

    #[setter]
    pub fn set_height(&mut self, height: Option<f32>) -> PyResult<()> {
        self.as_mut()?.height = height;
        Ok(())
    }

    #[staticmethod]
    pub fn new_horizontal(width: f32) -> PyResult<Py<Self>> {
        Self::create(Some(width), None)
    }

    #[staticmethod]
    pub fn new_vertical(height: f32) -> PyResult<Py<Self>> {
        Self::create(None, Some(height))
    }
}
