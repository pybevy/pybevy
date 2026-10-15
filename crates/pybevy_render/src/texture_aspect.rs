use bevy::render::render_resource::TextureAspect;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(TextureAspect)]
#[pyclass(
    name = "TextureAspect",
    module = "pybevy.render",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyTextureAspect {
    All,
    StencilOnly,
    DepthOnly,
    Plane0,
    Plane1,
    Plane2,
}
