use bevy::window::WindowResizeConstraints;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

#[pyvalue]
#[pyclass(name = "WindowResizeConstraints", eq, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyWindowResizeConstraints {
    pub(crate) storage: ValueStorage<WindowResizeConstraints>,
}

impl PartialEq for PyWindowResizeConstraints {
    fn eq(&self, other: &Self) -> bool {
        match (self.to_bevy(), other.to_bevy()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl From<WindowResizeConstraints> for PyWindowResizeConstraints {
    fn from(value: WindowResizeConstraints) -> Self {
        PyWindowResizeConstraints::from_owned(value)
    }
}

impl TryFrom<PyWindowResizeConstraints> for WindowResizeConstraints {
    type Error = PyErr;

    fn try_from(value: PyWindowResizeConstraints) -> PyResult<Self> {
        Ok(value.storage.get()?)
    }
}

impl TryFrom<&PyWindowResizeConstraints> for WindowResizeConstraints {
    type Error = PyErr;

    fn try_from(value: &PyWindowResizeConstraints) -> PyResult<Self> {
        Ok(value.storage.get()?)
    }
}

#[pymethods]
impl PyWindowResizeConstraints {
    #[new]
    #[pyo3(signature = (
        min_width = 180.0,
        min_height = 120.0,
        max_width = f32::INFINITY,
        max_height = f32::INFINITY,
    ))]
    pub fn new(min_width: f32, min_height: f32, max_width: f32, max_height: f32) -> Self {
        PyWindowResizeConstraints::from_owned(WindowResizeConstraints {
            min_width,
            min_height,
            max_width,
            max_height,
        })
    }

    #[getter]
    pub fn min_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.min_width)
    }

    #[setter]
    pub fn set_min_width(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.min_width = value;
        Ok(())
    }

    #[getter]
    pub fn min_height(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.min_height)
    }

    #[setter]
    pub fn set_min_height(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.min_height = value;
        Ok(())
    }

    #[getter]
    pub fn max_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_width)
    }

    #[setter]
    pub fn set_max_width(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.max_width = value;
        Ok(())
    }

    #[getter]
    pub fn max_height(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_height)
    }

    #[setter]
    pub fn set_max_height(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.max_height = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let c = self.to_bevy()?;
        Ok(format!(
            "WindowResizeConstraints(min_width={}, min_height={}, max_width={}, max_height={})",
            c.min_width, c.min_height, c.max_width, c.max_height
        ))
    }
}
