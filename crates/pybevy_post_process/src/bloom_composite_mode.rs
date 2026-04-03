use bevy::post_process::bloom::BloomCompositeMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(BloomCompositeMode)]
#[pyclass(name = "BloomCompositeMode", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyBloomCompositeMode {
    EnergyConserving,
    Additive,
}
