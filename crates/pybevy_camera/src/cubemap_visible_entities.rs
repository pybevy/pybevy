use bevy::camera::visibility::CubemapVisibleEntities;
use pybevy_core::{
    ComponentStorage, ComponentStorageInner, PyComponent, public_error::CUBEMAP_FACE_INDEX,
};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyIndexError, prelude::*};

use crate::visible_mesh_entities::PyVisibleMeshEntities;

const CUBEMAP_FACE_COUNT: usize = 6;

#[pycomponent(CubemapVisibleEntities, bridge)]
#[pyclass(name = "CubemapVisibleEntities", module = "pybevy.camera", extends = PyComponent)]
pub struct PyCubemapVisibleEntities {
    pub(crate) storage: ComponentStorage<CubemapVisibleEntities>,
}

#[pymethods]
impl PyCubemapVisibleEntities {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (
            PyCubemapVisibleEntities {
                storage: ComponentStorage::owned(CubemapVisibleEntities::default()),
            },
            PyComponent,
        )
            .into()
    }

    pub fn get(&self, py: Python<'_>, i: isize) -> PyResult<Py<PyVisibleMeshEntities>> {
        let i = usize::try_from(i)
            .ok()
            .filter(|&i| i < CUBEMAP_FACE_COUNT)
            .ok_or_else(|| PyIndexError::new_err(CUBEMAP_FACE_INDEX))?;
        let storage = match &self.storage.inner {
            // A re-resolving component handle can only represent whole components.
            ComponentStorageInner::Revalidating(_) => {
                ComponentStorage::read_only_snapshot(self.as_ref()?.get(i))
            }
            _ => self.storage.borrow_field(|cve| cve.get(i))?,
        };
        Py::new(py, PyVisibleMeshEntities::from_borrowed(storage))
    }

    pub fn __len__(&self) -> usize {
        CUBEMAP_FACE_COUNT
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "CubemapVisibleEntities([{CUBEMAP_FACE_COUNT} faces])"
        ))
    }
}
