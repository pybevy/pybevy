pub mod asset_reference;
pub mod capture;
pub mod catalog;
pub mod codec;
pub mod discovery;
pub mod engine;
pub mod plugin;
pub mod protocol;
#[cfg(feature = "pyo3-runtime")]
pub mod pyo3;
pub mod reload;
pub mod scene;
pub mod schedule;
pub mod transport;

#[cfg(feature = "pyo3-runtime")]
use ::pyo3::{exceptions::PyRuntimeError, prelude::*};
#[cfg(feature = "pyo3-runtime")]
pub use plugin::PyControlPlugin;
#[cfg(feature = "pyo3-runtime")]
use pyo3::api_index::PyApiIndex;
#[cfg(feature = "pyo3-runtime")]
pub use pyo3::{
    components::register_lifecycle_mutation_hooks, execute::register_world_wrapper_hook,
};

/// Return MCP tool definitions generated from ControlOperation's JSON schema.
#[cfg(feature = "pyo3-runtime")]
#[pyfunction]
fn rust_tool_definitions(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let tools = catalog::tools::list_tools();
    let json_str = serde_json::to_string(&tools)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize tools: {e}")))?;
    // Parse JSON string into Python objects
    let json_mod = py.import("json")?;
    let result = json_mod.call_method1("loads", (json_str,))?;
    Ok(result.into())
}

/// Return MCP resource definitions advertised by the engine.
#[cfg(feature = "pyo3-runtime")]
#[pyfunction]
fn rust_resource_definitions(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let res = catalog::resources::list_resources();
    let json_str = serde_json::to_string(&res)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize resources: {e}")))?;
    let json_mod = py.import("json")?;
    let result = json_mod.call_method1("loads", (json_str,))?;
    Ok(result.into())
}

#[cfg(feature = "pyo3-runtime")]
pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "mcp")?;
    m.add_class::<PyControlPlugin>()?;
    m.add_class::<PyApiIndex>()?;
    m.add_function(wrap_pyfunction!(rust_tool_definitions, &m)?)?;
    m.add_function(wrap_pyfunction!(rust_resource_definitions, &m)?)?;
    transport::client_pyo3::add_functions(&m)?;
    parent.add_submodule(&m)
}
