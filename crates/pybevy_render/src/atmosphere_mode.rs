use bevy::pbr::AtmosphereMode;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(AtmosphereMode)]
#[pyclass(name = "AtmosphereMode", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyAtmosphereMode {
    LookupTexture,
    Raymarched,
}

#[pymethods]
impl PyAtmosphereMode {
    #[classattr]
    pub const LOOKUP_TEXTURE: Self = PyAtmosphereMode::LookupTexture;
    #[classattr]
    pub const RAYMARCHED: Self = PyAtmosphereMode::Raymarched;

}
