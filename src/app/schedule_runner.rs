use std::time::Duration;

use bevy::app::{RunMode, ScheduleRunnerPlugin};
use pybevy_core::PyPlugin;
use pyo3::prelude::*;

use crate::prelude::PyApp;

#[pyclass(name = "ScheduleRunnerPlugin", extends = PyPlugin, skip_from_py_object)]
#[derive(Clone)]
pub struct PyScheduleRunnerPlugin {
    run_mode: PyRunMode,
}

#[pyclass(name = "RunMode", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyRunMode {
    #[pyo3(constructor = (wait = None))]
    Loop {
        wait: Option<u64>,
    },
    Once(),
}

impl From<PyRunMode> for RunMode {
    fn from(py_run_mode: PyRunMode) -> Self {
        match py_run_mode {
            PyRunMode::Loop { wait } => RunMode::Loop {
                wait: wait.map(Duration::from_millis),
            },
            PyRunMode::Once() => RunMode::Once,
        }
    }
}

impl PyScheduleRunnerPlugin {
    fn with_mode(run_mode: PyRunMode) -> Self {
        PyScheduleRunnerPlugin { run_mode }
    }
}

#[pymethods]
impl PyScheduleRunnerPlugin {
    #[new]
    #[pyo3(signature = (run_mode = PyRunMode::Loop { wait: None }))]
    pub fn new(run_mode: PyRunMode) -> PyClassInitializer<Self> {
        (PyScheduleRunnerPlugin { run_mode }, PyPlugin).into()
    }

    #[staticmethod]
    pub fn run_once(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, (Self::with_mode(PyRunMode::Once()), PyPlugin))
    }

    #[staticmethod]
    #[pyo3(signature = (wait_duration = None))]
    pub fn run_loop(py: Python, wait_duration: Option<u64>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self::with_mode(PyRunMode::Loop {
                    wait: wait_duration,
                }),
                PyPlugin,
            ),
        )
    }

    pub fn build(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        let run_mode: RunMode = self.run_mode.into();

        app.borrow().with_bevy_app(|bevy_app| {
            bevy_app.add_plugins(ScheduleRunnerPlugin { run_mode });
            Ok(())
        })
    }
}
