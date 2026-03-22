use std::time::Duration;

use bevy::winit::UpdateMode;
use pyo3::prelude::*;
#[pyclass(name = "UpdateMode", frozen)]
#[derive(Debug, Clone)]
pub struct PyUpdateMode(pub(crate) UpdateMode);

#[pymethods]
impl PyUpdateMode {
    /// Update as fast as possible until an AppExit event occurs.
    #[staticmethod]
    pub fn continuous() -> Self {
        PyUpdateMode(UpdateMode::Continuous)
    }

    /// Reactive mode - updates in response to events or after `wait` seconds.
    ///
    /// Reacts to all event types (window, device, and user events).
    ///
    #[staticmethod]
    #[pyo3(signature = (wait = 1.0))]
    pub fn reactive(wait: f64) -> Self {
        PyUpdateMode(UpdateMode::reactive(Duration::from_secs_f64(wait)))
    }

    /// Low power reactive mode - only reacts to window and user events.
    ///
    /// Unlike `reactive()`, this ignores device events like general mouse movement
    /// (only reacts when the cursor is over a window). This can greatly reduce
    /// power consumption.
    ///
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
