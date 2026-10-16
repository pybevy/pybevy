use bevy::{math::Vec2, post_process::effect_stack::LensDistortion};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

// Bevy documents center in UV space [0, 1]; the shader divides by scale.
#[pycomponent(
    LensDistortion,
    bridge,
    domains = [
        intensity => [finite],
        scale => [positive],
        multiplier => [finite],
        center => [range_inclusive(0.0, 1.0)],
        edge_curvature => [finite],
    ],
    view_fields = [intensity, scale, edge_curvature]
)]
#[pyclass(name = "LensDistortion", module = "pybevy.post_process", extends = PyComponent)]
pub struct PyLensDistortion {
    pub(crate) storage: ComponentStorage<LensDistortion>,
}

#[pymethods]
impl PyLensDistortion {
    #[new]
    #[pyo3(signature = (
        *,
        intensity = 0.5,
        scale = 1.0,
        multiplier = PyVec2::ONE,
        center = PyVec2::from(Vec2::splat(0.5)),
        edge_curvature = 0.0
    ))]
    pub fn new(
        intensity: f32,
        scale: f32,
        multiplier: PyVec2,
        center: PyVec2,
        edge_curvature: f32,
    ) -> PyResult<PyClassInitializer<Self>> {
        let lens_distortion = LensDistortion {
            intensity,
            scale,
            multiplier: multiplier.try_into()?,
            center: center.try_into()?,
            edge_curvature,
        };
        Self::validate_domains(&lens_distortion)?;

        Ok(Self::from_owned(lens_distortion).into())
    }

    #[getter]
    pub fn intensity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.intensity)
    }

    #[setter]
    pub fn set_intensity(&mut self, intensity: f32) -> PyResult<()> {
        let intensity = Self::check_intensity(intensity)?;
        self.as_mut()?.intensity = intensity;
        Ok(())
    }

    #[getter]
    pub fn scale(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.scale)
    }

    #[setter]
    pub fn set_scale(&mut self, scale: f32) -> PyResult<()> {
        let scale = Self::check_scale(scale)?;
        self.as_mut()?.scale = scale;
        Ok(())
    }

    #[getter]
    pub fn multiplier(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |v| &v.multiplier,
            |v| &mut v.multiplier,
            Self::validate_multiplier,
        )?)
    }

    #[setter]
    pub fn set_multiplier(&mut self, multiplier: PyVec2) -> PyResult<()> {
        let multiplier = Self::check_multiplier(Vec2::try_from(multiplier)?)?;
        self.as_mut()?.multiplier = multiplier;
        Ok(())
    }

    #[getter]
    pub fn center(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |v| &v.center,
            |v| &mut v.center,
            Self::validate_center,
        )?)
    }

    #[setter]
    pub fn set_center(&mut self, center: PyVec2) -> PyResult<()> {
        let center = Self::check_center(Vec2::try_from(center)?)?;
        self.as_mut()?.center = center;
        Ok(())
    }

    #[getter]
    pub fn edge_curvature(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.edge_curvature)
    }

    #[setter]
    pub fn set_edge_curvature(&mut self, edge_curvature: f32) -> PyResult<()> {
        let edge_curvature = Self::check_edge_curvature(edge_curvature)?;
        self.as_mut()?.edge_curvature = edge_curvature;
        Ok(())
    }
}
