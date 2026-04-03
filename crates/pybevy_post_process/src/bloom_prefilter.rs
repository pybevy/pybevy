use bevy::post_process::bloom::BloomPrefilter;
use pybevy_core::{FromBorrowedStorage, field_storage::FieldStorage};
use pybevy_macros::pyfield;
use pyo3::prelude::*;

#[pyfield]
#[pyclass(name = "BloomPrefilter")]
pub struct PyBloomPrefilter {
    pub(crate) storage: FieldStorage<BloomPrefilter>,
}

#[pymethods]
impl PyBloomPrefilter {
    #[new]
    #[pyo3(signature = (threshold = 0.0, threshold_softness = 0.0))]
    pub fn new(threshold: f32, threshold_softness: f32) -> Self {
        BloomPrefilter {
            threshold,
            threshold_softness,
        }
        .into()
    }

    #[getter]
    pub fn threshold(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.threshold)
    }

    #[setter]
    pub fn set_threshold(&mut self, threshold: f32) -> PyResult<()> {
        self.as_mut()?.threshold = threshold;
        Ok(())
    }

    #[getter]
    pub fn threshold_softness(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.threshold_softness)
    }

    #[setter]
    pub fn set_threshold_softness(&mut self, threshold_softness: f32) -> PyResult<()> {
        self.as_mut()?.threshold_softness = threshold_softness;
        Ok(())
    }
}
