use bevy::camera::PhysicalCameraParameters;
use pybevy_core::{public_error, repr::float_repr};
use pyo3::{exceptions::PyValueError, prelude::*};

fn validate_parameters(params: &PhysicalCameraParameters) -> PyResult<()> {
    for (name, value) in [
        ("aperture_f_stops", params.aperture_f_stops),
        ("shutter_speed_s", params.shutter_speed_s),
        ("sensitivity_iso", params.sensitivity_iso),
        ("sensor_height", params.sensor_height),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(PyValueError::new_err(
                public_error::physical_camera_parameter(name, value),
            ));
        }
    }
    let ev100 = params.ev100();
    if !ev100.is_finite() {
        return Err(PyValueError::new_err(public_error::physical_camera_ev100(
            ev100,
        )));
    }
    Ok(())
}

#[pyclass(
    name = "PhysicalCameraParameters",
    module = "pybevy.camera",
    skip_from_py_object
)]
#[derive(Clone, Copy, Default)]
pub struct PyPhysicalCameraParameters {
    pub(crate) inner: PhysicalCameraParameters,
}

impl From<PhysicalCameraParameters> for PyPhysicalCameraParameters {
    fn from(params: PhysicalCameraParameters) -> Self {
        Self { inner: params }
    }
}

impl From<PyPhysicalCameraParameters> for PhysicalCameraParameters {
    fn from(params: PyPhysicalCameraParameters) -> Self {
        params.inner
    }
}

impl From<&PyPhysicalCameraParameters> for PhysicalCameraParameters {
    fn from(params: &PyPhysicalCameraParameters) -> Self {
        params.inner
    }
}

#[pymethods]
impl PyPhysicalCameraParameters {
    #[new]
    #[pyo3(signature = (
        *,
        aperture_f_stops = 1.0,
        shutter_speed_s = 1.0 / 125.0,
        sensitivity_iso = 100.0,
        sensor_height = 0.01866
    ))]
    pub fn new(
        aperture_f_stops: f32,
        shutter_speed_s: f32,
        sensitivity_iso: f32,
        sensor_height: f32,
    ) -> PyResult<Self> {
        let inner = PhysicalCameraParameters {
            aperture_f_stops,
            shutter_speed_s,
            sensitivity_iso,
            sensor_height,
        };
        validate_parameters(&inner)?;
        Ok(Self { inner })
    }

    #[getter]
    pub fn aperture_f_stops(&self) -> f32 {
        self.inner.aperture_f_stops
    }

    #[setter]
    pub fn set_aperture_f_stops(&mut self, value: f32) -> PyResult<()> {
        let mut candidate = self.inner;
        candidate.aperture_f_stops = value;
        validate_parameters(&candidate)?;
        self.inner = candidate;
        Ok(())
    }

    #[getter]
    pub fn shutter_speed_s(&self) -> f32 {
        self.inner.shutter_speed_s
    }

    #[setter]
    pub fn set_shutter_speed_s(&mut self, value: f32) -> PyResult<()> {
        let mut candidate = self.inner;
        candidate.shutter_speed_s = value;
        validate_parameters(&candidate)?;
        self.inner = candidate;
        Ok(())
    }

    #[getter]
    pub fn sensitivity_iso(&self) -> f32 {
        self.inner.sensitivity_iso
    }

    #[setter]
    pub fn set_sensitivity_iso(&mut self, value: f32) -> PyResult<()> {
        let mut candidate = self.inner;
        candidate.sensitivity_iso = value;
        validate_parameters(&candidate)?;
        self.inner = candidate;
        Ok(())
    }

    #[getter]
    pub fn sensor_height(&self) -> f32 {
        self.inner.sensor_height
    }

    #[setter]
    pub fn set_sensor_height(&mut self, value: f32) -> PyResult<()> {
        let mut candidate = self.inner;
        candidate.sensor_height = value;
        validate_parameters(&candidate)?;
        self.inner = candidate;
        Ok(())
    }

    pub fn ev100(&self) -> f32 {
        self.inner.ev100()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "PhysicalCameraParameters(aperture_f_stops={}, shutter_speed_s={}, sensitivity_iso={}, sensor_height={})",
            float_repr(self.inner.aperture_f_stops),
            float_repr(self.inner.shutter_speed_s),
            float_repr(self.inner.sensitivity_iso),
            float_repr(self.inner.sensor_height)
        )
    }
}
