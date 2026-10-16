use bevy::camera::Exposure;
use pybevy_core::{PyComponent, public_error, pycomponent::ComponentStorage};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::physical_camera_parameters::PyPhysicalCameraParameters;

#[pycomponent(Exposure, bridge, view_fields = [ev100 => [range(f32::MIN, f32::INFINITY, public_error::EXPOSURE_EV100_NON_FINITE)]])]
#[pyclass(name = "Exposure", module = "pybevy.camera", extends = PyComponent)]
pub struct PyExposure {
    pub(crate) storage: ComponentStorage<Exposure>,
}

impl PyExposure {
    fn preset(exposure: Exposure) -> PyResult<Py<Self>> {
        Python::attach(|py| Py::new(py, Self::from_owned(exposure)))
    }
}

#[pymethods]
impl PyExposure {
    #[new]
    #[pyo3(signature = (*, ev100 = Exposure::EV100_BLENDER))]
    pub fn new(ev100: f32) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(Exposure {
            ev100: validate_ev100(ev100)?,
        })
        .into())
    }

    #[staticmethod]
    #[pyo3(name = "SUNLIGHT")]
    pub fn sunlight() -> PyResult<Py<Self>> {
        Self::preset(Exposure::SUNLIGHT)
    }

    #[staticmethod]
    #[pyo3(name = "OVERCAST")]
    pub fn overcast() -> PyResult<Py<Self>> {
        Self::preset(Exposure::OVERCAST)
    }

    #[staticmethod]
    #[pyo3(name = "INDOOR")]
    pub fn indoor() -> PyResult<Py<Self>> {
        Self::preset(Exposure::INDOOR)
    }

    #[staticmethod]
    #[pyo3(name = "BLENDER")]
    pub fn blender() -> PyResult<Py<Self>> {
        Self::preset(Exposure::BLENDER)
    }

    #[classattr]
    pub const EV100_SUNLIGHT: f32 = Exposure::EV100_SUNLIGHT;

    #[classattr]
    pub const EV100_OVERCAST: f32 = Exposure::EV100_OVERCAST;

    #[classattr]
    pub const EV100_INDOOR: f32 = Exposure::EV100_INDOOR;

    #[classattr]
    pub const EV100_BLENDER: f32 = Exposure::EV100_BLENDER;

    #[getter]
    pub fn ev100(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.ev100)
    }

    #[setter]
    pub fn set_ev100(&mut self, ev100: f32) -> PyResult<()> {
        let ev100 = validate_ev100(ev100)?;
        self.as_mut()?.ev100 = ev100;
        Ok(())
    }

    pub fn exposure(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.exposure())
    }

    #[staticmethod]
    pub fn from_physical_camera(
        py: Python<'_>,
        physical_camera_parameters: &PyPhysicalCameraParameters,
    ) -> PyResult<Py<Self>> {
        let exposure = Exposure::from_physical_camera(physical_camera_parameters.into());
        validate_ev100(exposure.ev100)?;
        Py::new(py, Self::from_owned(exposure))
    }

    fn __repr__(&self) -> PyResult<String> {
        let ev100 = self.as_ref()?.ev100;
        Ok(format!("Exposure(ev100={:.1})", ev100))
    }
}

fn validate_ev100(ev100: f32) -> PyResult<f32> {
    if ev100.is_finite() {
        Ok(ev100)
    } else {
        Err(PyValueError::new_err(
            public_error::EXPOSURE_EV100_NON_FINITE,
        ))
    }
}
