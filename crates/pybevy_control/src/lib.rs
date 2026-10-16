pub mod asset_reference;
pub mod capture;
pub mod catalog;
pub mod codec;
pub mod discovery;
pub mod engine;
pub mod plugin;
pub mod protocol;
pub mod pyo3;
pub mod reload;
pub mod scene;
pub mod schedule;
pub mod transport;

use ::pyo3::{exceptions::PyRuntimeError, prelude::*};
#[doc(hidden)]
pub use catalog::{resources, tools};
#[doc(hidden)]
pub use discovery as api_index;
#[doc(hidden)]
pub use engine::runtime;
pub use plugin::PyControlPlugin;
#[doc(hidden)]
pub use pyo3::runtime as runtime_pyo3;
pub use pyo3::{
    components::register_lifecycle_mutation_hooks, execute::register_world_wrapper_hook,
};
#[doc(hidden)]
pub use transport::{client, server, sse};

#[doc(hidden)]
pub mod bridge {
    pub use crate::{
        capture::{hold::*, queue::*},
        engine::{channel::*, deferred::*, poll::*},
        protocol::{entity_ref::*, error::*, operation::*, params::*},
        reload::{PendingReload, PendingReloads},
        transport::sse::SseEventBroadcaster,
    };
}

#[doc(hidden)]
pub mod handlers {
    pub use crate::{
        capture::{analysis as frame_analysis, depth, screenshot, turnaround},
        codec::reflect as reflect_mutate,
        engine::dispatch::dispatch,
        pyo3, reload,
        scene::{diagnostics, entity, entity_count, spatial, time as time_control},
        schedule,
    };
}

/// Return MCP tool definitions generated from ControlOperation's JSON schema.
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
#[pyfunction]
fn rust_resource_definitions(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let res = catalog::resources::list_resources();
    let json_str = serde_json::to_string(&res)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize resources: {e}")))?;
    let json_mod = py.import("json")?;
    let result = json_mod.call_method1("loads", (json_str,))?;
    Ok(result.into())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "mcp")?;
    m.add_class::<PyControlPlugin>()?;
    m.add_class::<discovery::PyApiIndex>()?;
    m.add_function(wrap_pyfunction!(rust_tool_definitions, &m)?)?;
    m.add_function(wrap_pyfunction!(rust_resource_definitions, &m)?)?;
    transport::client_pyo3::add_functions(&m)?;
    parent.add_submodule(&m)
}
