use bevy::{color::Color, math::Vec2, post_process::effect_stack::Vignette};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent, StorageError, public_error};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(Vignette, bridge, view_fields = [
    intensity => [range(f32::MIN, f32::INFINITY, public_error::VIGNETTE_INTENSITY_FINITE)],
    radius => [range(f32::MIN, f32::INFINITY, public_error::VIGNETTE_RADIUS_FINITE)],
    smoothness => [range(f32::MIN, f32::INFINITY, public_error::VIGNETTE_SMOOTHNESS_FINITE)],
    roundness => [range(f32::MIN, f32::INFINITY, public_error::VIGNETTE_ROUNDNESS_FINITE)],
    edge_compensation => [range(f32::MIN, f32::INFINITY, public_error::VIGNETTE_EDGE_COMPENSATION_FINITE)]
])]
#[pyclass(name = "Vignette", module = "pybevy.post_process", extends = PyComponent)]
pub struct PyVignette {
    pub(crate) storage: ComponentStorage<Vignette>,
}

fn check_finite(value: f32, message: &'static str) -> Result<f32, StorageError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(StorageError::InvalidValue(message.to_string()))
    }
}

fn check_center(center: &Vec2) -> Result<(), StorageError> {
    if center.is_finite() {
        Ok(())
    } else {
        Err(StorageError::InvalidValue(
            public_error::VIGNETTE_CENTER_FINITE.to_string(),
        ))
    }
}

#[pymethods]
impl PyVignette {
    #[new]
    #[pyo3(signature = (
        *,
        intensity = 1.0,
        radius = 0.75,
        smoothness = 5.0,
        roundness = 1.0,
        center = PyVec2::from(Vec2::new(0.5, 0.5)),
        edge_compensation = 1.0,
        color = PyColor::from(Color::BLACK)
    ))]
    pub fn new(
        intensity: f32,
        radius: f32,
        smoothness: f32,
        roundness: f32,
        center: PyVec2,
        edge_compensation: f32,
        color: PyColor,
    ) -> PyResult<PyClassInitializer<Self>> {
        let center: Vec2 = center.try_into()?;
        check_center(&center)?;
        let vignette = Vignette {
            intensity: check_finite(intensity, public_error::VIGNETTE_INTENSITY_FINITE)?,
            radius: check_finite(radius, public_error::VIGNETTE_RADIUS_FINITE)?,
            smoothness: check_finite(smoothness, public_error::VIGNETTE_SMOOTHNESS_FINITE)?,
            roundness: check_finite(roundness, public_error::VIGNETTE_ROUNDNESS_FINITE)?,
            center,
            edge_compensation: check_finite(
                edge_compensation,
                public_error::VIGNETTE_EDGE_COMPENSATION_FINITE,
            )?,
            color: color.try_into()?,
        };

        Ok(Self::from_owned(vignette).into())
    }

    #[getter]
    pub fn intensity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.intensity)
    }

    #[setter]
    pub fn set_intensity(&mut self, intensity: f32) -> PyResult<()> {
        let intensity = check_finite(intensity, public_error::VIGNETTE_INTENSITY_FINITE)?;
        self.as_mut()?.intensity = intensity;
        Ok(())
    }

    #[getter]
    pub fn radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.radius)
    }

    #[setter]
    pub fn set_radius(&mut self, radius: f32) -> PyResult<()> {
        let radius = check_finite(radius, public_error::VIGNETTE_RADIUS_FINITE)?;
        self.as_mut()?.radius = radius;
        Ok(())
    }

    #[getter]
    pub fn smoothness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.smoothness)
    }

    #[setter]
    pub fn set_smoothness(&mut self, smoothness: f32) -> PyResult<()> {
        let smoothness = check_finite(smoothness, public_error::VIGNETTE_SMOOTHNESS_FINITE)?;
        self.as_mut()?.smoothness = smoothness;
        Ok(())
    }

    #[getter]
    pub fn roundness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.roundness)
    }

    #[setter]
    pub fn set_roundness(&mut self, roundness: f32) -> PyResult<()> {
        let roundness = check_finite(roundness, public_error::VIGNETTE_ROUNDNESS_FINITE)?;
        self.as_mut()?.roundness = roundness;
        Ok(())
    }

    #[getter]
    pub fn center(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |v| &v.center,
            |v| &mut v.center,
            check_center,
        )?)
    }

    #[setter]
    pub fn set_center(&mut self, center: PyVec2) -> PyResult<()> {
        let center: Vec2 = center.try_into()?;
        check_center(&center)?;
        self.as_mut()?.center = center;
        Ok(())
    }

    #[getter]
    pub fn edge_compensation(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.edge_compensation)
    }

    #[setter]
    pub fn set_edge_compensation(&mut self, edge_compensation: f32) -> PyResult<()> {
        let edge_compensation = check_finite(
            edge_compensation,
            public_error::VIGNETTE_EDGE_COMPENSATION_FINITE,
        )?;
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
