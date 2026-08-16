use bevy::camera::PhysicalCameraParameters;
use pyo3::prelude::*;

#[pyclass(name = "PhysicalCameraParameters", skip_from_py_object)]
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
    ) -> Self {
        Self {
            inner: PhysicalCameraParameters {
                aperture_f_stops,
                shutter_speed_s,
                sensitivity_iso,
                sensor_height,
            },
        }
    }

    #[getter]
    pub fn aperture_f_stops(&self) -> f32 {
        self.inner.aperture_f_stops
    }

    #[setter]
    pub fn set_aperture_f_stops(&mut self, value: f32) {
        self.inner.aperture_f_stops = value;
    }

    #[getter]
    pub fn shutter_speed_s(&self) -> f32 {
        self.inner.shutter_speed_s
    }

    #[setter]
    pub fn set_shutter_speed_s(&mut self, value: f32) {
        self.inner.shutter_speed_s = value;
    }

    #[getter]
    pub fn sensitivity_iso(&self) -> f32 {
        self.inner.sensitivity_iso
    }

    #[setter]
    pub fn set_sensitivity_iso(&mut self, value: f32) {
        self.inner.sensitivity_iso = value;
    }

    #[getter]
    pub fn sensor_height(&self) -> f32 {
        self.inner.sensor_height
    }

    #[setter]
    pub fn set_sensor_height(&mut self, value: f32) {
        self.inner.sensor_height = value;
    }

    pub fn ev100(&self) -> f32 {
        self.inner.ev100()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "PhysicalCameraParameters(aperture_f_stops={}, shutter_speed_s={}, sensitivity_iso={}, sensor_height={})",
            self.inner.aperture_f_stops,
            self.inner.shutter_speed_s,
            self.inner.sensitivity_iso,
            self.inner.sensor_height
        )
    }
}
