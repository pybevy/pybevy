use bevy::camera::visibility::VisibleMeshEntities;
use pybevy_core::{ComponentStorage, PyComponent, PyEntity};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(VisibleMeshEntities, bridge)]
#[pyclass(name = "VisibleMeshEntities", module = "pybevy.camera", extends = PyComponent)]
pub struct PyVisibleMeshEntities {
    pub(crate) storage: ComponentStorage<VisibleMeshEntities>,
}

#[pymethods]
impl PyVisibleMeshEntities {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (
            PyVisibleMeshEntities {
                storage: ComponentStorage::owned(VisibleMeshEntities::default()),
            },
            PyComponent,
        )
            .into()
    }
    #[getter]
    pub fn entities(&self) -> PyResult<Vec<PyEntity>> {
        Ok(self
            .as_ref()?
            .entities
            .iter()
            .copied()
            .map(PyEntity::from)
            .collect())
    }

    #[setter]
    pub fn set_entities(&mut self, entities: Vec<PyEntity>) -> PyResult<()> {
        self.as_mut()?.entities = entities.into_iter().map(|entity| entity.0).collect();
        Ok(())
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
