use bevy::ui::{GlobalZIndex, ZIndex};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(ZIndex, bridge)]
#[pyclass(name = "ZIndex", module = "pybevy.ui", extends = PyComponent, skip_from_py_object)]
#[derive(Debug)]
pub struct PyZIndex {
    pub(crate) storage: ComponentStorage<ZIndex>,
}

#[pymethods]
impl PyZIndex {
    #[new]
    #[pyo3(signature = (value = 0))]
    pub fn new(value: i32) -> PyClassInitializer<Self> {
        Self::from_owned(ZIndex(value)).into()
    }

    #[getter]
    pub fn value(&self) -> PyResult<i32> {
        Ok(self.as_ref()?.0)
    }

    #[setter]
    pub fn set_value(&mut self, value: i32) -> PyResult<()> {
        self.as_mut()?.0 = value;
        Ok(())
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("ZIndex({})", self.as_ref()?.0))
    }
}

#[pycomponent(GlobalZIndex, bridge)]
#[pyclass(name = "GlobalZIndex", module = "pybevy.ui", extends = PyComponent, skip_from_py_object)]
#[derive(Debug)]
pub struct PyGlobalZIndex {
    pub(crate) storage: ComponentStorage<GlobalZIndex>,
}

#[pymethods]
impl PyGlobalZIndex {
    #[new]
    #[pyo3(signature = (value = 0))]
    pub fn new(value: i32) -> PyClassInitializer<Self> {
        Self::from_owned(GlobalZIndex(value)).into()
    }

    #[getter]
    pub fn value(&self) -> PyResult<i32> {
        Ok(self.as_ref()?.0)
    }

    #[setter]
    pub fn set_value(&mut self, value: i32) -> PyResult<()> {
        self.as_mut()?.0 = value;
        Ok(())
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("GlobalZIndex({})", self.as_ref()?.0))
    }
}
