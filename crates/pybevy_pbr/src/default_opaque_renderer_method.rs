use bevy::{material::OpaqueRendererMethod, pbr::DefaultOpaqueRendererMethod};
use pybevy_core::{PyResource, ResourceStorage};
use pybevy_macros::pyresource;
use pybevy_material::opaque_renderer_method::PyOpaqueRendererMethod;
use pyo3::prelude::*;

#[pyresource(DefaultOpaqueRendererMethod, bridge)]
#[pyclass(name = "DefaultOpaqueRendererMethod", extends = PyResource)]
#[derive(Debug)]
pub struct PyDefaultOpaqueRendererMethod {
    pub storage: ResourceStorage<DefaultOpaqueRendererMethod>,
}

#[pymethods]
impl PyDefaultOpaqueRendererMethod {
    #[new]
    #[pyo3(signature = (method = PyOpaqueRendererMethod::Forward))]
    pub fn new(method: PyOpaqueRendererMethod) -> (Self, PyResource) {
        let bevy_method: OpaqueRendererMethod = method.into();
        let resource = match bevy_method {
            OpaqueRendererMethod::Forward => DefaultOpaqueRendererMethod::forward(),
            OpaqueRendererMethod::Deferred => DefaultOpaqueRendererMethod::deferred(),
            OpaqueRendererMethod::Auto => DefaultOpaqueRendererMethod::forward(),
        };
        Self::from_owned(resource)
    }

    #[staticmethod]
    pub fn forward(py: Python) -> PyResult<Py<Self>> {
        let (obj, base) = Self::from_owned(DefaultOpaqueRendererMethod::forward());
        Py::new(py, (obj, base))
    }

    #[staticmethod]
    pub fn deferred(py: Python) -> PyResult<Py<Self>> {
        let (obj, base) = Self::from_owned(DefaultOpaqueRendererMethod::deferred());
        Py::new(py, (obj, base))
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
