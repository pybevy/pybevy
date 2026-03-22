use bevy::{math::ops, pbr::FogFalloff};
use pybevy_color::PyColor;
use pybevy_math::PyVec3;
use pyo3::prelude::*;

#[pyclass(name = "FogFalloff", frozen)]
#[derive(Debug, Clone)]
pub enum PyFogFalloff {
    Linear {
        start: f32,
        end: f32,
    },
    Exponential {
        density: f32,
    },
    ExponentialSquared {
        density: f32,
    },
    Atmospheric {
        extinction: PyVec3,
        inscattering: PyVec3,
    },
}

#[pymethods]
impl PyFogFalloff {
    #[classattr]
    pub const REVISED_KOSCHMIEDER_CONTRAST_THRESHOLD: f32 =
        FogFalloff::REVISED_KOSCHMIEDER_CONTRAST_THRESHOLD;

    #[staticmethod]
    pub fn from_visibility(visibility: f32) -> Self {
        FogFalloff::from_visibility(visibility).into()
    }

    #[staticmethod]
    pub fn from_visibility_squared(visibility: f32) -> Self {
        FogFalloff::from_visibility_squared(visibility).into()
    }

    #[staticmethod]
    pub fn from_visibility_color(visibility: f32, extinction_inscattering_color: PyColor) -> Self {
        FogFalloff::from_visibility_color(visibility, extinction_inscattering_color.0).into()
    }

    #[staticmethod]
    pub fn from_visibility_colors(
        visibility: f32,
        extinction_color: PyColor,
        inscattering_color: PyColor,
    ) -> Self {
        FogFalloff::from_visibility_colors(visibility, extinction_color.0, inscattering_color.0)
            .into()
    }

    #[staticmethod]
    pub fn from_visibility_contrast(visibility: f32, contrast_threshold: f32) -> Self {
        FogFalloff::from_visibility_contrast(visibility, contrast_threshold).into()
    }

    #[staticmethod]
    pub fn from_visibility_contrast_squared(visibility: f32, contrast_threshold: f32) -> Self {
        FogFalloff::from_visibility_contrast_squared(visibility, contrast_threshold).into()
    }

    #[staticmethod]
    pub fn from_visibility_contrast_color(
        visibility: f32,
        contrast_threshold: f32,
        extinction_inscattering_color: PyColor,
    ) -> Self {
        FogFalloff::from_visibility_contrast_color(
            visibility,
            contrast_threshold,
            extinction_inscattering_color.0,
        )
        .into()
    }

    #[staticmethod]
    pub fn from_visibility_contrast_colors(
        visibility: f32,
        contrast_threshold: f32,
        extinction_color: PyColor,
        inscattering_color: PyColor,
    ) -> Self {
        FogFalloff::from_visibility_contrast_colors(
            visibility,
            contrast_threshold,
            extinction_color.0,
            inscattering_color.0,
        )
        .into()
    }

    #[staticmethod]
    pub fn koschmieder(v: f32, c_t: f32) -> f32 {
        -ops::ln(c_t) / v
    }
}

impl From<FogFalloff> for PyFogFalloff {
    fn from(falloff: FogFalloff) -> Self {
        match falloff {
            FogFalloff::Linear { start, end } => PyFogFalloff::Linear { start, end },
            FogFalloff::Exponential { density } => PyFogFalloff::Exponential { density },
            FogFalloff::ExponentialSquared { density } => {
                PyFogFalloff::ExponentialSquared { density }
            }
            FogFalloff::Atmospheric {
                extinction,
                inscattering,
            } => PyFogFalloff::Atmospheric {
                extinction: extinction.into(),
                inscattering: inscattering.into(),
            },
        }
    }
}

impl From<PyFogFalloff> for FogFalloff {
    fn from(falloff: PyFogFalloff) -> Self {
        match falloff {
            PyFogFalloff::Linear { start, end } => FogFalloff::Linear { start, end },
            PyFogFalloff::Exponential { density } => FogFalloff::Exponential { density },
            PyFogFalloff::ExponentialSquared { density } => {
                FogFalloff::ExponentialSquared { density }
            }
            PyFogFalloff::Atmospheric {
                extinction,
                inscattering,
            } => FogFalloff::Atmospheric {
                extinction: extinction.into(),
                inscattering: inscattering.into(),
            },
        }
    }
}
