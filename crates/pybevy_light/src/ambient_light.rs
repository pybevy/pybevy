use std::ptr;

use bevy::{
    ecs::resource::Resource,
    light::{AmbientLight, GlobalAmbientLight},
};
use pybevy_color::color::PyColor;
use pybevy_core::{
    ComponentStorage, PyComponent, PyResource, ResourceStorage, ResourceStorageInner,
};
use pybevy_macros::{pycomponent, pyresource};
use pyo3::prelude::*;

#[pyresource(GlobalAmbientLight, bridge)]
#[pyclass(name = "GlobalAmbientLight", extends = PyResource, eq, from_py_object)]
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
            (ResourceStorageInner::BorrowedRef(a), ResourceStorageInner::BorrowedRef(b)) => {
                ptr::eq(a.as_ptr(), b.as_ptr())
            }
            (ResourceStorageInner::BorrowedMut(a), ResourceStorageInner::BorrowedMut(b)) => {
                ptr::eq(a.as_ptr(), b.as_ptr())
            }
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
    ) -> PyClassInitializer<Self> {
        (
            GlobalAmbientLight {
                color: color.into(),
                brightness,
                affects_lightmapped_meshes,
            }
            .into(),
            PyResource,
        )
            .into()
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

#[pycomponent(AmbientLight, bridge, view_fields = [brightness, affects_lightmapped_meshes], batch_only_fields = [color])]
#[pyclass(name = "AmbientLight", extends = PyComponent, eq)]
#[derive(Debug)]
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
    ) -> PyClassInitializer<Self> {
        Self::from_owned(AmbientLight {
            color: color.into(),
            brightness,
            affects_lightmapped_meshes,
        })
        .into()
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
