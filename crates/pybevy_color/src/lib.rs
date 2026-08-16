pub mod color;
mod common;
pub mod hsla;
pub mod hsva;
pub mod hwba;
pub mod laba;
pub mod lcha;
pub mod linear_rgba;
pub mod oklaba;
pub mod oklcha;
pub mod srgba;
pub mod xyza;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        color::PyColor, hsla::PyHsla, hsva::PyHsva, hwba::PyHwba, laba::PyLaba, lcha::PyLcha,
        linear_rgba::PyLinearRgba, oklaba::PyOklaba, oklcha::PyOklcha, srgba::PySrgba,
        xyza::PyXyza,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "color")?;
    m.add_class::<color::PyColor>()?;
    m.add_class::<linear_rgba::PyLinearRgba>()?;
    m.add_class::<srgba::PySrgba>()?;
    m.add_class::<hsla::PyHsla>()?;
    m.add_class::<oklcha::PyOklcha>()?;
    m.add_class::<lcha::PyLcha>()?;
    m.add_class::<hsva::PyHsva>()?;
    m.add_class::<hwba::PyHwba>()?;
    m.add_class::<laba::PyLaba>()?;
    m.add_class::<oklaba::PyOklaba>()?;
    m.add_class::<xyza::PyXyza>()?;
    color::register_color_variants(&m)?;
    parent.add_submodule(&m)
}
