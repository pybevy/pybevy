use bevy::world_serialization::{DynamicWorld, DynamicWorldRoot};
use pybevy_core::{HandleArg, PyComponent, PyHandle};
use pybevy_macros::pyhandle;
use pyo3::prelude::*;

#[pyhandle(DynamicWorldRoot)]
#[pyclass(name = "DynamicWorldRoot", module = "pybevy.world_serialization", extends = PyComponent, eq, frozen, skip_from_py_object)]
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
    #[pyo3(signature = (value = HandleArg::default()))]
    pub fn new(value: HandleArg) -> PyResult<PyClassInitializer<Self>> {
        Ok((Self(value.into_py_handle::<DynamicWorld>()?), PyComponent).into())
    }

    #[getter]
    pub fn value(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
