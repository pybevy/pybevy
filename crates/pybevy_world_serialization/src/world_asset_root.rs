use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use pybevy_core::{HandleArg, PyComponent, PyHandle};
use pybevy_macros::pyhandle;
use pyo3::prelude::*;

#[pyhandle(WorldAssetRoot)]
#[pyclass(name = "WorldAssetRoot", module = "pybevy.world_serialization", extends = PyComponent, eq, frozen, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWorldAssetRoot(pub(crate) PyHandle);

impl TryFrom<&PyWorldAssetRoot> for WorldAssetRoot {
    type Error = PyErr;

    fn try_from(value: &PyWorldAssetRoot) -> Result<Self, Self::Error> {
        Ok(WorldAssetRoot((&value.0).try_into()?))
    }
}

impl From<&WorldAssetRoot> for PyWorldAssetRoot {
    fn from(value: &WorldAssetRoot) -> Self {
        PyWorldAssetRoot((&value.0).into())
    }
}

#[pymethods]
impl PyWorldAssetRoot {
    #[new]
    #[pyo3(signature = (value = HandleArg::default()))]
    pub fn new(value: HandleArg) -> PyResult<PyClassInitializer<Self>> {
        Ok((Self(value.into_py_handle::<WorldAsset>()?), PyComponent).into())
    }

    #[getter]
    pub fn value(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
