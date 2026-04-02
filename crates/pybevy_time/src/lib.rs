pub mod plugin;
pub mod stopwatch;
pub mod time;
pub mod time_context;
pub mod timer;

pub use plugin::PyTimePlugin;
use pyo3::prelude::*;
pub use stopwatch::PyStopwatch;
pub use time::{PyTime, PyTimeFixed, PyTimeReal, PyTimeVirtual};
pub use time_context::{PyFixed, PyReal, PyVirtual};
pub use timer::{PyTimer, PyTimerMode};

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "time")?;
    m.add_class::<PyTimePlugin>()?;

    m.add_class::<PyTime>()?;
    m.add_class::<PyTimeFixed>()?;
    m.add_class::<PyTimeVirtual>()?;
    m.add_class::<PyTimeReal>()?;

    m.add_class::<PyTimer>()?;
    m.add_class::<PyTimerMode>()?;
    m.add_class::<PyStopwatch>()?;
    m.add_class::<PyFixed>()?;
    m.add_class::<PyReal>()?;
    m.add_class::<PyVirtual>()?;
    parent.add_submodule(&m)
}
