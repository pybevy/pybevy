use bevy::pbr::DefaultOpaqueRendererMethod;
use pybevy_core::{PyResource, ResourceStorage};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

#[pyresource(DefaultOpaqueRendererMethod, bridge, no_user_remove)]
#[pyclass(name = "DefaultOpaqueRendererMethod", module = "pybevy.pbr", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyDefaultOpaqueRendererMethod {
    pub(crate) storage: ResourceStorage<DefaultOpaqueRendererMethod>,
}

#[pymethods]
impl PyDefaultOpaqueRendererMethod {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(DefaultOpaqueRendererMethod::default())
    }

    #[staticmethod]
    pub fn forward(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(DefaultOpaqueRendererMethod::forward()))
    }

    #[staticmethod]
    pub fn deferred(py: Python) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(DefaultOpaqueRendererMethod::deferred()),
        )
    }

    pub fn set_to_forward(&mut self) -> PyResult<()> {
        self.as_mut()?.set_to_forward();
        Ok(())
    }

    pub fn set_to_deferred(&mut self) -> PyResult<()> {
        self.as_mut()?.set_to_deferred();
        Ok(())
    }
}
