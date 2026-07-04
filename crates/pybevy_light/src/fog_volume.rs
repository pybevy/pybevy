use bevy::light::FogVolume;
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pycomponent;
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

#[pycomponent(FogVolume, bridge, view_fields = [
    density_factor,
    absorption,
    scattering,
    scattering_asymmetry,
    light_intensity
], batch_only_fields = [fog_color, light_tint])]
#[pyclass(name = "FogVolume", extends = PyComponent)]
pub struct PyFogVolume {
    pub(crate) storage: ComponentStorage<FogVolume>,
}

impl PyFogVolume {
    fn default_fog_color() -> PyColor {
        FogVolume::default().fog_color.into()
    }

    fn default_density_factor() -> f32 {
        FogVolume::default().density_factor
    }

    fn default_density_texture_offset() -> PyVec3 {
        FogVolume::default().density_texture_offset.into()
    }

    fn default_absorption() -> f32 {
        FogVolume::default().absorption
    }

    fn default_scattering() -> f32 {
        FogVolume::default().scattering
    }

    fn default_scattering_asymmetry() -> f32 {
        FogVolume::default().scattering_asymmetry
    }

    fn default_light_tint() -> PyColor {
        FogVolume::default().light_tint.into()
    }

    fn default_light_intensity() -> f32 {
        FogVolume::default().light_intensity
    }
}

#[pymethods]
impl PyFogVolume {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        fog_color = Self::default_fog_color(),
        density_factor = Self::default_density_factor(),
        density_texture = None,
        density_texture_offset = Self::default_density_texture_offset(),
        absorption = Self::default_absorption(),
        scattering = Self::default_scattering(),
        scattering_asymmetry = Self::default_scattering_asymmetry(),
        light_tint = Self::default_light_tint(),
        light_intensity = Self::default_light_intensity()
    ))]
    pub fn new(
        fog_color: PyColor,
        density_factor: f32,
        density_texture: Option<&Bound<'_, PyAny>>,
        density_texture_offset: PyVec3,
        absorption: f32,
        scattering: f32,
        scattering_asymmetry: f32,
        light_tint: PyColor,
        light_intensity: f32,
    ) -> PyResult<PyClassInitializer<Self>> {
        let texture = match density_texture {
            Some(h) => Some(extract_handle_from_any(h)?.try_into()?),
            None => None,
        };
        Ok(Self::from_owned(FogVolume {
            fog_color: fog_color.into(),
            density_factor,
            density_texture: texture,
            density_texture_offset: density_texture_offset.into(),
            absorption,
            scattering,
            scattering_asymmetry,
            light_tint: light_tint.into(),
            light_intensity,
        }).into())
    }

    #[getter]
    pub fn fog_color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.fog_color, py)
    }

    #[setter]
    pub fn set_fog_color(&mut self, value: PyColor) -> PyResult<()> {
        self.as_mut()?.fog_color = value.into();
        Ok(())
    }

    #[getter]
    pub fn density_factor(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.density_factor)
    }

    #[setter]
    pub fn set_density_factor(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.density_factor = value;
        Ok(())
    }

    #[getter]
    pub fn density_texture(&self) -> PyResult<Option<PyHandle>> {
        Ok(self.as_ref()?.density_texture.clone().map(Into::into))
    }

    #[setter]
    pub fn set_density_texture(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.as_mut()?.density_texture = match value {
            Some(h) => Some(extract_handle_from_any(h)?.try_into()?),
            None => None,
        };
        Ok(())
    }

    #[getter]
    pub fn density_texture_offset(&self) -> PyResult<PyVec3> {
        Ok(self
            .storage
            .borrow_field_as(|f| &f.density_texture_offset)?)
    }

    #[setter]
    pub fn set_density_texture_offset(&mut self, value: PyVec3) -> PyResult<()> {
        self.as_mut()?.density_texture_offset = value.into();
        Ok(())
    }

    #[getter]
    pub fn absorption(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.absorption)
    }

    #[setter]
    pub fn set_absorption(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.absorption = value;
        Ok(())
    }

    #[getter]
    pub fn scattering(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.scattering)
    }

    #[setter]
    pub fn set_scattering(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.scattering = value;
        Ok(())
    }

    #[getter]
    pub fn scattering_asymmetry(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.scattering_asymmetry)
    }

    #[setter]
    pub fn set_scattering_asymmetry(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.scattering_asymmetry = value;
        Ok(())
    }

    #[getter]
    pub fn light_tint(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.light_tint, py)
    }

    #[setter]
    pub fn set_light_tint(&mut self, value: PyColor) -> PyResult<()> {
        self.as_mut()?.light_tint = value.into();
        Ok(())
    }

    #[getter]
    pub fn light_intensity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.light_intensity)
    }

    #[setter]
    pub fn set_light_intensity(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.light_intensity = value;
        Ok(())
    }
}
