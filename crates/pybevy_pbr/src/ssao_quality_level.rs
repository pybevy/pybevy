use bevy::pbr::ScreenSpaceAmbientOcclusionQualityLevel;
use pyo3::prelude::*;

#[pyclass(name = "ScreenSpaceAmbientOcclusionQualityLevel", frozen, eq)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyScreenSpaceAmbientOcclusionQualityLevel {
    Low(),
    Medium(),
    High(),
    Ultra(),
    Custom {
        samples_per_slice_side: u32,
        slice_count: u32,
    },
}

#[pymethods]
impl PyScreenSpaceAmbientOcclusionQualityLevel {
    #[classattr]
    pub const LOW: Self = PyScreenSpaceAmbientOcclusionQualityLevel::Low();
    #[classattr]
    pub const MEDIUM: Self = PyScreenSpaceAmbientOcclusionQualityLevel::Medium();
    #[classattr]
    pub const HIGH: Self = PyScreenSpaceAmbientOcclusionQualityLevel::High();
    #[classattr]
    pub const ULTRA: Self = PyScreenSpaceAmbientOcclusionQualityLevel::Ultra();
}

impl From<ScreenSpaceAmbientOcclusionQualityLevel> for PyScreenSpaceAmbientOcclusionQualityLevel {
    fn from(level: ScreenSpaceAmbientOcclusionQualityLevel) -> Self {
        match level {
            ScreenSpaceAmbientOcclusionQualityLevel::Low => {
                PyScreenSpaceAmbientOcclusionQualityLevel::Low()
            }
            ScreenSpaceAmbientOcclusionQualityLevel::Medium => {
                PyScreenSpaceAmbientOcclusionQualityLevel::Medium()
            }
            ScreenSpaceAmbientOcclusionQualityLevel::High => {
                PyScreenSpaceAmbientOcclusionQualityLevel::High()
            }
            ScreenSpaceAmbientOcclusionQualityLevel::Ultra => {
                PyScreenSpaceAmbientOcclusionQualityLevel::Ultra()
            }
            ScreenSpaceAmbientOcclusionQualityLevel::Custom {
                samples_per_slice_side,
                slice_count,
            } => PyScreenSpaceAmbientOcclusionQualityLevel::Custom {
                samples_per_slice_side,
                slice_count,
            },
        }
    }
}

impl From<PyScreenSpaceAmbientOcclusionQualityLevel> for ScreenSpaceAmbientOcclusionQualityLevel {
    fn from(level: PyScreenSpaceAmbientOcclusionQualityLevel) -> Self {
        match level {
            PyScreenSpaceAmbientOcclusionQualityLevel::Low() => {
                ScreenSpaceAmbientOcclusionQualityLevel::Low
            }
            PyScreenSpaceAmbientOcclusionQualityLevel::Medium() => {
                ScreenSpaceAmbientOcclusionQualityLevel::Medium
            }
            PyScreenSpaceAmbientOcclusionQualityLevel::High() => {
                ScreenSpaceAmbientOcclusionQualityLevel::High
            }
            PyScreenSpaceAmbientOcclusionQualityLevel::Ultra() => {
                ScreenSpaceAmbientOcclusionQualityLevel::Ultra
            }
            PyScreenSpaceAmbientOcclusionQualityLevel::Custom {
                samples_per_slice_side,
                slice_count,
            } => ScreenSpaceAmbientOcclusionQualityLevel::Custom {
                samples_per_slice_side,
                slice_count,
            },
        }
    }
}
