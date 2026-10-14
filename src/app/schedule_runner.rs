use std::time::Duration;

use bevy::app::{App, RunMode, ScheduleRunnerPlugin};
use pybevy_core::{
    PluginBuild, PyPlugin,
    domain::{IntegerInput, NON_NEGATIVE, check_for},
};
use pybevy_macros::{pyenum, pyplugin};
use pyo3::{prelude::*, types::PyInt};

use crate::prelude::PyApp;

#[pyplugin(ScheduleRunnerPlugin)]
#[pyclass(name = "ScheduleRunnerPlugin", module = "pybevy.app", extends = PyPlugin, skip_from_py_object)]
#[derive(Clone)]
pub struct PyScheduleRunnerPlugin {
    run_mode: PyRunMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunModeWait(Duration);

impl<'py> IntoPyObject<'py> for RunModeWait {
    type Target = PyInt;
    type Output = Bound<'py, PyInt>;
    type Error = <u128 as IntoPyObject<'py>>::Error;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        self.0.as_millis().into_pyobject(py)
    }
}

impl FromPyObject<'_, '_> for RunModeWait {
    type Error = PyErr;

    fn extract(value: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        let value = value.extract::<IntegerInput<u64>>()?;
        let value = check_for::<PyRunMode, _>("Loop.wait", value, NON_NEGATIVE)?;
        Ok(Self(Duration::from_millis(value.stored())))
    }
}

#[pyenum(RunMode, manual)]
#[pyclass(name = "RunMode", module = "pybevy.app", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyRunMode {
    #[pyo3(constructor = (*, wait = None))]
    Loop {
        wait: Option<RunModeWait>,
    },
    Once(),
}

impl From<PyRunMode> for RunMode {
    fn from(py_run_mode: PyRunMode) -> Self {
        match py_run_mode {
            PyRunMode::Loop { wait } => RunMode::Loop {
                wait: wait.map(|wait| wait.0),
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
    #[pyo3(signature = (*, run_mode = PyRunMode::Loop { wait: None }))]
    pub fn new(run_mode: PyRunMode) -> PyClassInitializer<Self> {
        (PyScheduleRunnerPlugin { run_mode }, PyPlugin).into()
    }

    #[staticmethod]
    pub fn run_once(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, (Self::with_mode(PyRunMode::Once()), PyPlugin))
    }

    #[staticmethod]
    #[pyo3(signature = (wait_duration = None))]
    pub fn run_loop(py: Python, wait_duration: Option<IntegerInput<u64>>) -> PyResult<Py<Self>> {
        let wait_duration = check_for::<Self, _>("wait_duration", wait_duration, NON_NEGATIVE)?
            .map(IntegerInput::stored);
        Py::new(
            py,
            (
                Self::with_mode(PyRunMode::Loop {
                    wait: wait_duration.map(|wait| RunModeWait(Duration::from_millis(wait))),
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

impl PluginBuild for PyScheduleRunnerPlugin {
    fn build(plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        let config: PyRef<'_, Self> = plugin.extract()?;
        app.add_plugins(ScheduleRunnerPlugin {
            run_mode: config.run_mode.into(),
        });
        Ok(())
    }
}
