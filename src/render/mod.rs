use pyo3::prelude::*;

pub mod readback;
pub mod readback_py;

pub(crate) fn add_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    readback_py::register_readback_bridges();
    let module = PyModule::new(m.py(), "render_readback")?;
    readback_py::add_to_module(&module)?;
    m.add_submodule(&module)
}
