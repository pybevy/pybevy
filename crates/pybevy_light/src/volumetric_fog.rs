use bevy::{color::Color, light::VolumetricFog};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent, domain::IntegerInput};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

// The raymarch takes step_count samples per pixel; far larger counts stall the GPU.
#[pycomponent(
    VolumetricFog,
    bridge,
    view_fields = [ambient_intensity, step_count, jitter],
    batch_only_fields = [ambient_color],
    domains = [step_count => [range_inclusive(0.0, 1024.0)], ambient_intensity => [finite], jitter => [finite]]
)]
#[pyclass(name = "VolumetricFog", module = "pybevy.light", extends = PyComponent)]
#[derive(Debug)]
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

    fn default_step_count() -> IntegerInput<u32> {
        IntegerInput::new(VolumetricFog::default().step_count.into())
    }

    fn default_jitter() -> f32 {
        VolumetricFog::default().jitter
    }
}

#[pymethods]
impl PyVolumetricFog {
    #[new]
    #[pyo3(signature = (
        *,
        ambient_color = Self::default_ambient_color(),
        ambient_intensity = Self::default_ambient_intensity(),
        jitter = Self::default_jitter(),
        step_count = Self::default_step_count()
    ))]
    pub fn new(
        ambient_color: PyColor,
        ambient_intensity: f32,
        jitter: f32,
        step_count: IntegerInput<u32>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let ambient_intensity = Self::check_ambient_intensity(ambient_intensity)?;
        let jitter = Self::check_jitter(jitter)?;
        let step_count = Self::check_step_count(step_count)?.stored();
        let ambient_color = Color::try_from(ambient_color)?;
        Ok(Self::from_owned(VolumetricFog {
            ambient_color,
            ambient_intensity,
            step_count,
            jitter,
        })
        .into())
    }

    #[getter]
    pub fn ambient_color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |fog| &fog.ambient_color, py)
    }

    #[setter]
    pub fn set_ambient_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = Color::try_from(color)?;
        self.as_mut()?.ambient_color = color;
        Ok(())
    }

    #[getter]
    pub fn ambient_intensity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.ambient_intensity)
    }

    #[setter]
    pub fn set_ambient_intensity(&mut self, intensity: f32) -> PyResult<()> {
        let intensity = Self::check_ambient_intensity(intensity)?;
        self.as_mut()?.ambient_intensity = intensity;
        Ok(())
    }

    #[getter]
    pub fn step_count(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.step_count)
    }

    #[setter]
    pub fn set_step_count(&mut self, count: IntegerInput<u32>) -> PyResult<()> {
        let count = Self::check_step_count(count)?.stored();
        self.as_mut()?.step_count = count;
        Ok(())
    }

    #[getter]
    pub fn jitter(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.jitter)
    }

    #[setter]
    pub fn set_jitter(&mut self, jitter: f32) -> PyResult<()> {
        let jitter = Self::check_jitter(jitter)?;
        self.as_mut()?.jitter = jitter;
        Ok(())
    }
}
