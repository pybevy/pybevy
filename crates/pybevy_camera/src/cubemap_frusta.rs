use bevy::camera::primitives::CubemapFrusta;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::frustum::PyFrustum;

#[pycomponent(CubemapFrusta, bridge)]
#[pyclass(name = "CubemapFrusta", extends = PyComponent)]
pub struct PyCubemapFrusta {
    pub(crate) storage: ComponentStorage<CubemapFrusta>,
}

#[pymethods]
impl PyCubemapFrusta {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        (
            PyCubemapFrusta {
                storage: ComponentStorage::owned(CubemapFrusta::default()),
            },
            PyComponent,
        )
    }

    pub fn frusta(&self, py: Python<'_>) -> PyResult<Vec<Py<PyFrustum>>> {
        let cf = self.as_ref()?;
        let mut result = Vec::with_capacity(6);
        for frustum in cf.frusta.iter() {
            result.push(Py::new(py, PyFrustum::from_owned(*frustum))?);
        }
        Ok(result)
    }

    pub fn get(&self, py: Python<'_>, index: usize) -> PyResult<Py<PyFrustum>> {
        use pyo3::exceptions::PyIndexError;

        if index >= 6 {
            return Err(PyIndexError::new_err("Cubemap face index must be 0-5"));
        }
        let cf = self.as_ref()?;
        Py::new(py, PyFrustum::from_owned(cf.frusta[index]))
    }

    pub fn __len__(&self) -> usize {
        6
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok("CubemapFrusta([6 frustums])".to_string())
    }
}
