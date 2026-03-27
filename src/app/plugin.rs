pub use pybevy_core::PyPlugin;
use pyo3::{
    exceptions::PyNotImplementedError,
    prelude::*,
    types::{PyDict, PyTuple},
};

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

    pub fn build(pyself: Bound<'_, Self>, _py: Python) -> PyResult<Py<PyAny>> {
        Err(PyNotImplementedError::new_err(format!(
            "PluginGroup.build() not implemented for {}",
            pyself.get_type().name()?
        )))
    }
}
