use std::time::Duration;

use bevy::winit::UpdateMode;
use pyo3::prelude::*;

#[pyclass(
    name = "UpdateMode",
    module = "pybevy.winit",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyUpdateMode {
    Continuous(),
    Reactive {
        wait: f64,
        react_to_device_events: bool,
        react_to_user_events: bool,
        react_to_window_events: bool,
    },
}

#[pymethods]
impl PyUpdateMode {
    #[staticmethod]
    #[pyo3(signature = (wait = 1.0))]
    pub fn reactive(wait: f64) -> Self {
        UpdateMode::reactive(Duration::from_secs_f64(wait)).into()
    }

    #[staticmethod]
    #[pyo3(signature = (wait = 1.0))]
    pub fn reactive_low_power(wait: f64) -> Self {
        UpdateMode::reactive_low_power(Duration::from_secs_f64(wait)).into()
    }

    pub fn __repr__(&self) -> String {
        match self {
            Self::Continuous() => "UpdateMode.Continuous()".to_string(),
            Self::Reactive {
                wait,
                react_to_device_events,
                react_to_user_events,
                react_to_window_events,
            } => {
                format!(
                    "UpdateMode.Reactive(wait={wait}, react_to_device_events={react_to_device_events}, react_to_user_events={react_to_user_events}, react_to_window_events={react_to_window_events})"
                )
            }
        }
    }
}

impl From<PyUpdateMode> for UpdateMode {
    fn from(val: PyUpdateMode) -> Self {
        match val {
            PyUpdateMode::Continuous() => UpdateMode::Continuous,
            PyUpdateMode::Reactive {
                wait,
                react_to_device_events,
                react_to_user_events,
                react_to_window_events,
            } => UpdateMode::Reactive {
                wait: Duration::from_secs_f64(wait),
                react_to_device_events,
                react_to_user_events,
                react_to_window_events,
            },
        }
    }
}

impl From<UpdateMode> for PyUpdateMode {
    fn from(val: UpdateMode) -> Self {
        match val {
            UpdateMode::Continuous => PyUpdateMode::Continuous(),
            UpdateMode::Reactive {
                wait,
                react_to_device_events,
                react_to_user_events,
                react_to_window_events,
            } => PyUpdateMode::Reactive {
                wait: wait.as_secs_f64(),
                react_to_device_events,
                react_to_user_events,
                react_to_window_events,
            },
        }
    }
}
