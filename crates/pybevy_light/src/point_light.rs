use bevy::light::PointLight;
use pybevy_color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

#[component_storage(PointLight)]
#[pyclass(name = "PointLight", extends = PyComponent)]
#[derive(Debug, Clone)]
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

    fn default_shadows_enabled() -> bool {
        PointLight::default().shadows_enabled
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
    #[pyo3(signature = (
        color = Self::default_color(),
        intensity = Self::default_intensity(),
        range = Self::default_range(),
        radius = Self::default_radius(),
        shadows_enabled = Self::default_shadows_enabled(),
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
        shadows_enabled: bool,
        affects_lightmapped_mesh_diffuse: bool,
        shadow_depth_bias: f32,
        shadow_normal_bias: f32,
        shadow_map_near_z: f32,
    ) -> (Self, PyComponent) {
        Self::from_owned(PointLight {
            color: color.into(),
            intensity,
            range,
            radius,
            shadows_enabled,
            affects_lightmapped_mesh_diffuse,
            shadow_depth_bias,
            shadow_normal_bias,
            shadow_map_near_z,
        })
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.color = color.into();
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
    pub fn shadows_enabled(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.shadows_enabled)
    }

    #[setter]
    pub fn set_shadows_enabled(&mut self, shadows_enabled: bool) -> PyResult<()> {
        self.as_mut()?.shadows_enabled = shadows_enabled;
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
