use pybevy_macros::pyenum;
use pyo3::prelude::*;
use wgpu_types::PowerPreference;

#[pyenum(PowerPreference)]
#[pyclass(name = "PowerPreference", eq, from_py_object, frozen, hash)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyPowerPreference {
    #[pyo3(name = "None_")]
    None,
    LowPower,
    HighPerformance,
}
