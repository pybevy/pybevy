pub mod api_index;
pub mod bridge;
pub mod handlers;
pub mod plugin;
pub mod protocol;
pub mod runtime;
pub mod runtime_pyo3;
pub mod server;
pub mod sse;

pub use handlers::pyo3::execute::register_world_wrapper_hook;
pub use plugin::PyControlPlugin;
use pybevy_core::plugin::plugin_registry;
use pybevy_macros::plugin_bridge;
use pyo3::prelude::*;

plugin_bridge!(
    PyControlPlugin,
    plugin::ControlBevyPlugin,
    |py_plugin, app| {
        let config: pyo3::PyRef<'_, PyControlPlugin> = py_plugin.extract()?;
        app.add_plugins(plugin::ControlBevyPlugin {
            config: config.clone(),
        });
        Ok(())
    }
);

pub fn register_control_bridges() {
    plugin_registry::register_plugin_bridge(ControlBevyPluginBridge);
}

pub fn add_control_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_control_bridges();
    m.add_class::<PyControlPlugin>()?;
    m.add_class::<api_index::PyApiIndex>()?;
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "mcp")?;
    add_control_classes(&m)?;
    parent.add_submodule(&m)
}
