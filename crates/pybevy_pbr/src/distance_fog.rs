use bevy::{color::Color, pbr::DistanceFog};
use pybevy_color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

use crate::PyFogFalloff;

#[component_storage(DistanceFog, bridge, view_fields = [directional_light_exponent], batch_only_fields = [color, directional_light_color])]
#[pyclass(name = "DistanceFog", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyDistanceFog {
    pub(crate) storage: ComponentStorage<DistanceFog>,
}

#[pymethods]
impl PyDistanceFog {
    #[new]
    #[pyo3(signature = (
        color = PyColor(Color::WHITE),
        falloff = PyFogFalloff::Linear { start: 0.0, end: 100.0 },
        directional_light_color = PyColor(Color::WHITE),
        directional_light_exponent = 8.0
    ))]
    pub fn new(
        color: PyColor,
        falloff: PyFogFalloff,
        directional_light_color: PyColor,
        directional_light_exponent: f32,
    ) -> (Self, PyComponent) {
        Self::from_owned(DistanceFog {
            color: color.0,
            falloff: falloff.into(),
            directional_light_color: directional_light_color.0,
            directional_light_exponent,
        })
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

    #[getter]
    pub fn falloff(&self) -> PyResult<PyFogFalloff> {
        Ok(self.as_ref()?.falloff.clone().into())
    }

    #[setter]
    pub fn set_falloff(&mut self, falloff: PyFogFalloff) -> PyResult<()> {
        self.as_mut()?.falloff = falloff.into();
        Ok(())
    }

    #[getter]
    pub fn directional_light_color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.directional_light_color, py)
    }

    #[setter]
    pub fn set_directional_light_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.directional_light_color = color.0;
        Ok(())
    }

    #[getter]
    pub fn directional_light_exponent(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.directional_light_exponent)
    }

    #[setter]
    pub fn set_directional_light_exponent(&mut self, exponent: f32) -> PyResult<()> {
        self.as_mut()?.directional_light_exponent = exponent;
        Ok(())
    }
}
