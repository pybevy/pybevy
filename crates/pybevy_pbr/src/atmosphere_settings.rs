use bevy::{
    math::{UVec2, UVec3},
    pbr::AtmosphereSettings,
};
use pybevy_core::{ComponentStorage, PyComponent, domain::IntegerInput};
use pybevy_macros::pycomponent;
use pybevy_math::{uvec2::PyUVec2, uvec3::PyUVec3};
use pyo3::prelude::*;

use crate::atmosphere_mode::PyAtmosphereMode;

// Bevy creates the lookup textures with these sizes; wgpu rejects a zero side.
#[pycomponent(AtmosphereSettings, bridge, view_fields = [
    transmittance_lut_samples,
    multiscattering_lut_dirs,
    multiscattering_lut_samples,
    sky_view_lut_samples,
    aerial_view_lut_samples,
    aerial_view_lut_max_distance,
    sky_max_samples
], domains = [
    transmittance_lut_size => [range_inclusive(1.0, 8192.0)],
    multiscattering_lut_size => [range_inclusive(1.0, 8192.0)],
    sky_view_lut_size => [range_inclusive(1.0, 8192.0)],
    aerial_view_lut_size => [range_inclusive(1.0, 2048.0)],
    transmittance_lut_samples => [range_inclusive(0.0, 1024.0)],
    multiscattering_lut_dirs => [range_inclusive(0.0, 1024.0)],
    multiscattering_lut_samples => [range_inclusive(0.0, 1024.0)],
    sky_view_lut_samples => [range_inclusive(0.0, 1024.0)],
    aerial_view_lut_samples => [range_inclusive(0.0, 1024.0)],
    sky_max_samples => [range_inclusive(0.0, 1024.0)],
    aerial_view_lut_max_distance => [finite],
])]
#[pyclass(name = "AtmosphereSettings", module = "pybevy.pbr", extends = PyComponent)]
pub struct PyAtmosphereSettings {
    pub(crate) storage: ComponentStorage<AtmosphereSettings>,
}

