use bevy::pbr::ScreenSpaceAmbientOcclusionQualityLevel;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ScreenSpaceAmbientOcclusionQualityLevel, empty_tuple, unit_parens)]
#[pyclass(
    name = "ScreenSpaceAmbientOcclusionQualityLevel",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyScreenSpaceAmbientOcclusionQualityLevel {
    Low(),
    Medium(),
    High(),
    Ultra(),
    Custom {
        slice_count: u32,
        samples_per_slice_side: u32,
    },
}
