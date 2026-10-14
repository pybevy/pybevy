use bevy::sprite_render::AlphaMode2d;
use pybevy_core::domain::{FINITE, check};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(AlphaMode2d, empty_tuple)]
#[pyclass(
    name = "AlphaMode2d",
    module = "pybevy.sprite",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyAlphaMode2d {
    Opaque(),
    #[py_bevy(tuple)]
    Mask {
        value: f32,
    },
    Blend(),
}

impl Default for PyAlphaMode2d {
    fn default() -> Self {
        AlphaMode2d::default().into()
    }
}

/// Converts a mode for storage, rejecting a non-finite `Mask` cutoff.
pub fn checked_alpha_mode_2d(mode: PyAlphaMode2d) -> PyResult<AlphaMode2d> {
    if let PyAlphaMode2d::Mask { value } = mode {
        check("AlphaMode2d.Mask", "value", value, FINITE)?;
    }
    Ok(mode.into())
}
