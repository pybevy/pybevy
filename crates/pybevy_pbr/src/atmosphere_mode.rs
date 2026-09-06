use bevy::pbr::AtmosphereMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(AtmosphereMode)]
#[pyclass(
    name = "AtmosphereMode",
    module = "pybevy.pbr",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyAtmosphereMode {
    LookupTexture,
    Raymarched,
}
