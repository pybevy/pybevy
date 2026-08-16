use bevy::{color::Color, light::PointLight};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(PointLight, bridge, view_fields = [
    intensity,
    range,
    radius,
    shadow_depth_bias,
    shadow_normal_bias,
    shadow_map_near_z,
    shadow_maps_enabled,
    contact_shadows_enabled,
    affects_lightmapped_mesh_diffuse
], batch_only_fields = [color])]
#[pyclass(name = "PointLight", extends = PyComponent)]
#[derive(Debug)]
pub struct PyPointLight {
    pub(crate) storage: ComponentStorage<PointLight>,
}

impl PyPointLight {
    fn default_color() -> PyColor {
        PointLight::default().color.into()
    }

    fn default_intensity() -> f32 {
        PointLight::default().intensity
    }

    fn default_range() -> f32 {
        PointLight::default().range
    }

    fn default_radius() -> f32 {
        PointLight::default().radius
    }

    fn default_shadow_maps_enabled() -> bool {
        PointLight::default().shadow_maps_enabled
    }

    fn default_contact_shadows_enabled() -> bool {
        PointLight::default().contact_shadows_enabled
    }

    fn default_affects_lightmapped_mesh_diffuse() -> bool {
        PointLight::default().affects_lightmapped_mesh_diffuse
    }

    fn default_shadow_depth_bias() -> f32 {
        PointLight::default().shadow_depth_bias
    }

    fn default_shadow_normal_bias() -> f32 {
        PointLight::default().shadow_normal_bias
    }

    fn default_shadow_map_near_z() -> f32 {
        PointLight::default().shadow_map_near_z
    }
}

#[pymethods]
impl PyPointLight {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        color = Self::default_color(),
        intensity = Self::default_intensity(),
        range = Self::default_range(),
        radius = Self::default_radius(),
        shadow_maps_enabled = Self::default_shadow_maps_enabled(),
        contact_shadows_enabled = Self::default_contact_shadows_enabled(),
        affects_lightmapped_mesh_diffuse = Self::default_affects_lightmapped_mesh_diffuse(),
        shadow_depth_bias = Self::default_shadow_depth_bias(),
        shadow_normal_bias = Self::default_shadow_normal_bias(),
        shadow_map_near_z = Self::default_shadow_map_near_z()
    ))]
    pub fn new(
        color: PyColor,
        intensity: f32,
        range: f32,
        radius: f32,
        shadow_maps_enabled: bool,
        contact_shadows_enabled: bool,
        affects_lightmapped_mesh_diffuse: bool,
        shadow_depth_bias: f32,
        shadow_normal_bias: f32,
        shadow_map_near_z: f32,
    ) -> PyResult<PyClassInitializer<Self>> {
        let color = Color::try_from(color)?;
        Ok(Self::from_owned(PointLight {
            color,
            intensity,
            range,
            radius,
            shadow_maps_enabled,
            contact_shadows_enabled,
            affects_lightmapped_mesh_diffuse,
            shadow_depth_bias,
            shadow_normal_bias,
            shadow_map_near_z,
        })
        .into())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |light| &light.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = Color::try_from(color)?;
        self.as_mut()?.color = color;
        Ok(())
    }

    #[getter]
    pub fn intensity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.intensity)
    }

    #[setter]
    pub fn set_intensity(&mut self, intensity: f32) -> PyResult<()> {
        self.as_mut()?.intensity = intensity;
        Ok(())
    }

    #[getter]
    pub fn range(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.range)
    }

    #[setter]
    pub fn set_range(&mut self, range: f32) -> PyResult<()> {
        self.as_mut()?.range = range;
        Ok(())
    }

    #[getter]
    pub fn radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.radius)
    }

    #[setter]
    pub fn set_radius(&mut self, radius: f32) -> PyResult<()> {
        self.as_mut()?.radius = radius;
        Ok(())
    }

    #[getter]
    pub fn shadow_maps_enabled(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.shadow_maps_enabled)
    }

    #[setter]
    pub fn set_shadow_maps_enabled(&mut self, shadow_maps_enabled: bool) -> PyResult<()> {
        self.as_mut()?.shadow_maps_enabled = shadow_maps_enabled;
        Ok(())
    }

    #[getter]
    pub fn contact_shadows_enabled(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.contact_shadows_enabled)
    }

    #[setter]
    pub fn set_contact_shadows_enabled(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.contact_shadows_enabled = value;
        Ok(())
    }

    #[getter]
    pub fn affects_lightmapped_mesh_diffuse(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.affects_lightmapped_mesh_diffuse)
    }

    #[setter]
    pub fn set_affects_lightmapped_mesh_diffuse(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.affects_lightmapped_mesh_diffuse = value;
        Ok(())
    }

    #[getter]
    pub fn shadow_depth_bias(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.shadow_depth_bias)
    }

    #[setter]
    pub fn set_shadow_depth_bias(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.shadow_depth_bias = value;
        Ok(())
    }

    #[getter]
    pub fn shadow_normal_bias(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.shadow_normal_bias)
    }

    #[setter]
    pub fn set_shadow_normal_bias(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.shadow_normal_bias = value;
        Ok(())
    }

    #[getter]
    pub fn shadow_map_near_z(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.shadow_map_near_z)
    }

    #[setter]
    pub fn set_shadow_map_near_z(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.shadow_map_near_z = value;
        Ok(())
    }
}
