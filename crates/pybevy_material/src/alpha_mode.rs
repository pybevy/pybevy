use bevy::material::AlphaMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(AlphaMode, empty_tuple, unit_parens)]
#[pyclass(name = "AlphaMode", frozen, eq, from_py_object)]
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
