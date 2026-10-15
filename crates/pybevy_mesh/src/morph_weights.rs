use bevy::{
    asset::Handle,
    mesh::{Mesh, morph::MorphWeights},
};
use pybevy_core::{
    ComponentStorage, PyComponent, PyHandle,
    live_sequence::{compare_live_sequence, normalize_index},
    public_error,
    repr::float_list_repr,
};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyValueError, prelude::*, pyclass::CompareOp, types::PyList};

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
    pub fn weights(&self) -> PyResult<PyMorphWeightsView> {
        let storage = if self.storage.is_owned() {
            ComponentStorage::read_only_snapshot(&*self.as_ref()?)
        } else {
            self.storage.share_borrow()
        };
        Ok(PyMorphWeightsView { storage })
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

#[pyclass(
    name = "_MorphWeightsView",
    module = "pybevy.mesh",
    skip_from_py_object
)]
pub struct PyMorphWeightsView {
    storage: ComponentStorage<MorphWeights>,
}

impl PyMorphWeightsView {
    fn values(&self) -> PyResult<Vec<f32>> {
        Ok(self.storage.as_ref()?.weights().to_vec())
    }
}

#[pymethods]
impl PyMorphWeightsView {
    fn __len__(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.weights().len())
    }

    fn __getitem__(&self, index: isize) -> PyResult<f32> {
        let morph = self.storage.as_ref()?;
        let weights = morph.weights();
        Ok(weights[normalize_index(index, weights.len())?])
    }

    fn __setitem__(&mut self, index: isize, value: f32) -> PyResult<()> {
        let length = self.storage.as_ref()?.weights().len();
        let index = normalize_index(index, length)?;
        self.storage.as_mut()?.weights_mut()[index] = value;
        Ok(())
    }

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(PyList::new(py, self.values()?)?
            .call_method0("__iter__")?
            .unbind())
    }

    fn to_list(&self) -> PyResult<Vec<f32>> {
        self.values()
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<Py<PyAny>> {
        let py = other.py();
        let items = PyList::new(py, self.values()?)?;
        compare_live_sequence(py, &items, other, op, other.is_instance_of::<Self>())
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(float_list_repr(&self.values()?))
    }
}
