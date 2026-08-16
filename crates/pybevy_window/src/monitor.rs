use bevy::window::Monitor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::{ivec2::PyIVec2, uvec2::PyUVec2};
use pyo3::prelude::*;

use crate::video_mode::PyVideoMode;

#[pycomponent(Monitor, no_clone, bridge)]
#[pyclass(name = "Monitor", extends = PyComponent)]
pub struct PyMonitor {
    pub(crate) storage: ComponentStorage<Monitor>,
}

#[pymethods]
impl PyMonitor {
    #[getter]
    pub fn name(&self) -> PyResult<Option<String>> {
        Ok(self.as_ref()?.name.clone())
    }

    #[getter]
    pub fn physical_height(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.physical_height)
    }

    #[getter]
    pub fn physical_width(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.physical_width)
    }

    #[getter]
    pub fn physical_position(&self) -> PyResult<PyIVec2> {
        Ok(self
            .storage
            .snapshot_field_as(|monitor| &monitor.physical_position)?)
    }

    #[getter]
    pub fn refresh_rate_millihertz(&self) -> PyResult<Option<u32>> {
        Ok(self.as_ref()?.refresh_rate_millihertz)
    }

    #[getter]
    pub fn scale_factor(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.scale_factor)
    }

    #[getter]
    pub fn video_modes(&self) -> PyResult<Vec<PyVideoMode>> {
        Ok(self
            .as_ref()?
            .video_modes
            .iter()
            .map(|vm| (*vm).into())
            .collect())
    }

    pub fn physical_size(&self) -> PyResult<PyUVec2> {
        Ok(self.as_ref()?.physical_size().into())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let monitor = self.as_ref()?;
        Ok(format!(
            "Monitor(name={:?}, {}x{}, scale={})",
            monitor.name, monitor.physical_width, monitor.physical_height, monitor.scale_factor
        ))
    }
}
