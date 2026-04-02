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
use pyo3::prelude::*;

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "mcp")?;
    m.add_class::<PyControlPlugin>()?;
    m.add_class::<api_index::PyApiIndex>()?;
    parent.add_submodule(&m)
}
