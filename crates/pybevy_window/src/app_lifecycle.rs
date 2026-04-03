use bevy::window::AppLifecycle;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(AppLifecycle)]
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
