use bevy::sprite_render::AlphaMode2d;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(AlphaMode2d, empty_tuple)]
#[pyclass(name = "AlphaMode2d", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyAlphaMode2d {
    Opaque(),
    #[py_bevy(tuple)]
    Mask {
        threshold: f32,
    },
    Blend(),
}

impl Default for PyAlphaMode2d {
    fn default() -> Self {
        AlphaMode2d::default().into()
    }
}
