use bevy::{
    color::Color,
    ecs::resource::Resource,
    light::{AmbientLight, GlobalAmbientLight},
};
use pybevy_color::color::PyColor;
use pybevy_core::{
    ComponentStorage, PyComponent, PyResource, ResourceStorage, resource_initializer,
};
use pybevy_macros::{pycomponent, pyresource};
use pyo3::prelude::*;

#[pyresource(GlobalAmbientLight, bridge)]
#[pyclass(name = "GlobalAmbientLight", module = "pybevy.light", extends = PyResource, from_py_object)]
#[derive(Debug, Resource)]
pub struct PyGlobalAmbientLight {
    // TODO: make to crate-pub only. sweep for similar pub's
    pub storage: ResourceStorage<GlobalAmbientLight>,
}

#[pymethods]
impl PyGlobalAmbientLight {
    #[new]
    #[pyo3(signature = (
        color = bevy::color::Color::WHITE.into(),
        brightness = 80.0,
        affects_lightmapped_meshes = true
    ))]
    pub fn new(
        color: PyColor,
        brightness: f32,
        affects_lightmapped_meshes: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        let color = Color::try_from(color)?;
        Ok(resource_initializer(
            GlobalAmbientLight {
                color,
                brightness,
                affects_lightmapped_meshes,
            }
            .into(),
        ))
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_resource_field(&self.storage, |light| &light.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = Color::try_from(color)?;
        self.as_mut()?.color = color;
        Ok(())
    }

    #[getter]
    pub fn brightness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.brightness)
    }

    #[setter]
    pub fn set_brightness(&mut self, brightness: f32) -> PyResult<()> {
        self.as_mut()?.brightness = brightness;
        Ok(())
    }

    #[getter]
    pub fn affects_lightmapped_meshes(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.affects_lightmapped_meshes)
    }

    #[setter]
    pub fn set_affects_lightmapped_meshes(&mut self, affects: bool) -> PyResult<()> {
        self.as_mut()?.affects_lightmapped_meshes = affects;
        Ok(())
    }
}

#[pycomponent(AmbientLight, bridge, view_fields = [brightness, affects_lightmapped_meshes], batch_only_fields = [color])]
#[pyclass(name = "AmbientLight", module = "pybevy.light", extends = PyComponent)]
#[derive(Debug)]
pub struct PyAmbientLight {
    pub(crate) storage: ComponentStorage<AmbientLight>,
}

#[pymethods]
impl PyAmbientLight {
    #[new]
    #[pyo3(signature = (
        color = bevy::color::Color::WHITE.into(),
        brightness = 80.0,
        affects_lightmapped_meshes = true
    ))]
    pub fn new(
        color: PyColor,
        brightness: f32,
        affects_lightmapped_meshes: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        let color = Color::try_from(color)?;
        Ok(Self::from_owned(AmbientLight {
            color,
            brightness,
            affects_lightmapped_meshes,
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
    pub fn brightness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.brightness)
    }

    #[setter]
    pub fn set_brightness(&mut self, brightness: f32) -> PyResult<()> {
        self.as_mut()?.brightness = brightness;
        Ok(())
    }

    #[getter]
    pub fn affects_lightmapped_meshes(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.affects_lightmapped_meshes)
    }

    #[setter]
    pub fn set_affects_lightmapped_meshes(&mut self, affects: bool) -> PyResult<()> {
        self.as_mut()?.affects_lightmapped_meshes = affects;
        Ok(())
    }
}
