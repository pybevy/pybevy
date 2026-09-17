use bevy::render::render_resource::TextureViewDimension;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(TextureViewDimension)]
#[pyclass(
    name = "TextureViewDimension",
    module = "pybevy.render",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyTextureViewDimension {
    D1,
    D2,
    D2Array,
    Cube,
    CubeArray,
    D3,
}
