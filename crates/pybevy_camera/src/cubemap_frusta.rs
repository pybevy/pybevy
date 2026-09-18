use bevy::camera::primitives::CubemapFrusta;
use pybevy_core::{ComponentStorage, PyComponent, public_error::CUBEMAP_FACE_INDEX};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyIndexError, prelude::*};

use crate::frustum::PyFrustum;

const CUBEMAP_FACE_COUNT: usize = 6;

#[pycomponent(CubemapFrusta, bridge)]
#[pyclass(name = "CubemapFrusta", module = "pybevy.camera", extends = PyComponent)]
pub struct PyCubemapFrusta {
    pub(crate) storage: ComponentStorage<CubemapFrusta>,
}

#[pymethods]
impl PyCubemapFrusta {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (
            PyCubemapFrusta {
                storage: ComponentStorage::owned(CubemapFrusta::default()),
            },
            PyComponent,
        )
            .into()
    }

    #[getter]
    pub fn frusta(&self, py: Python<'_>) -> PyResult<Vec<Py<PyFrustum>>> {
        let cf = self.as_ref()?;
        let mut result = Vec::with_capacity(CUBEMAP_FACE_COUNT);
        for frustum in cf.frusta.iter() {
            result.push(Py::new(
                py,
                (
                    PyFrustum {
                        storage: ComponentStorage::read_only_snapshot(frustum),
                    },
                    PyComponent,
                ),
            )?);
        }
        Ok(result)
    }

    pub fn get(&self, py: Python<'_>, index: usize) -> PyResult<Py<PyFrustum>> {
        if index >= CUBEMAP_FACE_COUNT {
            return Err(PyIndexError::new_err(CUBEMAP_FACE_INDEX));
        }
        let cf = self.as_ref()?;
        Py::new(
            py,
            (
                PyFrustum {
                    storage: ComponentStorage::read_only_snapshot(&cf.frusta[index]),
                },
                PyComponent,
            ),
        )
    }

    pub fn __len__(&self) -> usize {
        CUBEMAP_FACE_COUNT
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("CubemapFrusta([{CUBEMAP_FACE_COUNT} frustums])"))
    }
}
