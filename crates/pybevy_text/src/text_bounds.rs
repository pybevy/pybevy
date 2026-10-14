use bevy::text::TextBounds;
use pybevy_core::{ComponentStorage, PyComponent, domain::F32Input};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

// Parley asserts on a negative or -inf line width; +inf means unbounded.
#[pycomponent(TextBounds, bridge, domains = [width => [non_negative], height => [non_negative]])]
#[pyclass(name = "TextBounds", module = "pybevy.text", extends = PyComponent)]
#[derive(Debug)]
pub struct PyTextBounds {
    pub(crate) storage: ComponentStorage<TextBounds>,
}

impl PyTextBounds {
    fn checked(width: Option<f64>, height: Option<f64>) -> PyResult<TextBounds> {
        Ok(TextBounds {
            width: Self::check_width(width.map(F32Input))?.map(F32Input::narrowed),
            height: Self::check_height(height.map(F32Input))?.map(F32Input::narrowed),
        })
    }

    fn create(width: Option<f64>, height: Option<f64>) -> PyResult<Py<Self>> {
        let bounds = Self::checked(width, height)?;
        Python::attach(|py| Py::new(py, Self::from_owned(bounds)))
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
    pub fn new(width: Option<f64>, height: Option<f64>) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(Self::checked(width, height)?).into())
    }

    #[getter]
    pub fn width(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.width)
    }

    #[setter]
    pub fn set_width(&mut self, width: Option<f64>) -> PyResult<()> {
        let width = Self::check_width(width.map(F32Input))?.map(F32Input::narrowed);
        self.as_mut()?.width = width;
        Ok(())
    }

    #[getter]
    pub fn height(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.height)
    }

    #[setter]
    pub fn set_height(&mut self, height: Option<f64>) -> PyResult<()> {
        let height = Self::check_height(height.map(F32Input))?.map(F32Input::narrowed);
        self.as_mut()?.height = height;
        Ok(())
    }

    #[staticmethod]
    pub fn new_horizontal(width: f64) -> PyResult<Py<Self>> {
        Self::create(Some(width), None)
    }

    #[staticmethod]
    pub fn new_vertical(height: f64) -> PyResult<Py<Self>> {
        Self::create(None, Some(height))
    }
}
