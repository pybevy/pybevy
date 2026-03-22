pub mod plugin;
pub mod stopwatch;
pub mod time;
pub mod time_context;
pub mod timer;

pub use plugin::PyTimePlugin;
use pybevy_core::{plugin::plugin_registry, registry::global_registry};
use pybevy_macros::{plugin_bridge, resource_bridge};
use pyo3::prelude::*;
pub use stopwatch::PyStopwatch;
pub use time::{PyTime, PyTimeFixed, PyTimeReal, PyTimeVirtual};
pub use time_context::{PyFixed, PyReal, PyVirtual};
pub use timer::{PyTimer, PyTimerMode};

plugin_bridge!(PyTimePlugin, bevy::time::TimePlugin);

resource_bridge!(bevy::time::Time, PyTime);
resource_bridge!(
    bevy::time::Time<bevy::time::Fixed>,
    PyTimeFixed,
    "TimeFixed"
);
resource_bridge!(
    bevy::time::Time<bevy::time::Virtual>,
    PyTimeVirtual,
    "TimeVirtual"
);

pub fn register_time_bridges() {
    plugin_registry::register_plugin_bridge(TimePluginBridge);

    global_registry::register_resource_bridge(TimeBridge);
    global_registry::register_resource_bridge(TimeFixedBridge);
    global_registry::register_resource_bridge(TimeVirtualBridge);
}

pub fn add_time_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_time_bridges();

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
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "time")?;
    add_time_classes(&m)?;
    parent.add_submodule(&m)
}
