use bevy::{color::Color, pbr::wireframe::WireframeColor};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(WireframeColor, bridge, batch_only_fields = [color])]
#[pyclass(name = "WireframeColor", extends = PyComponent)]
#[derive(Debug)]
pub struct PyWireframeColor {
    pub(crate) storage: ComponentStorage<WireframeColor>,
}

#[pymethods]
impl PyWireframeColor {
    #[new]
    #[pyo3(signature = (color = PyColor(Color::WHITE)))]
    pub fn new(color: PyColor) -> PyClassInitializer<Self> {
        Self::from_owned(WireframeColor { color: color.0 }).into()
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.color = color.0;
        Ok(())
    }
}
