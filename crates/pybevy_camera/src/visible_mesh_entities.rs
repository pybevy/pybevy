use bevy::camera::visibility::VisibleMeshEntities;
use pybevy_core::{ComponentStorage, PyComponent, PyEntity};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(VisibleMeshEntities, bridge)]
#[pyclass(name = "VisibleMeshEntities", extends = PyComponent)]
#[derive(Clone)]
pub struct PyVisibleMeshEntities {
    pub(crate) storage: ComponentStorage<VisibleMeshEntities>,
}

#[pymethods]
impl PyVisibleMeshEntities {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (
            PyVisibleMeshEntities {
                storage: ComponentStorage::owned(VisibleMeshEntities::default()),
            },
            PyComponent,
        )
    }
    pub fn entities(&self) -> PyResult<Vec<PyEntity>> {
        Ok(self
            .as_ref()?
            .entities
            .iter()
            .copied()
            .map(PyEntity::from)
            .collect())
    }

    pub fn __len__(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.entities.len())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.entities.is_empty())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let count = self.as_ref()?.entities.len();
        Ok(format!("VisibleMeshEntities({} entities)", count))
    }
}
