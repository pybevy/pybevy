use bevy::camera::visibility::CubemapVisibleEntities;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::visible_mesh_entities::PyVisibleMeshEntities;

#[pycomponent(CubemapVisibleEntities, bridge)]
#[pyclass(name = "CubemapVisibleEntities", extends = PyComponent)]
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
        ).into()
    }

    pub fn get(&self, py: Python<'_>, i: usize) -> PyResult<Py<PyVisibleMeshEntities>> {
        use pyo3::exceptions::PyIndexError;

        if i >= 6 {
            return Err(PyIndexError::new_err("Cubemap face index must be 0-5"));
        }
        let cve = self.as_ref()?;
        let vme = cve.get(i).clone();
        Py::new(py, PyVisibleMeshEntities::from_owned(vme))
    }

    pub fn __len__(&self) -> usize {
        6
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok("CubemapVisibleEntities([6 faces])".to_string())
    }
}
