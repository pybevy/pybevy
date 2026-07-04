use std::time::Duration;

use bevy::winit::UpdateMode;
use pyo3::prelude::*;
#[pyclass(name = "UpdateMode", frozen, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyUpdateMode(pub(crate) UpdateMode);

#[pymethods]
impl PyUpdateMode {
    #[staticmethod]
    pub fn continuous() -> Self {
        PyUpdateMode(UpdateMode::Continuous)
    }

    #[staticmethod]
    #[pyo3(signature = (wait = 1.0))]
    pub fn reactive(wait: f64) -> Self {
        PyUpdateMode(UpdateMode::reactive(Duration::from_secs_f64(wait)))
    }

    #[staticmethod]
    #[pyo3(signature = (wait = 1.0))]
    pub fn reactive_low_power(wait: f64) -> Self {
        PyUpdateMode(UpdateMode::reactive_low_power(Duration::from_secs_f64(
            wait,
        )))
    }

    pub fn __repr__(&self) -> String {
        match &self.0 {
            UpdateMode::Continuous => "UpdateMode.continuous()".to_string(),
            UpdateMode::Reactive {
                wait,
                react_to_device_events,
                ..
            } => {
                let secs = wait.as_secs_f64();
                if *react_to_device_events {
                    format!("UpdateMode.reactive(wait={secs})")
                } else {
                    format!("UpdateMode.reactive_low_power(wait={secs})")
                }
            }
        }
    }
}

impl From<PyUpdateMode> for UpdateMode {
    fn from(val: PyUpdateMode) -> Self {
        val.0
    }
}

impl From<UpdateMode> for PyUpdateMode {
    fn from(val: UpdateMode) -> Self {
        PyUpdateMode(val)
    }
}
