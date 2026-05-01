pub mod api_index;
pub mod bridge;
pub mod handlers;
pub mod plugin;
pub mod protocol;
pub mod runtime;
pub mod runtime_pyo3;
pub mod server;
pub mod sse;
pub mod tools;

pub use handlers::pyo3::execute::register_world_wrapper_hook;
pub use plugin::PyControlPlugin;
use pyo3::{exceptions::PyRuntimeError, prelude::*};

/// Return MCP tool definitions generated from ControlOperation's JSON schema.
#[pyfunction]
fn rust_tool_definitions(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let tools = tools::list_tools();
    let json_str = serde_json::to_string(&tools)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize tools: {e}")))?;
    // Parse JSON string into Python objects
    let json_mod = py.import("json")?;
    let result = json_mod.call_method1("loads", (json_str,))?;
    Ok(result.into())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "mcp")?;
    m.add_class::<PyControlPlugin>()?;
    m.add_class::<api_index::PyApiIndex>()?;
    m.add_function(wrap_pyfunction!(rust_tool_definitions, &m)?)?;
    parent.add_submodule(&m)
}
