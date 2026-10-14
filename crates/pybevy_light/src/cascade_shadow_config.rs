use bevy::light::CascadeShadowConfig;
use pybevy_core::{
    ComponentStorage, PyComponent, PyFloatLiveList, public_error,
    repr::{float_list_repr, float_repr},
};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyValueError, prelude::*};

#[pycomponent(
    CascadeShadowConfig,
    bridge,
    domains = [
        overlap_proportion => [range(0.0, 1.0)],
        minimum_distance => [non_negative],
    ],
    view_fields = [overlap_proportion, minimum_distance]
)]
#[pyclass(name = "CascadeShadowConfig", module = "pybevy.light", extends = PyComponent)]
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
        *,
        bounds = Self::default_bounds(),
        overlap_proportion = Self::default_overlap_proportion(),
        minimum_distance = Self::default_minimum_distance()
    ))]
    pub fn new(
        bounds: Vec<f32>,
        overlap_proportion: f32,
        minimum_distance: f32,
    ) -> PyResult<PyClassInitializer<Self>> {
        if bounds.is_empty() {
            return Err(PyValueError::new_err(public_error::CASCADE_BOUNDS_EMPTY));
        }
        let overlap_proportion = Self::check_overlap_proportion(overlap_proportion)?;
        let minimum_distance = Self::check_minimum_distance(minimum_distance)?;
        Ok(Self::from_owned(CascadeShadowConfig {
            bounds,
            overlap_proportion,
            minimum_distance,
        })
        .into())
    }

    #[getter]
    pub fn bounds(&self) -> PyResult<PyFloatLiveList> {
        let bounds: PyFloatLiveList = self.storage.borrow_field_as(|c| &c.bounds)?;
        Ok(bounds.with_minimum_length(1, public_error::CASCADE_BOUNDS_EMPTY))
    }

    #[setter]
    pub fn set_bounds(&mut self, value: Vec<f32>) -> PyResult<()> {
        if value.is_empty() {
            return Err(PyValueError::new_err(public_error::CASCADE_BOUNDS_EMPTY));
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
        let value = Self::check_overlap_proportion(value)?;
        self.as_mut()?.overlap_proportion = value;
        Ok(())
    }

    #[getter]
    pub fn minimum_distance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.minimum_distance)
    }

    #[setter]
    pub fn set_minimum_distance(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_minimum_distance(value)?;
        self.as_mut()?.minimum_distance = value;
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let config = self.as_ref()?;
        Ok(format!(
            "CascadeShadowConfig(bounds={}, overlap_proportion={}, minimum_distance={})",
            float_list_repr(&config.bounds),
            float_repr(config.overlap_proportion),
            float_repr(config.minimum_distance)
        ))
    }
}
