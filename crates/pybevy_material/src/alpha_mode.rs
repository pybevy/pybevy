use bevy::material::AlphaMode;
use pybevy_core::domain::{FINITE, check};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(AlphaMode, empty_tuple, unit_parens)]
#[pyclass(
    name = "AlphaMode",
    module = "pybevy.material",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyAlphaMode {
    Opaque(),
    #[py_bevy(tuple)]
    Mask {
        value: f32,
    },
    Blend(),
    Premultiplied(),
    Add(),
    Multiply(),
    AlphaToCoverage(),
}

/// Converts a mode for storage, rejecting a non-finite `Mask` cutoff.
pub fn checked_alpha_mode(mode: PyAlphaMode) -> PyResult<AlphaMode> {
    if let PyAlphaMode::Mask { value } = mode {
        check("AlphaMode.Mask", "value", value, FINITE)?;
    }
    Ok(mode.into())
}
