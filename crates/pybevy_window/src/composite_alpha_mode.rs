use bevy::window::CompositeAlphaMode;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(CompositeAlphaMode)]
#[pyclass(name = "CompositeAlphaMode", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyCompositeAlphaMode {
    Auto,
    Opaque,
    PreMultiplied,
    PostMultiplied,
    Inherit,
}
