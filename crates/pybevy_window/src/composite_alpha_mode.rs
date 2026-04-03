use bevy::window::CompositeAlphaMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(CompositeAlphaMode)]
#[pyclass(name = "CompositeAlphaMode", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyCompositeAlphaMode {
    Auto,
    Opaque,
    PreMultiplied,
    PostMultiplied,
    Inherit,
}
