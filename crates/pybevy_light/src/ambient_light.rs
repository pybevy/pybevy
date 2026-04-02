use bevy::{
    ecs::resource::Resource,
    light::{AmbientLight, GlobalAmbientLight},
};
use pybevy_color::PyColor;
use pybevy_core::{
    ComponentStorage, PyComponent, PyResource, ResourceStorage, ResourceStorageInner,
};
use pybevy_macros::{component_storage, resource_storage};
use pyo3::prelude::*;

#[resource_storage(GlobalAmbientLight, bridge)]
#[pyclass(name = "GlobalAmbientLight", extends = PyResource, eq)]
#[derive(Debug, Resource)]
pub struct PyGlobalAmbientLight {
    pub storage: ResourceStorage<GlobalAmbientLight>,
}

impl PartialEq for PyGlobalAmbientLight {
    fn eq(&self, other: &Self) -> bool {
        match (&self.storage.inner, &other.storage.inner) {
            (
                ResourceStorageInner::Owned { data: a, .. },
                ResourceStorageInner::Owned { data: b, .. },
            ) => {
                a.color == b.color
                    && a.brightness == b.brightness
                    && a.affects_lightmapped_meshes == b.affects_lightmapped_meshes
            }
            (
                ResourceStorageInner::Borrowed { ptr: a, .. },
                ResourceStorageInner::Borrowed { ptr: b, .. },
            ) => std::ptr::eq(
                *a as *const GlobalAmbientLight,
                *b as *const GlobalAmbientLight,
            ),
            _ => false,
        }
    }
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
    ) -> (Self, PyResource) {
        (
            GlobalAmbientLight {
                color: color.into(),
                brightness,
                affects_lightmapped_meshes,
            }
            .into(),
            PyResource,
        )
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

#[component_storage(AmbientLight, bridge, view_fields = [brightness, affects_lightmapped_meshes], batch_only_fields = [color])]
#[pyclass(name = "AmbientLight", extends = PyComponent, eq)]
#[derive(Debug, Clone)]
pub struct PyAmbientLight {
    pub(crate) storage: ComponentStorage<AmbientLight>,
}

impl PartialEq for PyAmbientLight {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => {
                a.color == b.color
                    && a.brightness == b.brightness
                    && a.affects_lightmapped_meshes == b.affects_lightmapped_meshes
            }
            _ => false,
        }
    }
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
    ) -> (Self, PyComponent) {
        Self::from_owned(AmbientLight {
            color: color.into(),
            brightness,
            affects_lightmapped_meshes,
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
