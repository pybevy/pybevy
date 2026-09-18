use bevy::camera::visibility::CubemapVisibleEntities;
use pybevy_core::{ComponentStorage, PyComponent, public_error::CUBEMAP_FACE_INDEX};
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

    pub fn get(&self, py: Python<'_>, i: usize) -> PyResult<Py<PyVisibleMeshEntities>> {
        if i >= CUBEMAP_FACE_COUNT {
            return Err(PyIndexError::new_err(CUBEMAP_FACE_INDEX));
        }
        let cve = self.as_ref()?;
        let vme = cve.get(i).clone();
        Py::new(py, PyVisibleMeshEntities::from_owned(vme))
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
