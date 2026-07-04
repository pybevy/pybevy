use bevy::world_serialization::WorldInstanceReady;
use pybevy_core::PyEntity;
use pyo3::prelude::*;

use crate::instance_id::PyInstanceId;

#[pyclass(name = "WorldInstanceReady", frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWorldInstanceReady {
    #[pyo3(get)]
    pub entity: PyEntity,
    #[pyo3(get)]
    pub instance_id: PyInstanceId,
}

#[pymethods]
impl PyWorldInstanceReady {
    fn __repr__(&self) -> String {
        format!(
            "WorldInstanceReady(entity={}, instance_id={:?})",
            self.entity.0, self.instance_id
        )
    }
}

impl From<&WorldInstanceReady> for PyWorldInstanceReady {
    fn from(event: &WorldInstanceReady) -> Self {
        PyWorldInstanceReady {
            entity: PyEntity(event.entity),
            instance_id: PyInstanceId::from(event.instance_id),
        }
    }
}
