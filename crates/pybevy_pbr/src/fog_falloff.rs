use bevy::{
    math::{Vec3, ops},
    pbr::FogFalloff,
};
use pybevy_color::color::PyColor;
use pybevy_macros::pyenum;
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

#[pyenum(FogFalloff, no_repr)]
#[pyclass(name = "FogFalloff", frozen, from_py_object)]
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
        #[py_type(PyVec3)]
        #[py_try_into]
        extinction: Vec3,
        #[py_type(PyVec3)]
        #[py_try_into]
        inscattering: Vec3,
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
    pub fn from_visibility_color(
        visibility: f32,
        extinction_inscattering_color: PyColor,
    ) -> PyResult<Self> {
        Ok(
            FogFalloff::from_visibility_color(
                visibility,
                extinction_inscattering_color.try_into()?,
            )
            .into(),
        )
    }

    #[staticmethod]
    pub fn from_visibility_colors(
        visibility: f32,
        extinction_color: PyColor,
        inscattering_color: PyColor,
    ) -> PyResult<Self> {
        Ok(FogFalloff::from_visibility_colors(
            visibility,
            extinction_color.try_into()?,
            inscattering_color.try_into()?,
        )
        .into())
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
    ) -> PyResult<Self> {
        Ok(FogFalloff::from_visibility_contrast_color(
            visibility,
            contrast_threshold,
            extinction_inscattering_color.try_into()?,
        )
        .into())
    }

    #[staticmethod]
    pub fn from_visibility_contrast_colors(
        visibility: f32,
        contrast_threshold: f32,
        extinction_color: PyColor,
        inscattering_color: PyColor,
    ) -> PyResult<Self> {
        Ok(FogFalloff::from_visibility_contrast_colors(
            visibility,
            contrast_threshold,
            extinction_color.try_into()?,
            inscattering_color.try_into()?,
        )
        .into())
    }

    #[staticmethod]
    pub fn koschmieder(v: f32, c_t: f32) -> f32 {
        -ops::ln(c_t) / v
    }
}
