pub mod plugin;
pub mod stopwatch;
pub mod time;
pub mod time_context;
pub mod time_update_strategy;
pub mod timer;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        plugin::PyTimePlugin,
        stopwatch::PyStopwatch,
        time::{PyTime, PyTimeFixed, PyTimeReal, PyTimeVirtual},
        time_context::{PyFixed, PyReal, PyVirtual},
        time_update_strategy::PyTimeUpdateStrategy,
        timer::{PyTimer, PyTimerMode},
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "time")?;
    m.add_class::<plugin::PyTimePlugin>()?;

    m.add_class::<time::PyTime>()?;
    m.add_class::<time::PyTimeFixed>()?;
    m.add_class::<time::PyTimeVirtual>()?;
    m.add_class::<time::PyTimeReal>()?;
    m.add_class::<time_update_strategy::PyTimeUpdateStrategy>()?;

    m.add_class::<timer::PyTimer>()?;
    m.add_class::<timer::PyTimerMode>()?;
    m.add_class::<stopwatch::PyStopwatch>()?;
    m.add_class::<time_context::PyFixed>()?;
    m.add_class::<time_context::PyReal>()?;
    m.add_class::<time_context::PyVirtual>()?;
    parent.add_submodule(&m)
}
