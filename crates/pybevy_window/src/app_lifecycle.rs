use bevy::window::AppLifecycle;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(AppLifecycle)]
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
}
