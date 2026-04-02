use bevy::post_process::bloom::BloomCompositeMode;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(BloomCompositeMode)]
#[pyclass(name = "BloomCompositeMode", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyBloomCompositeMode {
    EnergyConserving,
    Additive,
}
