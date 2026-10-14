use bevy::post_process::motion_blur::MotionBlur;
use pybevy_core::{ComponentStorage, PyComponent, domain::IntegerInput};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

// Each pixel takes 2 * samples + 1 taps; far larger counts stall the GPU.
#[pycomponent(
    MotionBlur,
    bridge,
    domains = [
        shutter_angle => [finite(view = checked)],
        samples => [range_inclusive(0.0, 1024.0)],
    ],
    view_fields = [shutter_angle, samples]
)]
#[pyclass(name = "MotionBlur", module = "pybevy.post_process", extends = PyComponent)]
pub struct PyMotionBlur {
    pub(crate) storage: ComponentStorage<MotionBlur>,
}

#[pymethods]
impl PyMotionBlur {
    #[new]
    #[pyo3(signature = (*, shutter_angle = 0.5, samples = IntegerInput::new(1)))]
    pub fn new(
        shutter_angle: f32,
        samples: IntegerInput<u32>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let motion_blur = MotionBlur {
            shutter_angle: Self::check_shutter_angle(shutter_angle)?,
            samples: Self::check_samples(samples)?.stored(),
        };
        Ok(Self::from_owned(motion_blur).into())
    }

    #[getter]
    pub fn shutter_angle(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.shutter_angle)
    }

    #[setter]
    pub fn set_shutter_angle(&mut self, shutter_angle: f32) -> PyResult<()> {
        let shutter_angle = Self::check_shutter_angle(shutter_angle)?;
        self.as_mut()?.shutter_angle = shutter_angle;
        Ok(())
    }

    #[getter]
    pub fn samples(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.samples)
    }

    #[setter]
    pub fn set_samples(&mut self, samples: IntegerInput<u32>) -> PyResult<()> {
        let samples = Self::check_samples(samples)?.stored();
        self.as_mut()?.samples = samples;
        Ok(())
    }
}
