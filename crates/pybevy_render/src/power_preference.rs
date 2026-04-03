use pybevy_macros::pyenum;
use pyo3::prelude::*;
use wgpu_types::PowerPreference;

#[pyenum(PowerPreference)]
#[pyclass(name = "PowerPreference", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyPowerPreference {
    #[pyo3(name = "None_")]
    None,
    LowPower,
    HighPerformance,
}

#[pymethods]
impl PyPowerPreference {
    #[classattr]
    pub const NONE: Self = PyPowerPreference::None;
    #[classattr]
    pub const LOW_POWER: Self = PyPowerPreference::LowPower;
    #[classattr]
    pub const HIGH_PERFORMANCE: Self = PyPowerPreference::HighPerformance;
}
