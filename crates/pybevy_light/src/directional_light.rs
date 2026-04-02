use bevy::light::DirectionalLight;
use pybevy_color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

#[component_storage(DirectionalLight, bridge, view_fields = [
    illuminance,
    shadow_depth_bias,
    shadow_normal_bias,
    shadows_enabled,
    affects_lightmapped_mesh_diffuse
], batch_only_fields = [color])]
#[pyclass(name = "DirectionalLight", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyDirectionalLight {
    pub(crate) storage: ComponentStorage<DirectionalLight>,
}

impl PyDirectionalLight {
    fn default_color() -> PyColor {
        DirectionalLight::default().color.into()
    }

    fn default_illuminance() -> f32 {
        DirectionalLight::default().illuminance
    }

    fn default_shadows_enabled() -> bool {
        DirectionalLight::default().shadows_enabled
    }

    fn default_affects_lightmapped_mesh_diffuse() -> bool {
        DirectionalLight::default().affects_lightmapped_mesh_diffuse
    }

    fn default_shadow_depth_bias() -> f32 {
        DirectionalLight::default().shadow_depth_bias
    }

    fn default_shadow_normal_bias() -> f32 {
        DirectionalLight::default().shadow_normal_bias
    }
}

#[pymethods]
impl PyDirectionalLight {
    #[new]
    #[pyo3(signature = (
        color = Self::default_color(),
        illuminance = Self::default_illuminance(),
        shadows_enabled = Self::default_shadows_enabled(),
        affects_lightmapped_mesh_diffuse = Self::default_affects_lightmapped_mesh_diffuse(),
        shadow_depth_bias = Self::default_shadow_depth_bias(),
        shadow_normal_bias = Self::default_shadow_normal_bias()
    ))]
    pub fn new(
        color: PyColor,
        illuminance: f32,
        shadows_enabled: bool,
        affects_lightmapped_mesh_diffuse: bool,
        shadow_depth_bias: f32,
        shadow_normal_bias: f32,
    ) -> (Self, PyComponent) {
        Self::from_owned(DirectionalLight {
            color: color.into(),
            illuminance,
            shadows_enabled,
            affects_lightmapped_mesh_diffuse,
            shadow_depth_bias,
            shadow_normal_bias,
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
    pub fn illuminance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.illuminance)
    }

    #[setter]
    pub fn set_illuminance(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.illuminance = value;
        Ok(())
    }

    #[getter]
    pub fn shadows_enabled(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.shadows_enabled)
    }

    #[setter]
    pub fn set_shadows_enabled(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.shadows_enabled = value;
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
}
