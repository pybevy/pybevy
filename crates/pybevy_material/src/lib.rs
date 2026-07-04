pub mod alpha_mode;
pub mod opaque_renderer_method;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{alpha_mode::PyAlphaMode, opaque_renderer_method::PyOpaqueRendererMethod};
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "material")?;
    m.add_class::<alpha_mode::PyAlphaMode>()?;
    m.add_class::<opaque_renderer_method::PyOpaqueRendererMethod>()?;
    parent.add_submodule(&m)
}
