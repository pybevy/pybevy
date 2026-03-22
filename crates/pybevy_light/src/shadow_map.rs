use bevy::light::{DirectionalLightShadowMap, PointLightShadowMap};
use pybevy_core::{PyResource, ResourceStorage};
use pybevy_macros::resource_storage;
use pyo3::prelude::*;

#[resource_storage(PointLightShadowMap)]
#[pyclass(name = "PointLightShadowMap", extends = PyResource)]
#[derive(Debug)]
pub struct PyPointLightShadowMap {
    pub storage: ResourceStorage<PointLightShadowMap>,
}

#[pymethods]
impl PyPointLightShadowMap {
    #[new]
    #[pyo3(signature = (size = 1024))]
    pub fn new(size: usize) -> (Self, PyResource) {
        (
            PyPointLightShadowMap::from(PointLightShadowMap { size }),
            PyResource,
        )
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
        format!("{:?}", self)
    }
}

#[resource_storage(DirectionalLightShadowMap)]
#[pyclass(name = "DirectionalLightShadowMap", extends = PyResource)]
#[derive(Debug)]
pub struct PyDirectionalLightShadowMap {
    pub storage: ResourceStorage<DirectionalLightShadowMap>,
}

#[pymethods]
impl PyDirectionalLightShadowMap {
    #[new]
    #[pyo3(signature = (size = 2048))]
    pub fn new(size: usize) -> (Self, PyResource) {
        (
            PyDirectionalLightShadowMap::from(DirectionalLightShadowMap { size }),
            PyResource,
        )
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
        format!("{:?}", self)
    }
}
