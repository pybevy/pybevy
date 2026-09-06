use bevy::{camera::ClearColor, color::Color};
use pybevy_color::color::PyColor;
use pybevy_core::{PyResource, ResourceStorage};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

#[pyresource(ClearColor, bridge)]
#[pyclass(name = "ClearColor", module = "pybevy.camera", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyClearColor {
    pub storage: ResourceStorage<ClearColor>,
}

#[pymethods]
impl PyClearColor {
    #[new]
    #[pyo3(signature = (color = ClearColor::default().0.into()))]
    pub fn new(color: PyColor) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(ClearColor(Color::try_from(color)?)))
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_resource_field(&self.storage, |clear_color| &clear_color.0, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = Color::try_from(color)?;
        self.as_mut()?.0 = color;
        Ok(())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(cc) => format!("ClearColor({:?})", cc.0),
            Err(_) => "ClearColor(<invalid>)".to_string(),
        }
    }
}
