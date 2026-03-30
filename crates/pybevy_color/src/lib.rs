mod common;
pub mod color;
pub mod hsla;
pub mod hsva;
pub mod laba;
pub mod lcha;
pub mod linear_rgba;
pub mod oklaba;
pub mod oklcha;
pub mod srgba;
pub mod xyza;

pub use color::PyColor;
pub use hsla::PyHsla;
pub use hsva::PyHsva;
pub use laba::PyLaba;
pub use lcha::PyLcha;
pub use linear_rgba::PyLinearRgba;
pub use oklaba::PyOklaba;
pub use oklcha::PyOklcha;
pub use srgba::PySrgba;
pub use xyza::PyXyza;

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
