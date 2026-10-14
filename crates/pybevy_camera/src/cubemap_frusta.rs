use bevy::camera::primitives::{CubemapFrusta, Frustum};
use pybevy_core::{
    ComponentStorage, ComponentStorageInner, PyComponent,
    public_error::{CUBEMAP_FACE_COUNT as CUBEMAP_FACE_COUNT_ERROR, CUBEMAP_FACE_INDEX},
};
use pybevy_macros::pycomponent;
use pyo3::{
    exceptions::{PyIndexError, PyValueError},
    prelude::*,
    types::PyList,
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

    /// Each frustum borrowed from the component, like
    /// `CubemapVisibleEntities.get`.
    #[getter]
    pub fn frusta(&self, py: Python<'_>) -> PyResult<Vec<Py<PyFrustum>>> {
        (0..CUBEMAP_FACE_COUNT)
            .map(|index| {
                let storage = match &self.storage.inner {
                    // A re-resolving component handle can only represent whole components.
                    ComponentStorageInner::Revalidating(_) => {
                        ComponentStorage::read_only_snapshot(&self.as_ref()?.frusta[index])
                    }
                    _ => self.storage.borrow_field(|cf| &cf.frusta[index])?,
                };
                Py::new(py, PyFrustum::from_borrowed(storage))
            })
            .collect()
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

    pub fn get(&self, py: Python<'_>, index: isize) -> PyResult<Py<PyFrustum>> {
        let index = usize::try_from(index).unwrap_or(usize::MAX);
        if index >= CUBEMAP_FACE_COUNT {
            return Err(PyIndexError::new_err(CUBEMAP_FACE_INDEX));
        }
        Ok(self.frusta(py)?.swap_remove(index))
    }

    /// The six frustums in face order, as `get` returns them.
    pub fn iter(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(PyList::new(py, self.frusta(py)?)?
            .call_method0("__iter__")?
            .unbind())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("CubemapFrusta([{CUBEMAP_FACE_COUNT} frustums])"))
    }
}
