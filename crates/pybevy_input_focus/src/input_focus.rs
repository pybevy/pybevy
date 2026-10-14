use bevy::input_focus::{InputFocus, InputFocusVisible};
use pybevy_core::{PyEntity, PyResource, ResourceStorage, resource_initializer};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

use crate::focus_cause::PyFocusCause;

#[pyresource(InputFocus, bridge, no_user_remove)]
#[pyclass(name = "InputFocus", module = "pybevy.input_focus", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyInputFocus {
    pub(crate) storage: ResourceStorage<InputFocus>,
}

#[pymethods]
impl PyInputFocus {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        resource_initializer(Self {
            storage: ResourceStorage::owned(InputFocus::default()),
        })
    }

    #[staticmethod]
    pub fn from_entity(py: Python<'_>, entity: PyEntity) -> PyResult<Py<Self>> {
        Py::new(
            py,
            resource_initializer(Self {
                storage: ResourceStorage::owned(InputFocus::from_entity(entity.0)),
            }),
        )
    }

    pub fn get(&self) -> PyResult<Option<PyEntity>> {
        Ok(self.as_ref()?.get().map(PyEntity))
    }

    pub fn set(&mut self, entity: PyEntity, cause: PyFocusCause) -> PyResult<()> {
        self.as_mut()?.set(entity.0, cause.into());
        Ok(())
    }

    pub fn clear(&mut self) -> PyResult<()> {
        self.as_mut()?.clear();
        Ok(())
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(match self.as_ref()?.get() {
            Some(entity) => format!("InputFocus({entity})"),
            None => "InputFocus(None)".to_string(),
        })
    }
}

#[pyresource(InputFocusVisible, bridge, no_user_remove)]
#[pyclass(name = "InputFocusVisible", module = "pybevy.input_focus", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyInputFocusVisible {
    pub(crate) storage: ResourceStorage<InputFocusVisible>,
}

#[pymethods]
impl PyInputFocusVisible {
    #[new]
    #[pyo3(signature = (value = false))]
    pub fn new(value: bool) -> PyClassInitializer<Self> {
        resource_initializer(Self {
            storage: ResourceStorage::owned(InputFocusVisible(value)),
        })
    }

    #[getter]
    pub fn value(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0)
    }

    #[setter]
    pub fn set_value(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.0 = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "InputFocusVisible({})",
            if self.as_ref()?.0 { "True" } else { "False" }
        ))
    }
}
