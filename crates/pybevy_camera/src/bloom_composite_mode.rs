use bevy::post_process::bloom::BloomCompositeMode;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(BloomCompositeMode)]
#[pyclass(name = "BloomCompositeMode", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyBloomCompositeMode {
    EnergyConserving,
    Additive,
}
