use bevy::text::TextBounds;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyValueError, prelude::*};

fn validate_dimension(value: Option<f32>, name: &'static str) -> PyResult<Option<f32>> {
    if value.is_some_and(f32::is_nan) {
        return Err(PyValueError::new_err(format!(
            "TextBounds.{name} must not be NaN"
        )));
    }
    Ok(value)
}

#[pycomponent(TextBounds, bridge)]
#[pyclass(name = "TextBounds", module = "pybevy.text", extends = PyComponent)]
#[derive(Debug)]
pub struct PyTextBounds {
    pub(crate) storage: ComponentStorage<TextBounds>,
}

impl PyTextBounds {
    fn create(width: Option<f32>, height: Option<f32>) -> PyResult<Py<Self>> {
        let width = validate_dimension(width, "width")?;
        let height = validate_dimension(height, "height")?;
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
    pub fn new(width: Option<f32>, height: Option<f32>) -> PyResult<PyClassInitializer<Self>> {
        let width = validate_dimension(width, "width")?;
        let height = validate_dimension(height, "height")?;
        Ok(Self::from_owned(TextBounds { width, height }).into())
    }

    #[getter]
    pub fn width(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.width)
    }

    #[setter]
    pub fn set_width(&mut self, width: Option<f32>) -> PyResult<()> {
        let width = validate_dimension(width, "width")?;
        self.as_mut()?.width = width;
        Ok(())
    }

    #[getter]
    pub fn height(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.height)
    }

    #[setter]
    pub fn set_height(&mut self, height: Option<f32>) -> PyResult<()> {
        let height = validate_dimension(height, "height")?;
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
