use bevy::world_serialization::{DynamicWorld, DynamicWorldRoot};
use pybevy_core::{PyComponent, PyHandle, ensure_asset_type, extract_handle_from_any};
use pybevy_macros::pyhandle;
use pyo3::prelude::*;

#[pyhandle(DynamicWorldRoot)]
#[pyclass(name = "DynamicWorldRoot", extends = PyComponent, eq, frozen, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyDynamicWorldRoot(pub(crate) PyHandle);

impl TryFrom<&PyDynamicWorldRoot> for DynamicWorldRoot {
    type Error = PyErr;

    fn try_from(value: &PyDynamicWorldRoot) -> Result<Self, Self::Error> {
        Ok(DynamicWorldRoot((&value.0).try_into()?))
    }
}

impl From<&DynamicWorldRoot> for PyDynamicWorldRoot {
    fn from(value: &DynamicWorldRoot) -> Self {
        PyDynamicWorldRoot((&value.0).into())
    }
}

#[pymethods]
impl PyDynamicWorldRoot {
    #[new]
    pub fn new(value: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        let value = extract_handle_from_any(value)?;

        ensure_asset_type::<DynamicWorld>(&value)?;

        Ok((Self(value), PyComponent).into())
    }

    #[getter]
    pub fn value(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
