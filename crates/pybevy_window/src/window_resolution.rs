use bevy::window::WindowResolution;
use pybevy_core::{FieldStorage, FromBorrowedStorage};
use pybevy_macros::native_field;
use pybevy_math::{uvec2::PyUVec2, vec2::PyVec2};
use pyo3::{PyRefMut, prelude::*};

#[native_field]
#[pyclass(name = "WindowResolution")]
#[derive(Debug)]
pub struct PyWindowResolution {
    storage: FieldStorage<WindowResolution>,
}

impl Default for PyWindowResolution {
    fn default() -> Self {
        Self {
            storage: FieldStorage::owned(WindowResolution::default()),
        }
    }
}

#[pymethods]
impl PyWindowResolution {
    #[new]
    #[pyo3(signature = (width = 1280.0, height = 720.0, scale_factor_override = None))]
    pub fn new(width: f32, height: f32, scale_factor_override: Option<f32>) -> Self {
        let mut resolution = WindowResolution::new(width as u32, height as u32);
        if let Some(scale) = scale_factor_override {
            resolution.set_scale_factor_override(Some(scale));
        }
        Self::from_owned(resolution)
    }

    #[getter]
    pub fn width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.width())
    }

    #[getter]
    pub fn height(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.height())
    }

    #[getter]
    pub fn physical_width(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.physical_width())
    }

    #[getter]
    pub fn physical_height(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.physical_height())
    }

    #[getter]
    pub fn scale_factor(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.scale_factor() as f64)
    }

    pub fn set(&mut self, width: f32, height: f32) -> PyResult<()> {
        self.as_mut()?.set(width, height);
        Ok(())
    }

    pub fn set_physical_resolution(&mut self, width: u32, height: u32) -> PyResult<()> {
        self.as_mut()?.set_physical_resolution(width, height);
        Ok(())
    }

    pub fn set_scale_factor_override(
        &mut self,
        scale_factor_override: Option<f32>,
    ) -> PyResult<()> {
        self.as_mut()?
            .set_scale_factor_override(scale_factor_override);
        Ok(())
    }

    pub fn size(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.size().into())
    }

    pub fn physical_size(&self) -> PyResult<PyUVec2> {
        Ok(self.as_ref()?.physical_size().into())
    }

    #[getter]
    pub fn base_scale_factor(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.base_scale_factor())
    }

    pub fn scale_factor_override(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.scale_factor_override())
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) -> PyResult<()> {
        self.as_mut()?.set_scale_factor(scale_factor);
        Ok(())
    }

    pub fn with_scale_factor_override(
        mut slf: PyRefMut<'_, Self>,
        scale_factor_override: f32,
    ) -> PyResult<PyRefMut<'_, Self>> {
        slf.as_mut()?
            .set_scale_factor_override(Some(scale_factor_override));
        Ok(slf)
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let inner = self.as_ref()?;
        Ok(format!(
            "WindowResolution({}x{}, scale={})",
            inner.width(),
            inner.height(),
            inner.scale_factor()
        ))
    }
}
