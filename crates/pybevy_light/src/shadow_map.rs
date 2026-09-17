use bevy::light::{DirectionalLightShadowMap, PointLightShadowMap};
use pybevy_core::{PyResource, ResourceStorage, resource_initializer};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

#[pyresource(PointLightShadowMap, bridge)]
#[pyclass(name = "PointLightShadowMap", module = "pybevy.light", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyPointLightShadowMap {
    pub(crate) storage: ResourceStorage<PointLightShadowMap>,
}

#[pymethods]
impl PyPointLightShadowMap {
    #[new]
    #[pyo3(signature = (*, size = 1024))]
    pub fn new(size: usize) -> PyClassInitializer<Self> {
        resource_initializer(PyPointLightShadowMap::from(PointLightShadowMap { size }))
    }

    #[getter]
    pub fn size(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.size)
    }

    #[setter]
    pub fn set_size(&mut self, value: usize) -> PyResult<()> {
        self.as_mut()?.size = value;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(map) => format!("PointLightShadowMap(size={})", map.size),
            Err(_) => "PointLightShadowMap(<invalid>)".to_string(),
        }
    }
}

#[pyresource(DirectionalLightShadowMap, bridge)]
#[pyclass(name = "DirectionalLightShadowMap", module = "pybevy.light", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyDirectionalLightShadowMap {
    pub(crate) storage: ResourceStorage<DirectionalLightShadowMap>,
}

#[pymethods]
impl PyDirectionalLightShadowMap {
    #[new]
    #[pyo3(signature = (*, size = 2048))]
    pub fn new(size: usize) -> PyClassInitializer<Self> {
        resource_initializer(PyDirectionalLightShadowMap::from(
            DirectionalLightShadowMap { size },
        ))
    }

    #[getter]
    pub fn size(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.size)
    }

    #[setter]
    pub fn set_size(&mut self, value: usize) -> PyResult<()> {
        self.as_mut()?.size = value;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(map) => format!("DirectionalLightShadowMap(size={})", map.size),
            Err(_) => "DirectionalLightShadowMap(<invalid>)".to_string(),
        }
    }
}
