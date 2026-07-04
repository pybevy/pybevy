use bevy::light::CascadeShadowConfig;
use pybevy_core::{ComponentStorage, PyComponent, PyF32List};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyRuntimeError, prelude::*};

#[pycomponent(CascadeShadowConfig, bridge, view_fields = [overlap_proportion, minimum_distance])]
#[pyclass(name = "CascadeShadowConfig", extends = PyComponent)]
#[derive(Debug)]
pub struct PyCascadeShadowConfig {
    pub(crate) storage: ComponentStorage<CascadeShadowConfig>,
}

impl PyCascadeShadowConfig {
    fn default_bounds() -> Vec<f32> {
        CascadeShadowConfig::default().bounds
    }

    fn default_overlap_proportion() -> f32 {
        CascadeShadowConfig::default().overlap_proportion
    }

    fn default_minimum_distance() -> f32 {
        CascadeShadowConfig::default().minimum_distance
    }
}

#[pymethods]
impl PyCascadeShadowConfig {
    #[new]
    #[pyo3(signature = (
        bounds = Self::default_bounds(),
        overlap_proportion = Self::default_overlap_proportion(),
        minimum_distance = Self::default_minimum_distance()
    ))]
    pub fn new(
        bounds: Vec<f32>,
        overlap_proportion: f32,
        minimum_distance: f32,
    ) -> PyClassInitializer<Self> {
        Self::from_owned(CascadeShadowConfig {
            bounds,
            overlap_proportion,
            minimum_distance,
        }).into()
    }

    #[getter]
    pub fn bounds(&self) -> PyResult<PyF32List> {
        Ok(self.storage.borrow_field_as(|c| &c.bounds)?)
    }

    #[setter]
    pub fn set_bounds(&mut self, value: Vec<f32>) -> PyResult<()> {
        if value.is_empty() {
            return Err(PyRuntimeError::new_err("bounds cannot be empty"));
        }
        self.as_mut()?.bounds = value;
        Ok(())
    }

    #[getter]
    pub fn overlap_proportion(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.overlap_proportion)
    }

    #[setter]
    pub fn set_overlap_proportion(&mut self, value: f32) -> PyResult<()> {
        if !(0.0..1.0).contains(&value) {
            return Err(PyRuntimeError::new_err(
                "overlap_proportion must be in range [0.0, 1.0)",
            ));
        }
        self.as_mut()?.overlap_proportion = value;
        Ok(())
    }

    #[getter]
    pub fn minimum_distance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.minimum_distance)
    }

    #[setter]
    pub fn set_minimum_distance(&mut self, value: f32) -> PyResult<()> {
        if value < 0.0 {
            return Err(PyRuntimeError::new_err(
                "minimum_distance must be non-negative",
            ));
        }
        self.as_mut()?.minimum_distance = value;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let config = self.as_ref()?;
        Ok(format!(
            "CascadeShadowConfig(bounds={:?}, overlap_proportion={}, minimum_distance={})",
            config.bounds, config.overlap_proportion, config.minimum_distance
        ))
    }
}
