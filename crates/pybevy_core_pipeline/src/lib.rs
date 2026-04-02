pub mod plugin;
pub mod tonemapping;

use pyo3::prelude::*;

pub use plugin::PyCorePipelinePlugin;
pub use tonemapping::PyTonemapping;

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "core_pipeline")?;
    m.add_class::<PyCorePipelinePlugin>()?;
    m.add_class::<PyTonemapping>()?;
    parent.add_submodule(&m)
}