#[pymethods]
impl PyAtmosphereSettings {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        *,
        transmittance_lut_size = PyUVec2::new(256, 128),
        multiscattering_lut_size = PyUVec2::new(32, 32),
        sky_view_lut_size = PyUVec2::new(400, 200),
        aerial_view_lut_size = PyUVec3::new(32, 32, 32),
        transmittance_lut_samples = IntegerInput::new(40),
        multiscattering_lut_dirs = IntegerInput::new(64),
        multiscattering_lut_samples = IntegerInput::new(20),
        sky_view_lut_samples = IntegerInput::new(16),
        aerial_view_lut_samples = IntegerInput::new(10),
        aerial_view_lut_max_distance = 3.2e4,
        sky_max_samples = IntegerInput::new(16),
        rendering_method = PyAtmosphereMode::LookupTexture
    ))]
    pub fn new(
        transmittance_lut_size: PyUVec2,
        multiscattering_lut_size: PyUVec2,
        sky_view_lut_size: PyUVec2,
        aerial_view_lut_size: PyUVec3,
        transmittance_lut_samples: IntegerInput<u32>,
        multiscattering_lut_dirs: IntegerInput<u32>,
        multiscattering_lut_samples: IntegerInput<u32>,
        sky_view_lut_samples: IntegerInput<u32>,
        aerial_view_lut_samples: IntegerInput<u32>,
        aerial_view_lut_max_distance: f32,
        sky_max_samples: IntegerInput<u32>,
        rendering_method: PyAtmosphereMode,
    ) -> PyResult<PyClassInitializer<Self>> {
        let aerial_view_lut_max_distance =
            Self::check_aerial_view_lut_max_distance(aerial_view_lut_max_distance)?;
        let transmittance_lut_samples =
            Self::check_transmittance_lut_samples(transmittance_lut_samples)?.stored();
        let multiscattering_lut_dirs =
            Self::check_multiscattering_lut_dirs(multiscattering_lut_dirs)?.stored();
        let multiscattering_lut_samples =
            Self::check_multiscattering_lut_samples(multiscattering_lut_samples)?.stored();
        let sky_view_lut_samples = Self::check_sky_view_lut_samples(sky_view_lut_samples)?.stored();
        let aerial_view_lut_samples =
            Self::check_aerial_view_lut_samples(aerial_view_lut_samples)?.stored();
        let sky_max_samples = Self::check_sky_max_samples(sky_max_samples)?.stored();
        Ok(Self::from_owned(AtmosphereSettings {
            transmittance_lut_size: Self::check_transmittance_lut_size(UVec2::try_from(
                transmittance_lut_size,
            )?)?,
            multiscattering_lut_size: Self::check_multiscattering_lut_size(UVec2::try_from(
                multiscattering_lut_size,
            )?)?,
            sky_view_lut_size: Self::check_sky_view_lut_size(UVec2::try_from(sky_view_lut_size)?)?,
            aerial_view_lut_size: Self::check_aerial_view_lut_size(UVec3::try_from(
                aerial_view_lut_size,
            )?)?,
            transmittance_lut_samples,
            multiscattering_lut_dirs,
            multiscattering_lut_samples,
            sky_view_lut_samples,
            aerial_view_lut_samples,
            aerial_view_lut_max_distance,
            sky_max_samples,
            rendering_method: rendering_method.into(),
        })
        .into())
    }

    #[getter]
    pub fn transmittance_lut_size(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |s| &s.transmittance_lut_size,
            |s| &mut s.transmittance_lut_size,
            Self::validate_transmittance_lut_size,
        )?)
    }

    #[setter]
    pub fn set_transmittance_lut_size(&mut self, value: PyUVec2) -> PyResult<()> {
        self.as_mut()?.transmittance_lut_size =
            Self::check_transmittance_lut_size(UVec2::try_from(value)?)?;
        Ok(())
    }

    #[getter]
    pub fn multiscattering_lut_size(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |s| &s.multiscattering_lut_size,
            |s| &mut s.multiscattering_lut_size,
            Self::validate_multiscattering_lut_size,
        )?)
    }

    #[setter]
    pub fn set_multiscattering_lut_size(&mut self, value: PyUVec2) -> PyResult<()> {
        self.as_mut()?.multiscattering_lut_size =
            Self::check_multiscattering_lut_size(UVec2::try_from(value)?)?;
        Ok(())
    }

    #[getter]
    pub fn sky_view_lut_size(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |s| &s.sky_view_lut_size,
            |s| &mut s.sky_view_lut_size,
            Self::validate_sky_view_lut_size,
        )?)
    }

    #[setter]
    pub fn set_sky_view_lut_size(&mut self, value: PyUVec2) -> PyResult<()> {
        self.as_mut()?.sky_view_lut_size = Self::check_sky_view_lut_size(UVec2::try_from(value)?)?;
        Ok(())
    }

    #[getter]
    pub fn aerial_view_lut_size(&self) -> PyResult<PyUVec3> {
        Ok(self.storage.borrow_validated_field_as(
            |s| &s.aerial_view_lut_size,
            |s| &mut s.aerial_view_lut_size,
            Self::validate_aerial_view_lut_size,
        )?)
    }

    #[setter]
    pub fn set_aerial_view_lut_size(&mut self, value: PyUVec3) -> PyResult<()> {
        self.as_mut()?.aerial_view_lut_size =
            Self::check_aerial_view_lut_size(UVec3::try_from(value)?)?;
        Ok(())
    }

    #[getter]
    pub fn transmittance_lut_samples(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.transmittance_lut_samples)
    }

    #[setter]
    pub fn set_transmittance_lut_samples(&mut self, value: IntegerInput<u32>) -> PyResult<()> {
        let value = Self::check_transmittance_lut_samples(value)?.stored();
        self.as_mut()?.transmittance_lut_samples = value;
        Ok(())
    }

    #[getter]
    pub fn multiscattering_lut_dirs(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.multiscattering_lut_dirs)
    }

    #[setter]
    pub fn set_multiscattering_lut_dirs(&mut self, value: IntegerInput<u32>) -> PyResult<()> {
        let value = Self::check_multiscattering_lut_dirs(value)?.stored();
        self.as_mut()?.multiscattering_lut_dirs = value;
        Ok(())
    }

    #[getter]
    pub fn multiscattering_lut_samples(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.multiscattering_lut_samples)
    }

    #[setter]
    pub fn set_multiscattering_lut_samples(&mut self, value: IntegerInput<u32>) -> PyResult<()> {
        let value = Self::check_multiscattering_lut_samples(value)?.stored();
        self.as_mut()?.multiscattering_lut_samples = value;
        Ok(())
    }

    #[getter]
    pub fn sky_view_lut_samples(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.sky_view_lut_samples)
    }

    #[setter]
    pub fn set_sky_view_lut_samples(&mut self, value: IntegerInput<u32>) -> PyResult<()> {
        let value = Self::check_sky_view_lut_samples(value)?.stored();
        self.as_mut()?.sky_view_lut_samples = value;
        Ok(())
    }

    #[getter]
    pub fn aerial_view_lut_samples(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.aerial_view_lut_samples)
    }

    #[setter]
    pub fn set_aerial_view_lut_samples(&mut self, value: IntegerInput<u32>) -> PyResult<()> {
        let value = Self::check_aerial_view_lut_samples(value)?.stored();
        self.as_mut()?.aerial_view_lut_samples = value;
        Ok(())
    }

    #[getter]
    pub fn aerial_view_lut_max_distance(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.aerial_view_lut_max_distance)
    }

    #[setter]
    pub fn set_aerial_view_lut_max_distance(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_aerial_view_lut_max_distance(value)?;
        self.as_mut()?.aerial_view_lut_max_distance = value;
        Ok(())
    }

    #[getter]
    pub fn sky_max_samples(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.sky_max_samples)
    }

    #[setter]
    pub fn set_sky_max_samples(&mut self, value: IntegerInput<u32>) -> PyResult<()> {
        let value = Self::check_sky_max_samples(value)?.stored();
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
