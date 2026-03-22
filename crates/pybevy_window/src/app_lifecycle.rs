use bevy::window::AppLifecycle;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(AppLifecycle, from_only)]
#[pyclass(name = "AppLifecycle", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyAppLifecycle {
    Idle,
    Running,
    WillSuspend,
    Suspended,
    WillResume,
}

#[pymethods]
impl PyAppLifecycle {
    pub fn is_active(&self) -> bool {
        let bevy_lifecycle: AppLifecycle = (*self).into();
        bevy_lifecycle.is_active()
    }

    pub fn __repr__(&self) -> &'static str {
        match self {
            PyAppLifecycle::Idle => "AppLifecycle.Idle",
            PyAppLifecycle::Running => "AppLifecycle.Running",
            PyAppLifecycle::WillSuspend => "AppLifecycle.WillSuspend",
            PyAppLifecycle::Suspended => "AppLifecycle.Suspended",
            PyAppLifecycle::WillResume => "AppLifecycle.WillResume",
        }
    }
}
