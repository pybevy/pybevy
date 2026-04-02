use pyo3::prelude::*;

pub mod readback;
pub mod readback_py;
pub mod wgpu_error_handler;

pub(crate) fn add_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(m.py(), "render_readback")?;
    readback_py::add_to_module(&module)?;
    m.add_submodule(&module)
}
