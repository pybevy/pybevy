use bevy::scene::SceneInstanceReady;
use pybevy_core::PyEntity;
use pyo3::prelude::*;

use crate::instance_id::PyInstanceId;

#[pyclass(name = "SceneInstanceReady", frozen, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PySceneInstanceReady {
    #[pyo3(get)]
    pub entity: PyEntity,
    #[pyo3(get)]
    pub instance_id: PyInstanceId,
}

#[pymethods]
impl PySceneInstanceReady {
    fn __repr__(&self) -> String {
        format!(
            "SceneInstanceReady(entity={}, instance_id={:?})",
            self.entity.0, self.instance_id
        )
    }
}

impl From<&SceneInstanceReady> for PySceneInstanceReady {
    fn from(event: &SceneInstanceReady) -> Self {
        PySceneInstanceReady {
            entity: PyEntity(event.entity),
            instance_id: PyInstanceId::from(event.instance_id),
        }
    }
}
