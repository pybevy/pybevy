use bevy::{color::Color, pbr::DistanceFog};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::fog_falloff::PyFogFalloff;

#[pycomponent(DistanceFog, bridge, view_fields = [directional_light_exponent], batch_only_fields = [color, directional_light_color])]
#[pyclass(name = "DistanceFog", module = "pybevy.pbr", extends = PyComponent)]
#[derive(Debug)]
pub struct PyDistanceFog {
    pub(crate) storage: ComponentStorage<DistanceFog>,
}

#[pymethods]
impl PyDistanceFog {
    #[new]
    #[pyo3(signature = (
        color = Color::WHITE.into(),
        falloff = PyFogFalloff::Linear { start: 0.0, end: 100.0 },
        directional_light_color = Color::NONE.into(),
        directional_light_exponent = 8.0
    ))]
    pub fn new(
        color: PyColor,
        falloff: PyFogFalloff,
        directional_light_color: PyColor,
        directional_light_exponent: f32,
    ) -> PyResult<PyClassInitializer<Self>> {
        let color = color.try_into()?;
        let directional_light_color = directional_light_color.try_into()?;
        Ok(Self::from_owned(DistanceFog {
            color,
            falloff: falloff.try_into()?,
            directional_light_color,
            directional_light_exponent: validate_directional_light_exponent(
                directional_light_exponent,
            )?,
        })
        .into())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |fog| &fog.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        self.as_mut()?.color = color;
        Ok(())
    }

    #[getter]
    pub fn falloff(&self) -> PyResult<PyFogFalloff> {
        Ok(self.as_ref()?.falloff.clone().into())
    }

    #[setter]
    pub fn set_falloff(&mut self, falloff: PyFogFalloff) -> PyResult<()> {
        self.as_mut()?.falloff = falloff.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn directional_light_color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |fog| &fog.directional_light_color, py)
    }

    #[setter]
    pub fn set_directional_light_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        self.as_mut()?.directional_light_color = color;
        Ok(())
    }

    #[getter]
    pub fn directional_light_exponent(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.directional_light_exponent)
    }

    #[setter]
    pub fn set_directional_light_exponent(&mut self, exponent: f32) -> PyResult<()> {
        self.as_mut()?.directional_light_exponent = validate_directional_light_exponent(exponent)?;
        Ok(())
    }
}

fn validate_directional_light_exponent(exponent: f32) -> PyResult<f32> {
    if exponent.is_finite() {
        Ok(exponent)
    } else {
        Err(PyValueError::new_err(format!(
            "directional_light_exponent must be finite (got {exponent})"
        )))
    }
}
