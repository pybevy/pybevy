use bevy::pbr::AtmosphereSettings;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::{uvec2::PyUVec2, uvec3::PyUVec3};
use pyo3::prelude::*;

use crate::atmosphere_mode::PyAtmosphereMode;

#[pycomponent(AtmosphereSettings, bridge, view_fields = [
    transmittance_lut_samples,
    multiscattering_lut_dirs,
    multiscattering_lut_samples,
    sky_view_lut_samples,
    aerial_view_lut_samples,
    aerial_view_lut_max_distance,
    scene_units_to_m,
    sky_max_samples
])]
#[pyclass(name = "AtmosphereSettings", extends = PyComponent)]
#[derive(Clone)]
pub struct PyAtmosphereSettings {
    pub(crate) storage: ComponentStorage<AtmosphereSettings>,
}

#[pymethods]
impl PyAtmosphereSettings {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        transmittance_lut_size = PyUVec2::new(256, 128),
        multiscattering_lut_size = PyUVec2::new(32, 32),
        sky_view_lut_size = PyUVec2::new(400, 200),
        aerial_view_lut_size = PyUVec3::new(32, 32, 32),
        transmittance_lut_samples = 40,
        multiscattering_lut_dirs = 64,
        multiscattering_lut_samples = 20,
        sky_view_lut_samples = 16,
        aerial_view_lut_samples = 10,
        aerial_view_lut_max_distance = 3.2e4,
        scene_units_to_m = 1.0,
        sky_max_samples = 16,
        rendering_method = PyAtmosphereMode::LookupTexture,
    ))]
    pub fn new(
        transmittance_lut_size: PyUVec2,
        multiscattering_lut_size: PyUVec2,
        sky_view_lut_size: PyUVec2,
        aerial_view_lut_size: PyUVec3,
        transmittance_lut_samples: u32,
        multiscattering_lut_dirs: u32,
        multiscattering_lut_samples: u32,
        sky_view_lut_samples: u32,
        aerial_view_lut_samples: u32,
        aerial_view_lut_max_distance: f32,
        scene_units_to_m: f32,
        sky_max_samples: u32,
        rendering_method: PyAtmosphereMode,
    ) -> (Self, PyComponent) {
        Self::from_owned(AtmosphereSettings {
            transmittance_lut_size: transmittance_lut_size.into(),
            multiscattering_lut_size: multiscattering_lut_size.into(),
            sky_view_lut_size: sky_view_lut_size.into(),
            aerial_view_lut_size: aerial_view_lut_size.into(),
            transmittance_lut_samples,
            multiscattering_lut_dirs,
            multiscattering_lut_samples,
            sky_view_lut_samples,
            aerial_view_lut_samples,
            aerial_view_lut_max_distance,
            scene_units_to_m,
            sky_max_samples,
            rendering_method: rendering_method.into(),
        })
    }

    #[getter]
    pub fn transmittance_lut_size(&self) -> PyResult<PyUVec2> {
        Ok(self
            .storage
            .borrow_field_as(|s| &s.transmittance_lut_size)?)
    }

    #[setter]
    pub fn set_transmittance_lut_size(&mut self, value: PyUVec2) -> PyResult<()> {
        self.as_mut()?.transmittance_lut_size = value.into();
        Ok(())
    }

    #[getter]
    pub fn multiscattering_lut_size(&self) -> PyResult<PyUVec2> {
        Ok(self
            .storage
            .borrow_field_as(|s| &s.multiscattering_lut_size)?)
    }

    #[setter]
    pub fn set_multiscattering_lut_size(&mut self, value: PyUVec2) -> PyResult<()> {
        self.as_mut()?.multiscattering_lut_size = value.into();
        Ok(())
    }

    #[getter]
    pub fn sky_view_lut_size(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.sky_view_lut_size)?)
    }

    #[setter]
    pub fn set_sky_view_lut_size(&mut self, value: PyUVec2) -> PyResult<()> {
        self.as_mut()?.sky_view_lut_size = value.into();
        Ok(())
    }

    #[getter]
    pub fn aerial_view_lut_size(&self) -> PyResult<PyUVec3> {
        Ok(self.storage.borrow_field_as(|s| &s.aerial_view_lut_size)?)
    }

    #[setter]
    pub fn set_aerial_view_lut_size(&mut self, value: PyUVec3) -> PyResult<()> {
        self.as_mut()?.aerial_view_lut_size = value.into();
        Ok(())
    }

    #[getter]
    pub fn transmittance_lut_samples(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.transmittance_lut_samples)
    }

    #[setter]
    pub fn set_transmittance_lut_samples(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.transmittance_lut_samples = value;
        Ok(())
    }

    #[getter]
    pub fn multiscattering_lut_dirs(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.multiscattering_lut_dirs)
    }

    #[setter]
    pub fn set_multiscattering_lut_dirs(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.multiscattering_lut_dirs = value;
        Ok(())
    }

    #[getter]
    pub fn multiscattering_lut_samples(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.multiscattering_lut_samples)
    }

    #[setter]
    pub fn set_multiscattering_lut_samples(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.multiscattering_lut_samples = value;
        Ok(())
    }

    #[getter]
    pub fn sky_view_lut_samples(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.sky_view_lut_samples)
    }

    #[setter]
    pub fn set_sky_view_lut_samples(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.sky_view_lut_samples = value;
        Ok(())
    }

    #[getter]
    pub fn aerial_view_lut_samples(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.aerial_view_lut_samples)
    }

    #[setter]
    pub fn set_aerial_view_lut_samples(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.aerial_view_lut_samples = value;
        Ok(())
    }

    #[getter]
    pub fn aerial_view_lut_max_distance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.aerial_view_lut_max_distance)
    }

    #[setter]
    pub fn set_aerial_view_lut_max_distance(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.aerial_view_lut_max_distance = value;
        Ok(())
    }

    #[getter]
    pub fn scene_units_to_m(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.scene_units_to_m)
    }

    #[setter]
    pub fn set_scene_units_to_m(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.scene_units_to_m = value;
        Ok(())
    }

    #[getter]
    pub fn sky_max_samples(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.sky_max_samples)
    }

    #[setter]
    pub fn set_sky_max_samples(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.sky_max_samples = value;
        Ok(())
    }

    #[getter]
    pub fn rendering_method(&self) -> PyResult<PyAtmosphereMode> {
        Ok(self.as_ref()?.rendering_method.into())
    }

    #[setter]
    pub fn set_rendering_method(&mut self, value: PyAtmosphereMode) -> PyResult<()> {
        self.as_mut()?.rendering_method = value.into();
        Ok(())
    }
}
