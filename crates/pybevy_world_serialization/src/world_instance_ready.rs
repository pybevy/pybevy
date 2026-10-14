use bevy::world_serialization::WorldInstanceReady;
use pybevy_core::{FromBorrowedStorage, PyEntity, PyMessage, ValueStorage};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

use crate::instance_id::PyInstanceId;

#[pyvalue]
#[pyclass(name = "WorldInstanceReady", module = "pybevy.world_serialization", extends = PyMessage, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyWorldInstanceReady {
    pub(crate) storage: ValueStorage<WorldInstanceReady>,
}

impl PartialEq for PyWorldInstanceReady {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.to_bevy(), other.to_bevy()), (Ok(a), Ok(b)) if a == b)
    }
}

#[pymethods]
impl PyWorldInstanceReady {
    #[getter]
    fn entity(&self) -> PyResult<PyEntity> {
        Ok(PyEntity(self.as_ref()?.entity))
    }

    #[getter]
    fn instance_id(&self) -> PyResult<PyInstanceId> {
        Ok(self.as_ref()?.instance_id.into())
    }

    fn __repr__(&self) -> PyResult<String> {
        let ready = self.to_bevy()?;
        Ok(format!(
            "WorldInstanceReady(entity={}, instance_id={:?})",
            ready.entity,
            PyInstanceId::from(ready.instance_id)
        ))
    }
}

impl From<&WorldInstanceReady> for PyWorldInstanceReady {
    fn from(event: &WorldInstanceReady) -> Self {
        Self::from_owned(*event)
    }
}
