use bevy::pbr::ScreenSpaceTransmission;
use pybevy_core::{ComponentStorage, PyComponent, domain::IntegerInput};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::screen_space_transmission_quality::PyScreenSpaceTransmissionQuality;

// Each step is a full-screen copy and pass, up to one per transmissive mesh; cap the stall.
#[pycomponent(
    ScreenSpaceTransmission,
    bridge,
    domains = [steps => [range_inclusive(0.0, 1024.0)]]
)]
#[pyclass(name = "ScreenSpaceTransmission", module = "pybevy.pbr", extends = PyComponent)]
pub struct PyScreenSpaceTransmission {
    pub(crate) storage: ComponentStorage<ScreenSpaceTransmission>,
}

#[pymethods]
impl PyScreenSpaceTransmission {
    #[new]
    #[pyo3(signature = (
        *,
        steps = IntegerInput::new(1),
        quality = PyScreenSpaceTransmissionQuality::Medium
    ))]
    pub fn new(
        steps: IntegerInput<usize>,
        quality: PyScreenSpaceTransmissionQuality,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(ScreenSpaceTransmission {
            steps: Self::check_steps(steps)?.stored(),
            quality: quality.into(),
        })
        .into())
    }

    #[getter]
    pub fn steps(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.steps)
    }

    #[setter]
    pub fn set_steps(&mut self, steps: IntegerInput<usize>) -> PyResult<()> {
        let steps = Self::check_steps(steps)?.stored();
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
