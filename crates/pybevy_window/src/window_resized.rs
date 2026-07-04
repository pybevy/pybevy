use bevy::window::WindowResized;
use pybevy_core::{PyEntity, PyMessage};
use pybevy_macros::pymessage;
use pyo3::prelude::*;

#[pymessage(WindowResized)]
#[pyclass(name = "WindowResized", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWindowResized {
    pub width: f32,
    pub height: f32,
    pub window: PyEntity,
}

impl From<&WindowResized> for PyWindowResized {
    fn from(event: &WindowResized) -> Self {
        PyWindowResized {
            width: event.width,
            height: event.height,
            window: event.window.into(),
        }
    }
}

#[pymethods]
impl PyWindowResized {
    #[new]
    fn new(width: f32, height: f32, window: PyEntity) -> PyClassInitializer<Self> {
        (
            PyWindowResized {
                width,
                height,
                window,
            },
            PyMessage,
        ).into()
    }

    #[getter]
    fn width(&self) -> f32 {
        self.width
    }

    #[getter]
    fn height(&self) -> f32 {
        self.height
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    fn __repr__(&self) -> String {
        format!(
            "WindowResized(width={}, height={})",
            self.width, self.height
        )
    }
}
