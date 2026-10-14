use bevy::{light::AtmosphereEnvironmentMapLight, math::UVec2};
use pybevy_core::{ComponentStorage, PyComponent, StorageError, public_error};
use pybevy_macros::pycomponent;
use pybevy_math::uvec2::PyUVec2;
use pyo3::prelude::*;

const MAX_ENVIRONMENT_MAP_SIZE: u32 = 8192;

// Bevy rounds each side up to a power of two, then panics on a cubemap that is not square or exceeds 8192.
fn validate_size(size: &UVec2) -> Result<(), StorageError> {
    let side = |value: u32| value.max(1).checked_next_power_of_two();
    match (side(size.x), side(size.y)) {
        (Some(width), Some(height)) if width == height && width <= MAX_ENVIRONMENT_MAP_SIZE => {
            Ok(())
        }
        _ => Err(StorageError::InvalidValue(
            public_error::atmosphere_environment_map_size(size.x, size.y),
        )),
    }
}

#[pycomponent(AtmosphereEnvironmentMapLight, bridge, no_reflect, view_fields = [intensity, affects_lightmapped_mesh_diffuse], domains = [intensity => [finite]])]
#[pyclass(name = "AtmosphereEnvironmentMapLight", module = "pybevy.light", extends = PyComponent)]
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
        *,
        intensity = Self::default_intensity(),
        affects_lightmapped_mesh_diffuse = Self::default_affects_lightmapped_mesh_diffuse(),
        size = Self::default_size()
    ))]
    pub fn new(
        intensity: f32,
        affects_lightmapped_mesh_diffuse: bool,
        size: PyUVec2,
    ) -> PyResult<PyClassInitializer<Self>> {
        let intensity = Self::check_intensity(intensity)?;
        let size: UVec2 = size.try_into()?;
        validate_size(&size)?;
        Ok(Self::from_owned(AtmosphereEnvironmentMapLight {
            intensity,
            affects_lightmapped_mesh_diffuse,
            size,
        })
        .into())
    }

    #[getter]
    pub fn intensity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.intensity)
    }

    #[setter]
    pub fn set_intensity(&mut self, intensity: f32) -> PyResult<()> {
        let intensity = Self::check_intensity(intensity)?;
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
        Ok(self
            .storage
            .borrow_validated_field_as(|e| &e.size, |e| &mut e.size, validate_size)?)
    }

    #[setter]
    pub fn set_size(&mut self, size: PyUVec2) -> PyResult<()> {
        let size: UVec2 = size.try_into()?;
        validate_size(&size)?;
        self.as_mut()?.size = size;
        Ok(())
    }
}
