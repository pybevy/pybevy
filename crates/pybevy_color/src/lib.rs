pub mod color_impl;

// Re-export main types
pub use color_impl::{
    PyColor, PyHsla, PyHsva, PyLaba, PyLcha, PyLinearRgba, PyOklaba, PyOklcha, PySrgba, PyXyza,
};
use pyo3::prelude::*;
pub fn add_color_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyColor>()?;
    m.add_class::<PyLinearRgba>()?;
    m.add_class::<PySrgba>()?;
    m.add_class::<PyHsla>()?;
    m.add_class::<PyOklcha>()?;
    m.add_class::<PyLcha>()?;
    m.add_class::<PyHsva>()?;
    m.add_class::<PyLaba>()?;
    m.add_class::<PyOklaba>()?;
    m.add_class::<PyXyza>()?;
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "color")?;
    add_color_classes(&m)?;
    parent.add_submodule(&m)
}
