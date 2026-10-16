use bevy::post_process::dof::DepthOfField;
use pybevy_camera::physical_camera_parameters::PyPhysicalCameraParameters;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::depth_of_field_mode::PyDepthOfFieldMode;

#[pycomponent(
    DepthOfField,
    bridge,
    domains = [
        focal_distance => [finite(view = checked)],
        sensor_height => [finite(view = checked)],
        aperture_f_stops => [finite(view = checked)],
        max_circle_of_confusion_diameter => [finite(view = checked)],
        max_depth => [finite_or_pos_inf(view = checked)],
    ],
    view_fields = [
        focal_distance,
        sensor_height,
        aperture_f_stops,
        max_circle_of_confusion_diameter,
        max_depth
    ]
)]
#[pyclass(name = "DepthOfField", module = "pybevy.post_process", extends = PyComponent)]
pub struct PyDepthOfField {
    pub(crate) storage: ComponentStorage<DepthOfField>,
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
        Self::validate_domains(&depth_of_field)?;
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
        let focal_distance = Self::check_focal_distance(focal_distance)?;
        self.as_mut()?.focal_distance = focal_distance;
        Ok(())
    }

    #[getter]
    pub fn sensor_height(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.sensor_height)
    }

    #[setter]
    pub fn set_sensor_height(&mut self, sensor_height: f32) -> PyResult<()> {
        let sensor_height = Self::check_sensor_height(sensor_height)?;
        self.as_mut()?.sensor_height = sensor_height;
        Ok(())
    }

    #[getter]
    pub fn aperture_f_stops(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.aperture_f_stops)
    }

    #[setter]
    pub fn set_aperture_f_stops(&mut self, aperture_f_stops: f32) -> PyResult<()> {
        let aperture_f_stops = Self::check_aperture_f_stops(aperture_f_stops)?;
        self.as_mut()?.aperture_f_stops = aperture_f_stops;
        Ok(())
    }

    #[getter]
    pub fn max_circle_of_confusion_diameter(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_circle_of_confusion_diameter)
    }

    #[setter]
    pub fn set_max_circle_of_confusion_diameter(&mut self, diameter: f32) -> PyResult<()> {
        let diameter = Self::check_max_circle_of_confusion_diameter(diameter)?;
        self.as_mut()?.max_circle_of_confusion_diameter = diameter;
        Ok(())
    }

    #[getter]
    pub fn max_depth(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_depth)
    }

    #[setter]
    pub fn set_max_depth(&mut self, max_depth: f32) -> PyResult<()> {
        let max_depth = Self::check_max_depth(max_depth)?;
        self.as_mut()?.max_depth = max_depth;
        Ok(())
    }
}
