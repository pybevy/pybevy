pub mod plugin;
pub mod tonemapping;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{plugin::PyCorePipelinePlugin, tonemapping::PyTonemapping};
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "core_pipeline")?;
    m.add_class::<plugin::PyCorePipelinePlugin>()?;
    m.add_class::<tonemapping::PyTonemapping>()?;
    parent.add_submodule(&m)
}
