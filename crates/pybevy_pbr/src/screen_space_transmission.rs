use bevy::pbr::ScreenSpaceTransmission;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::screen_space_transmission_quality::PyScreenSpaceTransmissionQuality;

#[pycomponent(ScreenSpaceTransmission, bridge)]
#[pyclass(name = "ScreenSpaceTransmission", extends = PyComponent)]
pub struct PyScreenSpaceTransmission {
    pub(crate) storage: ComponentStorage<ScreenSpaceTransmission>,
}

#[pymethods]
impl PyScreenSpaceTransmission {
    #[new]
    #[pyo3(signature = (
        steps = 1,
        quality = PyScreenSpaceTransmissionQuality::Medium
    ))]
    pub fn new(
        steps: usize,
        quality: PyScreenSpaceTransmissionQuality,
    ) -> PyClassInitializer<Self> {
        Self::from_owned(ScreenSpaceTransmission {
            steps,
            quality: quality.into(),
        })
        .into()
    }

    #[getter]
    pub fn steps(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.steps)
    }

    #[setter]
    pub fn set_steps(&mut self, steps: usize) -> PyResult<()> {
        self.as_mut()?.steps = steps;
        Ok(())
    }

    #[getter]
    pub fn quality(&self) -> PyResult<PyScreenSpaceTransmissionQuality> {
        Ok(self.as_ref()?.quality.into())
    }

    #[setter]
    pub fn set_quality(&mut self, quality: PyScreenSpaceTransmissionQuality) -> PyResult<()> {
        self.as_mut()?.quality = quality.into();
        Ok(())
    }
}
