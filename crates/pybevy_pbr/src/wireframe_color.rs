use bevy::{color::Color, pbr::wireframe::WireframeColor};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(WireframeColor, bridge, batch_only_fields = [color])]
#[pyclass(name = "WireframeColor", module = "pybevy.pbr", extends = PyComponent)]
#[derive(Debug)]
pub struct PyWireframeColor {
    pub(crate) storage: ComponentStorage<WireframeColor>,
}

#[pymethods]
impl PyWireframeColor {
    #[new]
    #[pyo3(signature = (color = Color::WHITE.into()))]
    pub fn new(color: PyColor) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(WireframeColor {
            color: color.try_into()?,
        })
        .into())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |wireframe| &wireframe.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        self.as_mut()?.color = color;
        Ok(())
    }
}
