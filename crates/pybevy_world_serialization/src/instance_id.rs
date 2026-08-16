use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use bevy::world_serialization::InstanceId;
use pyo3::prelude::*;

#[pyclass(name = "InstanceId", eq, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyInstanceId(pub(crate) InstanceId);

#[pymethods]
impl PyInstanceId {
    fn __repr__(&self) -> String {
        format!("{:?}", self.0)
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish()
    }
}

impl From<InstanceId> for PyInstanceId {
    fn from(id: InstanceId) -> Self {
        PyInstanceId(id)
    }
}

impl From<PyInstanceId> for InstanceId {
    fn from(id: PyInstanceId) -> Self {
        id.0
    }
}
