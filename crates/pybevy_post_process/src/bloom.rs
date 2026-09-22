use bevy::{
    math::Vec2,
    post_process::bloom::{Bloom, BloomPrefilter},
};
use numpy::PyReadonlyArray2;
use pybevy_core::{ComponentStorage, PyComponent, PyRustComponentBatch, public_error};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::{PyVec2, extract_vec2_from_any};
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{bloom_composite_mode::PyBloomCompositeMode, bloom_prefilter::PyBloomPrefilter};

fn validate_max_mip_dimension(dimension: u32) -> PyResult<()> {
    if dimension == 0 {
        return Err(PyValueError::new_err(
            public_error::BLOOM_MAX_MIP_DIMENSION_ZERO,
        ));
    }
    Ok(())
}

fn validate_bloom_batch(py: Python<'_>, batch: &PyRustComponentBatch) -> PyResult<()> {
    if let Some(values) = batch.get_field_values("max_mip_dimension") {
        for &dimension in values {
            validate_max_mip_dimension(dimension as u32)?;
        }
        return Ok(());
    }
    if let Some(array) = batch.get_field_array(py, "max_mip_dimension") {
        let array: PyReadonlyArray2<f32> = array.extract()?;
        for &dimension in array.as_slice()? {
            validate_max_mip_dimension(dimension as u32)?;
        }
    }
    Ok(())
}

#[pycomponent(Bloom, bridge, batch_validate = validate_bloom_batch, view_fields = [
    intensity,
    low_frequency_boost,
    low_frequency_boost_curvature,
    high_pass_frequency,
    max_mip_dimension
])]
#[pyclass(name = "Bloom", module = "pybevy.post_process", extends = PyComponent)]
pub struct PyBloom {
    pub(crate) storage: ComponentStorage<Bloom>,
}

#[pymethods]
impl PyBloom {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        *,
        intensity = 0.15,
        low_frequency_boost = 0.7,
        low_frequency_boost_curvature = 0.95,
        high_pass_frequency = 1.0,
        prefilter = None,
        composite_mode = PyBloomCompositeMode::EnergyConserving,
        max_mip_dimension = 512,
        scale = None
    ))]
    pub fn new(
        intensity: f32,
        low_frequency_boost: f32,
        low_frequency_boost_curvature: f32,
        high_pass_frequency: f32,
        prefilter: Option<PyBloomPrefilter>,
        composite_mode: PyBloomCompositeMode,
        max_mip_dimension: u32,
        scale: Option<Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let scale_vec = match scale {
            Some(s) => extract_vec2_from_any(&s)?,
            None => Vec2::ONE,
        };

        validate_max_mip_dimension(max_mip_dimension)?;

        let bloom = Bloom {
            intensity,
            low_frequency_boost,
            low_frequency_boost_curvature,
            high_pass_frequency,
            prefilter: match prefilter {
                Some(p) => p.try_into()?,
                None => BloomPrefilter::default(),
            },
            composite_mode: composite_mode.into(),
            max_mip_dimension,
            scale: scale_vec,
        };

        Ok(Self::from_owned(bloom).into())
    }

    #[staticmethod]
    #[pyo3(name = "NATURAL")]
    pub fn natural() -> PyResult<Py<Self>> {
        Python::attach(|py| Py::new(py, (PyBloom::from(Bloom::NATURAL), PyComponent)))
    }

    #[staticmethod]
    #[pyo3(name = "ANAMORPHIC")]
    pub fn anamorphic() -> PyResult<Py<Self>> {
        Python::attach(|py| Py::new(py, (PyBloom::from(Bloom::ANAMORPHIC), PyComponent)))
    }

    #[staticmethod]
    #[pyo3(name = "OLD_SCHOOL")]
    pub fn old_school() -> PyResult<Py<Self>> {
        Python::attach(|py| Py::new(py, (PyBloom::from(Bloom::OLD_SCHOOL), PyComponent)))
    }

    #[staticmethod]
    #[pyo3(name = "SCREEN_BLUR")]
    pub fn screen_blur() -> PyResult<Py<Self>> {
        Python::attach(|py| Py::new(py, (PyBloom::from(Bloom::SCREEN_BLUR), PyComponent)))
    }

    #[getter]
    pub fn intensity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.intensity)
    }

    #[setter]
    pub fn set_intensity(&mut self, intensity: f32) -> PyResult<()> {
        self.as_mut()?.intensity = intensity;
        Ok(())
    }

    #[getter]
    pub fn low_frequency_boost(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.low_frequency_boost)
    }

    #[setter]
    pub fn set_low_frequency_boost(&mut self, boost: f32) -> PyResult<()> {
        self.as_mut()?.low_frequency_boost = boost;
        Ok(())
    }

    #[getter]
    pub fn low_frequency_boost_curvature(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.low_frequency_boost_curvature)
    }

    #[setter]
    pub fn set_low_frequency_boost_curvature(&mut self, curvature: f32) -> PyResult<()> {
        self.as_mut()?.low_frequency_boost_curvature = curvature;
        Ok(())
    }

    #[getter]
    pub fn high_pass_frequency(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.high_pass_frequency)
    }

    #[setter]
    pub fn set_high_pass_frequency(&mut self, frequency: f32) -> PyResult<()> {
        self.as_mut()?.high_pass_frequency = frequency;
        Ok(())
    }

    #[getter]
    pub fn prefilter(&self) -> PyResult<PyBloomPrefilter> {
        Ok(self.storage.borrow_field_as(|b| &b.prefilter)?)
    }

    #[setter]
    pub fn set_prefilter(&mut self, prefilter: PyBloomPrefilter) -> PyResult<()> {
        self.as_mut()?.prefilter = prefilter.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn composite_mode(&self) -> PyResult<PyBloomCompositeMode> {
        Ok(self.as_ref()?.composite_mode.into())
    }

    #[setter]
    pub fn set_composite_mode(&mut self, mode: PyBloomCompositeMode) -> PyResult<()> {
        self.as_mut()?.composite_mode = mode.into();
        Ok(())
    }

    #[getter]
    pub fn max_mip_dimension(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.max_mip_dimension)
    }

    #[setter]
    pub fn set_max_mip_dimension(&mut self, dimension: u32) -> PyResult<()> {
        validate_max_mip_dimension(dimension)?;
        self.as_mut()?.max_mip_dimension = dimension;
        Ok(())
    }

    #[getter]
    pub fn scale(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|b| &b.scale)?)
    }

    #[setter]
    pub fn set_scale(&mut self, scale: PyVec2) -> PyResult<()> {
        self.as_mut()?.scale = scale.try_into()?;
        Ok(())
    }
}
