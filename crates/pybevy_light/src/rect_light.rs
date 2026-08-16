use bevy::{color::Color, light::RectLight};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(RectLight, bridge, view_fields = [
    intensity,
    range,
    width,
    height
], batch_only_fields = [color])]
#[pyclass(name = "RectLight", extends = PyComponent)]
#[derive(Debug)]
pub struct PyRectLight {
    pub(crate) storage: ComponentStorage<RectLight>,
}

impl PyRectLight {
    fn default_color() -> PyColor {
        RectLight::default().color.into()
    }

    fn default_intensity() -> f32 {
        RectLight::default().intensity
    }

    fn default_range() -> f32 {
        RectLight::default().range
    }

    fn default_width() -> f32 {
        RectLight::default().width
    }

    fn default_height() -> f32 {
        RectLight::default().height
    }
}

#[pymethods]
impl PyRectLight {
    #[new]
    #[pyo3(signature = (
        color = Self::default_color(),
        intensity = Self::default_intensity(),
        range = Self::default_range(),
        width = Self::default_width(),
        height = Self::default_height()
    ))]
    pub fn new(
        color: PyColor,
        intensity: f32,
        range: f32,
        width: f32,
        height: f32,
    ) -> PyResult<PyClassInitializer<Self>> {
        let color = Color::try_from(color)?;
        Ok(Self::from_owned(RectLight {
            color,
            intensity,
            range,
            width,
            height,
        })
        .into())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |light| &light.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        let color = Color::try_from(color)?;
        self.as_mut()?.color = color;
        Ok(())
    }

    #[getter]
    pub fn intensity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.intensity)
    }

    #[setter]
    pub fn set_intensity(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.intensity = value;
        Ok(())
    }

    #[getter]
    pub fn range(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.range)
    }

    #[setter]
    pub fn set_range(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.range = value;
        Ok(())
    }

    #[getter]
    pub fn width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.width)
    }

    #[setter]
    pub fn set_width(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.width = value;
        Ok(())
    }

    #[getter]
    pub fn height(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.height)
    }

    #[setter]
    pub fn set_height(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.height = value;
        Ok(())
    }
}
