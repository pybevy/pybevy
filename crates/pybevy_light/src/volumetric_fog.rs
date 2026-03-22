use bevy::light::VolumetricFog;
use pybevy_color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

#[component_storage(VolumetricFog)]
#[pyclass(name = "VolumetricFog", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyVolumetricFog {
    pub(crate) storage: ComponentStorage<VolumetricFog>,
}

impl PyVolumetricFog {
    fn default_ambient_color() -> PyColor {
        VolumetricFog::default().ambient_color.into()
    }

    fn default_ambient_intensity() -> f32 {
        VolumetricFog::default().ambient_intensity
    }

    fn default_step_count() -> u32 {
        VolumetricFog::default().step_count
    }

    fn default_jitter() -> f32 {
        VolumetricFog::default().jitter
    }
}

#[pymethods]
impl PyVolumetricFog {
    #[new]
    #[pyo3(signature = (
        ambient_color = Self::default_ambient_color(),
        ambient_intensity = Self::default_ambient_intensity(),
        step_count = Self::default_step_count(),
        jitter = Self::default_jitter()
    ))]
    pub fn new(
        ambient_color: PyColor,
        ambient_intensity: f32,
        step_count: u32,
        jitter: f32,
    ) -> (Self, PyComponent) {
        Self::from_owned(VolumetricFog {
            ambient_color: ambient_color.into(),
            ambient_intensity,
            step_count,
            jitter,
        })
    }

    #[getter]
    pub fn ambient_color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.ambient_color, py)
    }

    #[setter]
    pub fn set_ambient_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.ambient_color = color.into();
        Ok(())
    }

    #[getter]
    pub fn ambient_intensity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.ambient_intensity)
    }

    #[setter]
    pub fn set_ambient_intensity(&mut self, intensity: f32) -> PyResult<()> {
        self.as_mut()?.ambient_intensity = intensity;
        Ok(())
    }

    #[getter]
    pub fn step_count(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.step_count)
    }

    #[setter]
    pub fn set_step_count(&mut self, count: u32) -> PyResult<()> {
        self.as_mut()?.step_count = count;
        Ok(())
    }

    #[getter]
    pub fn jitter(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.jitter)
    }

    #[setter]
    pub fn set_jitter(&mut self, jitter: f32) -> PyResult<()> {
        self.as_mut()?.jitter = jitter;
        Ok(())
    }
}
