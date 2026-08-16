use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use pybevy_core::{PyComponent, PyHandle, ensure_asset_type, extract_handle_from_any};
use pybevy_macros::pyhandle;
use pyo3::prelude::*;

#[pyhandle(WorldAssetRoot)]
#[pyclass(name = "WorldAssetRoot", extends = PyComponent, eq, frozen, skip_from_py_object)]
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
    pub fn new(value: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        let value = extract_handle_from_any(value)?;

        ensure_asset_type::<WorldAsset>(&value)?;

        Ok((Self(value), PyComponent).into())
    }

    #[getter]
    pub fn value(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
