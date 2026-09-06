use bevy::render::render_resource::TextureDimension;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(TextureDimension)]
#[pyclass(
    name = "TextureDimension",
    module = "pybevy.render",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyTextureDimension {
    D1,
    D2,
    D3,
}
