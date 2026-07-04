use bevy::light::AtmosphereEnvironmentMapLight;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::uvec2::PyUVec2;
use pyo3::prelude::*;

#[pycomponent(AtmosphereEnvironmentMapLight, bridge, view_fields = [intensity, affects_lightmapped_mesh_diffuse])]
#[pyclass(name = "AtmosphereEnvironmentMapLight", extends = PyComponent)]
pub struct PyAtmosphereEnvironmentMapLight {
    pub(crate) storage: ComponentStorage<AtmosphereEnvironmentMapLight>,
}

impl PyAtmosphereEnvironmentMapLight {
    fn default_intensity() -> f32 {
        AtmosphereEnvironmentMapLight::default().intensity
    }

    fn default_affects_lightmapped_mesh_diffuse() -> bool {
        AtmosphereEnvironmentMapLight::default().affects_lightmapped_mesh_diffuse
    }

    fn default_size() -> PyUVec2 {
        AtmosphereEnvironmentMapLight::default().size.into()
    }
}

#[pymethods]
impl PyAtmosphereEnvironmentMapLight {
    #[new]
    #[pyo3(signature = (
        intensity = Self::default_intensity(),
        affects_lightmapped_mesh_diffuse = Self::default_affects_lightmapped_mesh_diffuse(),
        size = Self::default_size()
    ))]
    pub fn new(
        intensity: f32,
        affects_lightmapped_mesh_diffuse: bool,
        size: PyUVec2,
    ) -> PyClassInitializer<Self> {
        Self::from_owned(AtmosphereEnvironmentMapLight {
            intensity,
            affects_lightmapped_mesh_diffuse,
            size: size.into(),
        })
        .into()
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
    pub fn affects_lightmapped_mesh_diffuse(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.affects_lightmapped_mesh_diffuse)
    }

    #[setter]
    pub fn set_affects_lightmapped_mesh_diffuse(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.affects_lightmapped_mesh_diffuse = value;
        Ok(())
    }

    #[getter]
    pub fn size(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.borrow_field_as(|e| &e.size)?)
    }

    #[setter]
    pub fn set_size(&mut self, size: PyUVec2) -> PyResult<()> {
        self.as_mut()?.size = size.into();
        Ok(())
    }
}
