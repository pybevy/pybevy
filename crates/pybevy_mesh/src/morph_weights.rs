use bevy::{
    asset::Handle,
    mesh::{Mesh, morph::MorphWeights},
    reflect::structs::Struct,
};
use pybevy_core::{
    ComponentStorage, PyComponent, PyFloatLiveList, PyHandle, public_error, repr::float_list_repr,
};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyValueError, prelude::*};

const WEIGHTS_FIELD: &str = "weights";

// Bevy keeps the Vec private behind weights()/weights_mut(); reflection reaches the same field.
fn weights_vec(morph: &MorphWeights) -> &Vec<f32> {
    morph
        .field(WEIGHTS_FIELD)
        .and_then(|field| field.try_downcast_ref::<Vec<f32>>())
        .expect("MorphWeights reflects its weights as Vec<f32>")
}

fn weights_vec_mut(morph: &mut MorphWeights) -> &mut Vec<f32> {
    morph
        .field_mut(WEIGHTS_FIELD)
        .and_then(|field| field.try_downcast_mut::<Vec<f32>>())
        .expect("MorphWeights reflects its weights as Vec<f32>")
}

#[pycomponent(MorphWeights, bridge)]
#[pyclass(name = "MorphWeights", module = "pybevy.mesh", extends = PyComponent)]
pub struct PyMorphWeights {
    pub(crate) storage: ComponentStorage<MorphWeights>,
}

#[pymethods]
impl PyMorphWeights {
    #[new]
    #[pyo3(signature = (weights = Vec::new(), first_mesh = None))]
    pub fn new(
        weights: Vec<f32>,
        first_mesh: Option<PyHandle>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let handle = match first_mesh {
            Some(h) => Some(Handle::<Mesh>::try_from(&h)?),
            None => None,
        };
        let morph = MorphWeights::new(weights, handle)
            .map_err(|e| PyValueError::new_err(public_error::morph_weights_creation_failed(e)))?;
        Ok(Self::from_owned(morph).into())
    }

    #[getter]
    pub fn first_mesh(&self) -> PyResult<Option<PyHandle>> {
        Ok(self
            .as_ref()?
            .first_mesh()
            .map(|h| PyHandle::from(h.clone())))
    }

    #[getter]
    pub fn weights(&self) -> PyResult<PyFloatLiveList> {
        let weights: PyFloatLiveList = self
            .storage
            .borrow_resolved_field_as(weights_vec, weights_vec_mut)?;
        Ok(weights.with_fixed_length("MorphWeights.weights"))
    }

    #[setter]
    pub fn set_weights(&mut self, weights: Vec<f32>) -> PyResult<()> {
        let mut morph = self.as_mut()?;
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

    pub fn __repr__(&self) -> PyResult<String> {
        let morph = self.as_ref()?;
        let weights = morph.weights();
        if weights.len() <= 5 {
            Ok(format!("MorphWeights({})", float_list_repr(weights)))
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
