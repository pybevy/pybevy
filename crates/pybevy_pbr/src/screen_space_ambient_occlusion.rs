use bevy::pbr::ScreenSpaceAmbientOcclusion;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::ssao_quality_level::PyScreenSpaceAmbientOcclusionQualityLevel;

#[pycomponent(ScreenSpaceAmbientOcclusion, bridge, view_fields = [constant_object_thickness])]
#[pyclass(name = "ScreenSpaceAmbientOcclusion", extends = PyComponent)]
#[derive(Debug)]
pub struct PyScreenSpaceAmbientOcclusion {
    pub(crate) storage: ComponentStorage<ScreenSpaceAmbientOcclusion>,
}

#[pymethods]
impl PyScreenSpaceAmbientOcclusion {
    #[new]
    #[pyo3(signature = (
        quality_level = PyScreenSpaceAmbientOcclusionQualityLevel::Medium(),
        constant_object_thickness = 0.25
    ))]
    pub fn new(
        quality_level: PyScreenSpaceAmbientOcclusionQualityLevel,
        constant_object_thickness: f32,
    ) -> PyClassInitializer<Self> {
        Self::from_owned(ScreenSpaceAmbientOcclusion {
            quality_level: quality_level.into(),
            constant_object_thickness,
        })
        .into()
    }

    #[getter]
    pub fn quality_level(&self) -> PyResult<PyScreenSpaceAmbientOcclusionQualityLevel> {
        Ok(self.as_ref()?.quality_level.into())
    }

    #[setter]
    pub fn set_quality_level(
        &mut self,
        level: PyScreenSpaceAmbientOcclusionQualityLevel,
    ) -> PyResult<()> {
        self.as_mut()?.quality_level = level.into();
        Ok(())
    }

    #[getter]
    pub fn constant_object_thickness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.constant_object_thickness)
    }

    #[setter]
    pub fn set_constant_object_thickness(&mut self, thickness: f32) -> PyResult<()> {
        self.as_mut()?.constant_object_thickness = thickness;
        Ok(())
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
