use bevy::camera::ScreenSpaceTransmissionQuality;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(ScreenSpaceTransmissionQuality)]
#[pyclass(name = "ScreenSpaceTransmissionQuality", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyScreenSpaceTransmissionQuality {
    Low,
    Medium,
    High,
    Ultra,
}
