use bevy::{asset::Handle, light::EnvironmentMapLight};
use pybevy_core::{ComponentStorage, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pycomponent;
use pybevy_math::quat::PyQuat;
use pyo3::prelude::*;

#[pycomponent(EnvironmentMapLight, bridge, view_fields = [intensity, affects_lightmapped_mesh_diffuse])]
#[pyclass(name = "EnvironmentMapLight", extends = PyComponent)]
#[derive(Clone)]
pub struct PyEnvironmentMapLight {
    pub(crate) storage: ComponentStorage<EnvironmentMapLight>,
}

impl PyEnvironmentMapLight {
    fn default_intensity() -> f32 {
        EnvironmentMapLight::default().intensity
    }

    fn default_rotation() -> PyQuat {
        EnvironmentMapLight::default().rotation.into()
    }

    fn default_affects_lightmapped_mesh_diffuse() -> bool {
        EnvironmentMapLight::default().affects_lightmapped_mesh_diffuse
    }
}

#[pymethods]
impl PyEnvironmentMapLight {
    #[new]
    #[pyo3(signature = (
        diffuse_map = None,
        specular_map = None,
        intensity = Self::default_intensity(),
        rotation = Self::default_rotation(),
        affects_lightmapped_mesh_diffuse = Self::default_affects_lightmapped_mesh_diffuse()
    ))]
    pub fn new(
        diffuse_map: Option<&Bound<'_, PyAny>>,
        specular_map: Option<&Bound<'_, PyAny>>,
        intensity: f32,
        rotation: PyQuat,
        affects_lightmapped_mesh_diffuse: bool,
    ) -> PyResult<(Self, PyComponent)> {
        let diffuse = match diffuse_map {
            Some(h) => extract_handle_from_any(h)?.try_into()?,
            None => Handle::default(),
        };
        let specular = match specular_map {
            Some(h) => extract_handle_from_any(h)?.try_into()?,
            None => Handle::default(),
        };
        Ok(Self::from_owned(EnvironmentMapLight {
            diffuse_map: diffuse,
            specular_map: specular,
            intensity,
            rotation: rotation.into(),
            affects_lightmapped_mesh_diffuse,
        }))
    }

    #[getter]
    pub fn diffuse_map(&self) -> PyResult<PyHandle> {
        Ok(self.as_ref()?.diffuse_map.clone().into())
    }

    #[setter]
    pub fn set_diffuse_map(&mut self, diffuse_map: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.diffuse_map = extract_handle_from_any(diffuse_map)?.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn specular_map(&self) -> PyResult<PyHandle> {
        Ok(self.as_ref()?.specular_map.clone().into())
    }

    #[setter]
    pub fn set_specular_map(&mut self, specular_map: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.specular_map = extract_handle_from_any(specular_map)?.try_into()?;
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
    pub fn rotation(&self) -> PyResult<PyQuat> {
        Ok(self.storage.borrow_field_as(|e| &e.rotation)?)
    }

    #[setter]
    pub fn set_rotation(&mut self, rotation: PyQuat) -> PyResult<()> {
        self.as_mut()?.rotation = rotation.into();
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
}
