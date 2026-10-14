use bevy::post_process::motion_blur::MotionBlur;
use pybevy_core::{ComponentStorage, PyComponent, StorageError, public_error};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(MotionBlur, bridge, view_fields = [
    shutter_angle => [range(f32::MIN, f32::INFINITY, public_error::MOTION_BLUR_SHUTTER_ANGLE_FINITE)],
    samples
])]
#[pyclass(name = "MotionBlur", module = "pybevy.post_process", extends = PyComponent)]
pub struct PyMotionBlur {
    pub(crate) storage: ComponentStorage<MotionBlur>,
}

fn check_shutter_angle(value: f32) -> Result<f32, StorageError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(StorageError::InvalidValue(
            public_error::MOTION_BLUR_SHUTTER_ANGLE_FINITE.to_string(),
        ))
    }
}

#[pymethods]
impl PyMotionBlur {
    #[new]
    #[pyo3(signature = (*, shutter_angle = 0.5, samples = 1))]
    pub fn new(shutter_angle: f32, samples: u32) -> PyResult<PyClassInitializer<Self>> {
        let motion_blur = MotionBlur {
            shutter_angle: check_shutter_angle(shutter_angle)?,
            samples,
        };
        Ok(Self::from_owned(motion_blur).into())
    }

    #[getter]
    pub fn shutter_angle(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.shutter_angle)
    }

    #[setter]
    pub fn set_shutter_angle(&mut self, shutter_angle: f32) -> PyResult<()> {
        let shutter_angle = check_shutter_angle(shutter_angle)?;
        self.as_mut()?.shutter_angle = shutter_angle;
        Ok(())
    }

    #[getter]
    pub fn samples(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.samples)
    }

    #[setter]
    pub fn set_samples(&mut self, samples: u32) -> PyResult<()> {
        self.as_mut()?.samples = samples;
        Ok(())
    }
}
