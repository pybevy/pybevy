use bevy::sprite::SpriteAlphaMode;
use pybevy_core::domain::{FINITE, check};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(SpriteAlphaMode, empty_tuple)]
#[pyclass(
    name = "SpriteAlphaMode",
    module = "pybevy.sprite",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PySpriteAlphaMode {
    Opaque(),
    #[py_bevy(tuple)]
    Mask {
        value: f32,
    },
    Blend(),
}

impl Default for PySpriteAlphaMode {
    fn default() -> Self {
        SpriteAlphaMode::default().into()
    }
}

/// Converts a mode for storage, rejecting a non-finite `Mask` cutoff.
pub fn checked_sprite_alpha_mode(mode: PySpriteAlphaMode) -> PyResult<SpriteAlphaMode> {
    if let PySpriteAlphaMode::Mask { value } = mode {
        check("SpriteAlphaMode.Mask", "value", value, FINITE)?;
    }
    Ok(mode.into())
}
