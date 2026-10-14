use bevy::light::GeneratedEnvironmentMapLight;
use pybevy_core::{ComponentStorage, HandleArg, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pycomponent;
use pybevy_math::quat::PyQuat;
use pyo3::prelude::*;

#[pycomponent(GeneratedEnvironmentMapLight, bridge, view_fields = [intensity, affects_lightmapped_mesh_diffuse], domains = [intensity => [finite]])]
#[pyclass(name = "GeneratedEnvironmentMapLight", module = "pybevy.light", extends = PyComponent)]
pub struct PyGeneratedEnvironmentMapLight {
    pub(crate) storage: ComponentStorage<GeneratedEnvironmentMapLight>,
}

impl PyGeneratedEnvironmentMapLight {
    fn default_intensity() -> f32 {
        GeneratedEnvironmentMapLight::default().intensity
    }

    fn default_rotation() -> PyQuat {
        GeneratedEnvironmentMapLight::default().rotation.into()
    }

    fn default_affects_lightmapped_mesh_diffuse() -> bool {
        GeneratedEnvironmentMapLight::default().affects_lightmapped_mesh_diffuse
    }
}

#[pymethods]
impl PyGeneratedEnvironmentMapLight {
    #[new]
    #[pyo3(signature = (
        *,
        environment_map = HandleArg::default(),
        intensity = Self::default_intensity(),
        rotation = Self::default_rotation(),
        affects_lightmapped_mesh_diffuse = Self::default_affects_lightmapped_mesh_diffuse()
    ))]
    pub fn new(
        environment_map: HandleArg,
        intensity: f32,
        rotation: PyQuat,
        affects_lightmapped_mesh_diffuse: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        let intensity = Self::check_intensity(intensity)?;
        Ok(Self::from_owned(GeneratedEnvironmentMapLight {
            environment_map: environment_map.into_handle()?,
            intensity,
            rotation: rotation.try_into()?,
            affects_lightmapped_mesh_diffuse,
        })
        .into())
    }

    #[getter]
    pub fn environment_map(&self) -> PyResult<PyHandle> {
        Ok(self.as_ref()?.environment_map.clone().into())
    }

    #[setter]
    pub fn set_environment_map(&mut self, environment_map: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.environment_map = extract_handle_from_any(environment_map)?.try_into()?;
        Ok(())
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
    pub fn rotation(&self) -> PyResult<PyQuat> {
        Ok(self.storage.borrow_field_as(|e| &e.rotation)?)
    }

    #[setter]
    pub fn set_rotation(&mut self, rotation: PyQuat) -> PyResult<()> {
        self.as_mut()?.rotation = rotation.try_into()?;
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
