// Re-export base Plugin class from pybevy_core to ensure all crates use the same type
pub use pybevy_core::PyPlugin;
use pyo3::{
    exceptions::PyNotImplementedError,
    prelude::*,
    types::{PyDict, PyTuple},
};

/// Base class for plugin groups (collections of plugins).
///
/// Plugin groups implement `build(self)` which returns a `PluginGroupBuilder`
/// for configuring which plugins to include/exclude.
/// This matches Bevy's `PluginGroup` trait.
#[pyclass(name = "PluginGroup", subclass)]
#[derive(Debug)]
pub struct PyPluginGroup;

#[pymethods]
impl PyPluginGroup {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    pub fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        PyPluginGroup
    }

    /// Build the plugin group, returning a builder for configuration.
    ///
    /// Returns a `PluginGroupBuilder` that can be used to enable/disable
    /// specific plugins before adding to the app.
    pub fn build(pyself: Bound<'_, Self>, _py: Python) -> PyResult<Py<PyAny>> {
        Err(PyNotImplementedError::new_err(format!(
            "PluginGroup.build() not implemented for {}",
            pyself.get_type().name()?.to_string()
        )))
    }
}
