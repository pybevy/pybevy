use bevy::{color::Color, math::Vec2, post_process::effect_stack::Vignette};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(Vignette, bridge, view_fields = [
    intensity,
    radius,
    smoothness,
    roundness,
    edge_compensation
])]
#[pyclass(name = "Vignette", extends = PyComponent)]
pub struct PyVignette {
    pub(crate) storage: ComponentStorage<Vignette>,
}

#[pymethods]
impl PyVignette {
    #[new]
    #[pyo3(signature = (
        intensity = 1.0,
        radius = 0.75,
        smoothness = 5.0,
        roundness = 1.0,
        center = None,
        edge_compensation = 1.0,
        color = None
    ))]
    pub fn new(
        intensity: f32,
        radius: f32,
        smoothness: f32,
        roundness: f32,
        center: Option<PyVec2>,
        edge_compensation: f32,
        color: Option<PyColor>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let vignette = Vignette {
            intensity,
            radius,
            smoothness,
            roundness,
            center: center
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or(Vec2::new(0.5, 0.5)),
            edge_compensation,
            color: color
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or(Color::BLACK),
        };

        Ok(Self::from_owned(vignette).into())
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
    pub fn radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.radius)
    }

    #[setter]
    pub fn set_radius(&mut self, radius: f32) -> PyResult<()> {
        self.as_mut()?.radius = radius;
        Ok(())
    }

    #[getter]
    pub fn smoothness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.smoothness)
    }

    #[setter]
    pub fn set_smoothness(&mut self, smoothness: f32) -> PyResult<()> {
        self.as_mut()?.smoothness = smoothness;
        Ok(())
    }

    #[getter]
    pub fn roundness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.roundness)
    }

    #[setter]
    pub fn set_roundness(&mut self, roundness: f32) -> PyResult<()> {
        self.as_mut()?.roundness = roundness;
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
    pub fn edge_compensation(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.edge_compensation)
    }

    #[setter]
    pub fn set_edge_compensation(&mut self, edge_compensation: f32) -> PyResult<()> {
        self.as_mut()?.edge_compensation = edge_compensation;
        Ok(())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |vignette| &vignette.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = color.try_into()?;
        self.as_mut()?.color = color;
        Ok(())
    }
}
