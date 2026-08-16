use bevy::{math::Vec2, post_process::effect_stack::LensDistortion};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(LensDistortion, bridge, view_fields = [
    intensity,
    scale,
    edge_curvature
])]
#[pyclass(name = "LensDistortion", extends = PyComponent)]
pub struct PyLensDistortion {
    pub(crate) storage: ComponentStorage<LensDistortion>,
}

#[pymethods]
impl PyLensDistortion {
    #[new]
    #[pyo3(signature = (
        intensity = 0.5,
        scale = 1.0,
        multiplier = None,
        center = None,
        edge_curvature = 0.0
    ))]
    pub fn new(
        intensity: f32,
        scale: f32,
        multiplier: Option<PyVec2>,
        center: Option<PyVec2>,
        edge_curvature: f32,
    ) -> PyResult<PyClassInitializer<Self>> {
        let lens_distortion = LensDistortion {
            intensity,
            scale,
            multiplier: multiplier
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or(Vec2::ONE),
            center: center
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or(Vec2::splat(0.5)),
            edge_curvature,
        };

        Ok(Self::from_owned(lens_distortion).into())
    }

    #[getter]
    pub fn intensity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.intensity)
    }

    #[setter]
    pub fn set_intensity(&mut self, intensity: f32) -> PyResult<()> {
        self.as_mut()?.intensity = intensity;
        Ok(())
    }

    #[getter]
    pub fn scale(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.scale)
    }

    #[setter]
    pub fn set_scale(&mut self, scale: f32) -> PyResult<()> {
        self.as_mut()?.scale = scale;
        Ok(())
    }

    #[getter]
    pub fn multiplier(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|v| &v.multiplier)?)
    }

    #[setter]
    pub fn set_multiplier(&mut self, multiplier: PyVec2) -> PyResult<()> {
        self.as_mut()?.multiplier = multiplier.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn center(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|v| &v.center)?)
    }

    #[setter]
    pub fn set_center(&mut self, center: PyVec2) -> PyResult<()> {
        self.as_mut()?.center = center.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn edge_curvature(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.edge_curvature)
    }

    #[setter]
    pub fn set_edge_curvature(&mut self, edge_curvature: f32) -> PyResult<()> {
        self.as_mut()?.edge_curvature = edge_curvature;
        Ok(())
    }
}
