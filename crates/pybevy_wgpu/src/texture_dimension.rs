use bevy::render::render_resource::TextureDimension;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(TextureDimension)]
#[pyclass(name = "TextureDimension")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyTextureDimension {
    D1,
    D2,
    D3,
}
