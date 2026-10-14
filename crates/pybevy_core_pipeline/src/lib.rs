pub mod plugin;
pub mod prepass;
pub mod tonemapping;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{plugin::PyCorePipelinePlugin, tonemapping::PyTonemapping};
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "core_pipeline")?;
    m.add_class::<plugin::PyCorePipelinePlugin>()?;
    m.add_class::<tonemapping::PyTonemapping>()?;
    m.add_class::<prepass::PyDepthPrepass>()?;
    m.add_class::<prepass::PyNormalPrepass>()?;
    m.add_class::<prepass::PyMotionVectorPrepass>()?;
    m.add_class::<prepass::PyDeferredPrepass>()?;
    parent.add_submodule(&m)
}
