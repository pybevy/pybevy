use pybevy_core::{DefaultPluginKind, plugin::plugin_registry};
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyType};

pub type PluginConfigType = DefaultPluginKind;

pub fn try_plugin_config_type(py_type: &Bound<'_, PyType>) -> Option<PluginConfigType> {
    plugin_registry::get_by_py_type(py_type.as_type_ptr())?.default_plugin_kind()
}

pub fn plugin_config_type(py_type: &Bound<'_, PyType>) -> PyResult<PluginConfigType> {
    if let Some(config_type) = try_plugin_config_type(py_type) {
        return Ok(config_type);
    }

    let type_name = py_type.name()?.to_string();
    Err(PyTypeError::new_err(format!(
        "Unknown plugin type: {}",
        type_name
    )))
}
