use bevy::light::{DirectionalLightShadowMap, PointLightShadowMap};
use pybevy_core::{PyResource, ResourceStorage, domain::IntegerInput, resource_initializer};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

// Bevy creates the shadow textures with `size as u32` sides.
#[pyresource(
    PointLightShadowMap,
    bridge,
    no_user_remove,
    domains = [size => [at_least(1)]]
)]
#[pyclass(name = "PointLightShadowMap", module = "pybevy.light", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyPointLightShadowMap {
    pub(crate) storage: ResourceStorage<PointLightShadowMap>,
}

#[pymethods]
impl PyPointLightShadowMap {
    #[new]
    #[pyo3(signature = (*, size = IntegerInput::new(1024)))]
    pub fn new(size: IntegerInput<u32>) -> PyResult<PyClassInitializer<Self>> {
        let size = Self::check_size(size)?.stored() as usize;
        Ok(resource_initializer(PyPointLightShadowMap::from(
            PointLightShadowMap { size },
        )))
    }

    #[getter]
    pub fn size(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.size)
    }

    #[setter]
    pub fn set_size(&mut self, value: IntegerInput<u32>) -> PyResult<()> {
        self.as_mut()?.size = Self::check_size(value)?.stored() as usize;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(map) => format!("PointLightShadowMap(size={})", map.size),
            Err(_) => "PointLightShadowMap(<invalid>)".to_string(),
        }
    }
}

#[pyresource(
    DirectionalLightShadowMap,
    bridge,
    no_user_remove,
    domains = [size => [at_least(1)]]
)]
#[pyclass(name = "DirectionalLightShadowMap", module = "pybevy.light", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyDirectionalLightShadowMap {
    pub(crate) storage: ResourceStorage<DirectionalLightShadowMap>,
}

#[pymethods]
impl PyDirectionalLightShadowMap {
    #[new]
    #[pyo3(signature = (*, size = IntegerInput::new(2048)))]
    pub fn new(size: IntegerInput<u32>) -> PyResult<PyClassInitializer<Self>> {
        let size = Self::check_size(size)?.stored() as usize;
        Ok(resource_initializer(PyDirectionalLightShadowMap::from(
            DirectionalLightShadowMap { size },
        )))
    }

    #[getter]
    pub fn size(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.size)
    }

    #[setter]
    pub fn set_size(&mut self, value: IntegerInput<u32>) -> PyResult<()> {
        self.as_mut()?.size = Self::check_size(value)?.stored() as usize;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(map) => format!("DirectionalLightShadowMap(size={})", map.size),
            Err(_) => "DirectionalLightShadowMap(<invalid>)".to_string(),
        }
    }
}
