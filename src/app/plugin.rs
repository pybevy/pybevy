pub use pybevy_core::PyPlugin;
use pybevy_core::public_error::{PLUGIN_GROUP_BUILD_MIGRATION, PLUGIN_GROUP_BUILD_RESULT};
use pyo3::{
    exceptions::{PyNotImplementedError, PyTypeError},
    prelude::*,
    types::{PyDict, PyTuple, PyType},
};

use super::plugins::PyPluginGroupBuilder;

#[pyclass(name = "PluginGroup", module = "pybevy.app", subclass)]
#[derive(Debug)]
pub struct PyPluginGroup;

#[pymethods]
impl PyPluginGroup {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    pub fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        PyPluginGroup
    }

    #[classmethod]
    pub fn name(class: &Bound<'_, PyType>) -> PyResult<String> {
        Ok(format!(
            "{}.{}",
            class.getattr("__module__")?.extract::<String>()?,
            class.getattr("__qualname__")?.extract::<String>()?
        ))
    }

    pub fn set(
        pyself: Bound<'_, Self>,
        plugin: Bound<'_, PyAny>,
    ) -> PyResult<Py<PyPluginGroupBuilder>> {
        let result = build_plugin_group(pyself.as_any())?;
        let builder = result
            .cast::<PyPluginGroupBuilder>()
            .map_err(|_| PyTypeError::new_err(PLUGIN_GROUP_BUILD_RESULT))?;
        builder.borrow().set(pyself.py(), plugin)
    }

    pub fn build(pyself: Bound<'_, Self>, _py: Python) -> PyResult<Py<PyAny>> {
        Err(PyNotImplementedError::new_err(format!(
            "PluginGroup.build() not implemented for {}",
            pyself.get_type().name()?
        )))
    }
}

pub(crate) fn build_plugin_group<'py>(group: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let build = group.getattr("build")?;
    if let Ok(inspect) = group.py().import("inspect")
        && let Ok(signature) = inspect.call_method1("signature", (&build,))
        && let Err(error) = signature.call_method0("bind")
        && error.is_instance_of::<PyTypeError>(group.py())
        && signature.call_method1("bind", (group.py().None(),)).is_ok()
    {
        return Err(PyTypeError::new_err(PLUGIN_GROUP_BUILD_MIGRATION));
    }
    build.call0()
}
