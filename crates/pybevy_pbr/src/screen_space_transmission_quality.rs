use bevy::pbr::ScreenSpaceTransmissionQuality;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ScreenSpaceTransmissionQuality)]
#[pyclass(name = "ScreenSpaceTransmissionQuality", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyScreenSpaceTransmissionQuality {
    Low,
    Medium,
    High,
    Ultra,
}
