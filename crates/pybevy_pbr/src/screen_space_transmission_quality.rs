use bevy::pbr::ScreenSpaceTransmissionQuality;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ScreenSpaceTransmissionQuality)]
#[pyclass(name = "ScreenSpaceTransmissionQuality", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyScreenSpaceTransmissionQuality {
    Low,
    Medium,
    High,
    Ultra,
}
