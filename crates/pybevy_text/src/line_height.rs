use bevy::text::LineHeight;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;

#[pywrap(LineHeight, bridge, copy)]
#[pyclass(name = "LineHeight", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyLineHeight(pub(crate) LineHeight);

#[pymethods]
impl PyLineHeight {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(LineHeight::default()).into()
    }

    #[staticmethod]
    #[pyo3(name = "Px")]
    pub fn px(py: Python<'_>, pixels: f32) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(LineHeight::Px(pixels)))
    }

    #[staticmethod]
    #[pyo3(name = "RelativeToFont")]
    pub fn relative_to_font(py: Python<'_>, scale: f32) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(LineHeight::RelativeToFont(scale)))
    }

    pub fn __repr__(&self) -> String {
        match self.0 {
            LineHeight::Px(px) => format!("LineHeight.Px({px})"),
            LineHeight::RelativeToFont(scale) => format!("LineHeight.RelativeToFont({scale})"),
        }
    }
}
