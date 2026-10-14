use bevy::window::WindowResolution;
use pybevy_core::{
    FieldStorage, FromBorrowedStorage,
    domain::{DomainViolation, FINITE_NON_NEGATIVE, POSITIVE, check_for},
    repr::float_repr,
};
use pybevy_macros::pyfield;
use pybevy_math::{uvec2::PyUVec2, vec2::PyVec2};
use pyo3::prelude::*;

/// winit asserts that every scale factor is positive and normal when it converts
/// sizes, so a zero, negative or non-finite one panics at window creation or on
/// input events.
pub(crate) fn validate_scale_factors(resolution: &WindowResolution) -> Result<(), DomainViolation> {
    check_for::<PyWindowResolution, _>("scale_factor", resolution.base_scale_factor(), POSITIVE)?;
    check_for::<PyWindowResolution, _>(
        "scale_factor_override",
        resolution.scale_factor_override(),
        POSITIVE,
    )?;
    Ok(())
}

#[pyfield]
#[pyclass(name = "WindowResolution", module = "pybevy.window", from_py_object)]
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
    #[pyo3(signature = (physical_width = 1280, physical_height = 720, *, scale_factor_override = None))]
    pub fn new(
        physical_width: u32,
        physical_height: u32,
        scale_factor_override: Option<f32>,
    ) -> PyResult<Self> {
        let scale_factor_override =
            check_for::<Self, _>("scale_factor_override", scale_factor_override, POSITIVE)?;
        let mut resolution = WindowResolution::new(physical_width, physical_height);
        if let Some(scale) = scale_factor_override {
            resolution.set_scale_factor_override(Some(scale));
        }
        Ok(Self::from_owned(resolution))
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
        // An infinite logical size becomes a surface larger than wgpu accepts.
        let width = check_for::<Self, _>("width", width, FINITE_NON_NEGATIVE)?;
        let height = check_for::<Self, _>("height", height, FINITE_NON_NEGATIVE)?;
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
        let scale_factor_override =
            check_for::<Self, _>("scale_factor_override", scale_factor_override, POSITIVE)?;
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
        let scale_factor = check_for::<Self, _>("scale_factor", scale_factor, POSITIVE)?;
        self.as_mut()?.set_scale_factor(scale_factor);
        Ok(())
    }

    pub fn with_scale_factor_override(&self, scale_factor_override: f32) -> PyResult<Self> {
        let scale_factor_override =
            check_for::<Self, _>("scale_factor_override", scale_factor_override, POSITIVE)?;
        let mut resolution = self.as_ref()?.clone();
        resolution.set_scale_factor_override(Some(scale_factor_override));
        Ok(Self::from_owned(resolution))
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
            float_repr(inner.scale_factor())
        ))
    }
}
