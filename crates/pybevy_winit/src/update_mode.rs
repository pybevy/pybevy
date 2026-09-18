use std::time::Duration;

use bevy::winit::UpdateMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(UpdateMode, empty_tuple, no_repr)]
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
    #[pyo3(constructor = (*, wait, react_to_device_events, react_to_user_events, react_to_window_events))]
    Reactive {
        wait: Duration,
        react_to_device_events: bool,
        react_to_user_events: bool,
        react_to_window_events: bool,
    },
}

#[pymethods]
impl PyUpdateMode {
    #[staticmethod]
    pub fn reactive(wait: Duration) -> Self {
        UpdateMode::reactive(wait).into()
    }

    #[staticmethod]
    pub fn reactive_low_power(wait: Duration) -> Self {
        UpdateMode::reactive_low_power(wait).into()
    }

    pub fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        match self {
            Self::Continuous() => Ok("UpdateMode.Continuous()".to_string()),
            Self::Reactive {
                wait,
                react_to_device_events,
                react_to_user_events,
                react_to_window_events,
            } => {
                let flag = |value: bool| if value { "True" } else { "False" };
                let wait_repr = wait.into_pyobject(py)?.repr()?;
                Ok(format!(
                    "UpdateMode.Reactive(wait={}, react_to_device_events={}, react_to_user_events={}, react_to_window_events={})",
                    wait_repr.to_str()?,
                    flag(*react_to_device_events),
                    flag(*react_to_user_events),
                    flag(*react_to_window_events)
                ))
            }
        }
    }
}
