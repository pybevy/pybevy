use bevy::window::EnabledButtons;
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

#[pyvalue]
#[pyclass(name = "EnabledButtons", eq, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyEnabledButtons {
    pub(crate) storage: ValueStorage<EnabledButtons>,
}

impl PartialEq for PyEnabledButtons {
    fn eq(&self, other: &Self) -> bool {
        match (self.to_bevy(), other.to_bevy()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl From<EnabledButtons> for PyEnabledButtons {
    fn from(value: EnabledButtons) -> Self {
        PyEnabledButtons::from_owned(value)
    }
}

impl TryFrom<PyEnabledButtons> for EnabledButtons {
    type Error = PyErr;

    fn try_from(value: PyEnabledButtons) -> PyResult<Self> {
        Ok(value.storage.get()?)
    }
}

impl TryFrom<&PyEnabledButtons> for EnabledButtons {
    type Error = PyErr;

    fn try_from(value: &PyEnabledButtons) -> PyResult<Self> {
        Ok(value.storage.get()?)
    }
}

#[pymethods]
impl PyEnabledButtons {
    #[new]
    #[pyo3(signature = (minimize = true, maximize = true, close = true))]
    pub fn new(minimize: bool, maximize: bool, close: bool) -> Self {
        PyEnabledButtons::from_owned(EnabledButtons {
            minimize,
            maximize,
            close,
        })
    }

    #[getter]
    pub fn minimize(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.minimize)
    }

    #[setter]
    pub fn set_minimize(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.minimize = value;
        Ok(())
    }

    #[getter]
    pub fn maximize(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.maximize)
    }

    #[setter]
    pub fn set_maximize(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.maximize = value;
        Ok(())
    }

    #[getter]
    pub fn close(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.close)
    }

    #[setter]
    pub fn set_close(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.close = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let buttons = self.to_bevy()?;
        Ok(format!(
            "EnabledButtons(minimize={}, maximize={}, close={})",
            if buttons.minimize { "True" } else { "False" },
            if buttons.maximize { "True" } else { "False" },
            if buttons.close { "True" } else { "False" }
        ))
    }
}
