use bevy::window::WindowResizeConstraints;
use pyo3::prelude::*;

#[pyclass(name = "WindowResizeConstraints", eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyWindowResizeConstraints(pub WindowResizeConstraints);

impl From<WindowResizeConstraints> for PyWindowResizeConstraints {
    fn from(value: WindowResizeConstraints) -> Self {
        PyWindowResizeConstraints(value)
    }
}

impl From<PyWindowResizeConstraints> for WindowResizeConstraints {
    fn from(value: PyWindowResizeConstraints) -> Self {
        value.0
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
        PyWindowResizeConstraints(WindowResizeConstraints {
            min_width,
            min_height,
            max_width,
            max_height,
        })
    }

    #[getter]
    pub fn min_width(&self) -> f32 {
        self.0.min_width
    }

    #[setter]
    pub fn set_min_width(&mut self, value: f32) {
        self.0.min_width = value;
    }

    #[getter]
    pub fn min_height(&self) -> f32 {
        self.0.min_height
    }

    #[setter]
    pub fn set_min_height(&mut self, value: f32) {
        self.0.min_height = value;
    }

    #[getter]
    pub fn max_width(&self) -> f32 {
        self.0.max_width
    }

    #[setter]
    pub fn set_max_width(&mut self, value: f32) {
        self.0.max_width = value;
    }

    #[getter]
    pub fn max_height(&self) -> f32 {
        self.0.max_height
    }

    #[setter]
    pub fn set_max_height(&mut self, value: f32) {
        self.0.max_height = value;
    }

    pub fn __repr__(&self) -> String {
        format!(
            "WindowResizeConstraints(min_width={}, min_height={}, max_width={}, max_height={})",
            self.0.min_width, self.0.min_height, self.0.max_width, self.0.max_height
        )
    }
}
