use bevy::window::EnabledButtons;
use pyo3::prelude::*;

#[pyclass(name = "EnabledButtons", eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyEnabledButtons(pub EnabledButtons);

impl From<EnabledButtons> for PyEnabledButtons {
    fn from(value: EnabledButtons) -> Self {
        PyEnabledButtons(value)
    }
}

impl From<PyEnabledButtons> for EnabledButtons {
    fn from(value: PyEnabledButtons) -> Self {
        value.0
    }
}

#[pymethods]
impl PyEnabledButtons {
    #[new]
    #[pyo3(signature = (minimize = true, maximize = true, close = true))]
    pub fn new(minimize: bool, maximize: bool, close: bool) -> Self {
        PyEnabledButtons(EnabledButtons {
            minimize,
            maximize,
            close,
        })
    }

    #[getter]
    pub fn minimize(&self) -> bool {
        self.0.minimize
    }

    #[setter]
    pub fn set_minimize(&mut self, value: bool) {
        self.0.minimize = value;
    }

    #[getter]
    pub fn maximize(&self) -> bool {
        self.0.maximize
    }

    #[setter]
    pub fn set_maximize(&mut self, value: bool) {
        self.0.maximize = value;
    }

    #[getter]
    pub fn close(&self) -> bool {
        self.0.close
    }

    #[setter]
    pub fn set_close(&mut self, value: bool) {
        self.0.close = value;
    }

    pub fn __repr__(&self) -> String {
        format!(
            "EnabledButtons(minimize={}, maximize={}, close={})",
            if self.0.minimize { "True" } else { "False" },
            if self.0.maximize { "True" } else { "False" },
            if self.0.close { "True" } else { "False" }
        )
    }
}
