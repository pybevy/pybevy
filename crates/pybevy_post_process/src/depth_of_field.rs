use bevy::post_process::dof::DepthOfField;
use pybevy_camera::physical_camera_parameters::PyPhysicalCameraParameters;
use pybevy_core::{ComponentStorage, PyComponent, StorageError, public_error};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::depth_of_field_mode::PyDepthOfFieldMode;

#[pycomponent(DepthOfField, bridge, view_fields = [
    focal_distance => [range(f32::MIN, f32::INFINITY, public_error::DEPTH_OF_FIELD_FOCAL_DISTANCE_FINITE)],
    sensor_height => [range(f32::MIN, f32::INFINITY, public_error::DEPTH_OF_FIELD_SENSOR_HEIGHT_FINITE)],
    aperture_f_stops => [range(f32::MIN, f32::INFINITY, public_error::DEPTH_OF_FIELD_APERTURE_F_STOPS_FINITE)],
    max_circle_of_confusion_diameter => [range(f32::MIN, f32::INFINITY, public_error::DEPTH_OF_FIELD_MAX_COC_DIAMETER_FINITE)],
    max_depth => [range_inclusive(f32::MIN, f32::INFINITY, public_error::DEPTH_OF_FIELD_MAX_DEPTH_NOT_NAN_OR_NEG_INF)]
])]
#[pyclass(name = "DepthOfField", module = "pybevy.post_process", extends = PyComponent)]
pub struct PyDepthOfField {
    pub(crate) storage: ComponentStorage<DepthOfField>,
}

fn check_finite(value: f32, message: &'static str) -> Result<f32, StorageError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(StorageError::InvalidValue(message.to_string()))
    }
}

fn check_max_depth(value: f32) -> Result<f32, StorageError> {
    if value.is_finite() || value == f32::INFINITY {
        Ok(value)
    } else {
        Err(StorageError::InvalidValue(
            public_error::DEPTH_OF_FIELD_MAX_DEPTH_NOT_NAN_OR_NEG_INF.to_string(),
        ))
    }
}

fn validate(depth_of_field: &DepthOfField) -> Result<(), StorageError> {
    check_finite(
        depth_of_field.focal_distance,
        public_error::DEPTH_OF_FIELD_FOCAL_DISTANCE_FINITE,
    )?;
    check_finite(
        depth_of_field.sensor_height,
        public_error::DEPTH_OF_FIELD_SENSOR_HEIGHT_FINITE,
    )?;
    check_finite(
        depth_of_field.aperture_f_stops,
        public_error::DEPTH_OF_FIELD_APERTURE_F_STOPS_FINITE,
    )?;
    check_finite(
        depth_of_field.max_circle_of_confusion_diameter,
        public_error::DEPTH_OF_FIELD_MAX_COC_DIAMETER_FINITE,
    )?;
    check_max_depth(depth_of_field.max_depth)?;
    Ok(())
}

#[pymethods]
impl PyDepthOfField {
    #[new]
    #[pyo3(signature = (
        *,
        mode = PyDepthOfFieldMode::Gaussian,
        focal_distance = 10.0,
        sensor_height = 0.01866,
        aperture_f_stops = 1.0,
        max_circle_of_confusion_diameter = 64.0,
        max_depth = f32::INFINITY
    ))]
    pub fn new(
        mode: PyDepthOfFieldMode,
        focal_distance: f32,
        sensor_height: f32,
        aperture_f_stops: f32,
        max_circle_of_confusion_diameter: f32,
        max_depth: f32,
    ) -> PyResult<PyClassInitializer<Self>> {
        let depth_of_field = DepthOfField {
            mode: mode.into(),
            focal_distance,
            sensor_height,
            aperture_f_stops,
            max_circle_of_confusion_diameter,
            max_depth,
        };
        validate(&depth_of_field)?;
        Ok(Self::from_owned(depth_of_field).into())
    }

    #[staticmethod]
    pub fn from_physical_camera(
        py: Python<'_>,
        camera: &PyPhysicalCameraParameters,
    ) -> PyResult<Py<Self>> {
        let depth_of_field = DepthOfField::from_physical_camera(&camera.into());
        Py::new(py, Self::from_owned(depth_of_field))
    }

    #[getter]
    pub fn mode(&self) -> PyResult<PyDepthOfFieldMode> {
        Ok(self.as_ref()?.mode.into())
    }

    #[setter]
    pub fn set_mode(&mut self, mode: PyDepthOfFieldMode) -> PyResult<()> {
        self.as_mut()?.mode = mode.into();
        Ok(())
    }

    #[getter]
    pub fn focal_distance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.focal_distance)
    }

    #[setter]
    pub fn set_focal_distance(&mut self, focal_distance: f32) -> PyResult<()> {
        let focal_distance = check_finite(
            focal_distance,
            public_error::DEPTH_OF_FIELD_FOCAL_DISTANCE_FINITE,
        )?;
        self.as_mut()?.focal_distance = focal_distance;
        Ok(())
    }

    #[getter]
    pub fn sensor_height(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.sensor_height)
    }

    #[setter]
    pub fn set_sensor_height(&mut self, sensor_height: f32) -> PyResult<()> {
        let sensor_height = check_finite(
            sensor_height,
            public_error::DEPTH_OF_FIELD_SENSOR_HEIGHT_FINITE,
        )?;
        self.as_mut()?.sensor_height = sensor_height;
        Ok(())
    }

    #[getter]
    pub fn aperture_f_stops(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.aperture_f_stops)
    }

    #[setter]
    pub fn set_aperture_f_stops(&mut self, aperture_f_stops: f32) -> PyResult<()> {
        let aperture_f_stops = check_finite(
            aperture_f_stops,
            public_error::DEPTH_OF_FIELD_APERTURE_F_STOPS_FINITE,
        )?;
        self.as_mut()?.aperture_f_stops = aperture_f_stops;
        Ok(())
    }

    #[getter]
    pub fn max_circle_of_confusion_diameter(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_circle_of_confusion_diameter)
    }

    #[setter]
    pub fn set_max_circle_of_confusion_diameter(&mut self, diameter: f32) -> PyResult<()> {
        let diameter = check_finite(
            diameter,
            public_error::DEPTH_OF_FIELD_MAX_COC_DIAMETER_FINITE,
        )?;
        self.as_mut()?.max_circle_of_confusion_diameter = diameter;
        Ok(())
    }

    #[getter]
    pub fn max_depth(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_depth)
    }

    #[setter]
    pub fn set_max_depth(&mut self, max_depth: f32) -> PyResult<()> {
        let max_depth = check_max_depth(max_depth)?;
        self.as_mut()?.max_depth = max_depth;
        Ok(())
    }
}
