use bevy::ui::{GlobalZIndex, ZIndex};
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;

#[pywrap(ZIndex, bridge)]
#[pyclass(name = "ZIndex", extends = PyComponent, eq, skip_from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyZIndex(pub(crate) ZIndex);

#[pymethods]
impl PyZIndex {
    #[new]
    #[pyo3(signature = (value = 0))]
    pub fn new(value: i32) -> PyClassInitializer<Self> {
        Self::from_owned(ZIndex(value)).into()
    }

    #[getter]
    pub fn value(&self) -> i32 {
        self.0.0
    }

    #[setter]
    pub fn set_value(&mut self, value: i32) -> PyResult<()> {
        self.0.0 = value;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        format!("ZIndex({})", self.0.0)
    }
}

#[pywrap(GlobalZIndex, bridge)]
#[pyclass(name = "GlobalZIndex", extends = PyComponent, eq, skip_from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyGlobalZIndex(pub(crate) GlobalZIndex);

#[pymethods]
impl PyGlobalZIndex {
    #[new]
    #[pyo3(signature = (value = 0))]
    pub fn new(value: i32) -> PyClassInitializer<Self> {
        Self::from_owned(GlobalZIndex(value)).into()
    }

    #[getter]
    pub fn value(&self) -> i32 {
        self.0.0
    }

    #[setter]
    pub fn set_value(&mut self, value: i32) -> PyResult<()> {
        self.0.0 = value;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        format!("GlobalZIndex({})", self.0.0)
    }
}
