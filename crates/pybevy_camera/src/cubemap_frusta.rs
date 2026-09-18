use bevy::camera::primitives::{CubemapFrusta, Frustum};
use pybevy_core::{
    ComponentStorage, PyComponent,
    public_error::{CUBEMAP_FACE_COUNT as CUBEMAP_FACE_COUNT_ERROR, CUBEMAP_FACE_INDEX},
};
use pybevy_macros::pycomponent;
use pyo3::{
    exceptions::{PyIndexError, PyValueError},
    prelude::*,
};

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

    #[setter]
    pub fn set_frusta(&mut self, frusta: Vec<PyRef<'_, PyFrustum>>) -> PyResult<()> {
        let frusta: Vec<Frustum> = frusta
            .iter()
            .map(|frustum| PyFrustum::as_ref(frustum).map(|value| *value))
            .collect::<PyResult<_>>()?;
        let frusta: [Frustum; CUBEMAP_FACE_COUNT] = frusta
            .try_into()
            .map_err(|_| PyValueError::new_err(CUBEMAP_FACE_COUNT_ERROR))?;
        self.as_mut()?.frusta = frusta;
        Ok(())
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
