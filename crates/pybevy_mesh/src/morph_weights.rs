use bevy::{
    asset::Handle,
    mesh::{Mesh, morph::MorphWeights},
};
use pybevy_core::{ComponentStorage, PyComponent, PyHandle};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyValueError, prelude::*};

#[pycomponent(MorphWeights, bridge)]
#[pyclass(name = "MorphWeights", extends = PyComponent)]
#[derive(Clone)]
pub struct PyMorphWeights {
    pub(crate) storage: ComponentStorage<MorphWeights>,
}

#[pymethods]
impl PyMorphWeights {
    #[new]
    #[pyo3(signature = (weights, first_mesh = None))]
    pub fn new(weights: Vec<f32>, first_mesh: Option<PyHandle>) -> PyResult<(Self, PyComponent)> {
        let handle = match first_mesh {
            Some(h) => Some(Handle::<Mesh>::try_from(&h)?),
            None => None,
        };
        let morph = MorphWeights::new(weights, handle).map_err(|e| {
            PyValueError::new_err(format!("Failed to create MorphWeights: {:?}", e))
        })?;
        Ok(Self::from_owned(morph))
    }

    #[getter]
    pub fn first_mesh(&self) -> PyResult<Option<PyHandle>> {
        Ok(self
            .as_ref()?
            .first_mesh()
            .map(|h| PyHandle::from(h.clone())))
    }

    #[getter]
    pub fn weights(&self) -> PyResult<Vec<f32>> {
        Ok(self.as_ref()?.weights().to_vec())
    }

    #[setter]
    pub fn set_weights(&mut self, weights: Vec<f32>) -> PyResult<()> {
        let morph = self.as_mut()?;
        let mutable_weights = morph.weights_mut();
        if weights.len() != mutable_weights.len() {
            return Err(PyValueError::new_err(format!(
                "Weight count mismatch: expected {}, got {}",
                mutable_weights.len(),
                weights.len()
            )));
        }
        mutable_weights.copy_from_slice(&weights);
        Ok(())
    }

    pub fn get_weight(&self, index: usize) -> PyResult<f32> {
        let weights = self.as_ref()?.weights();
        weights.get(index).copied().ok_or_else(|| {
            PyValueError::new_err(format!(
                "Index {} out of bounds for {} weights",
                index,
                weights.len()
            ))
        })
    }

    pub fn set_weight(&mut self, index: usize, value: f32) -> PyResult<()> {
        let weights = self.as_mut()?.weights_mut();
        if index >= weights.len() {
            return Err(PyValueError::new_err(format!(
                "Index {} out of bounds for {} weights",
                index,
                weights.len()
            )));
        }
        weights[index] = value;
        Ok(())
    }

    pub fn __len__(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.weights().len())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let morph = self.as_ref()?;
        let weights = morph.weights();
        if weights.len() <= 5 {
            Ok(format!("MorphWeights({:?})", weights))
        } else {
            Ok(format!(
                "MorphWeights([{:.3}, {:.3}, {:.3}, ... ({} total)])",
                weights[0],
                weights[1],
                weights[2],
                weights.len()
            ))
        }
    }
}
