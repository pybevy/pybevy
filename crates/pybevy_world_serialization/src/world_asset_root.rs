use bevy::world_serialization::WorldAssetRoot;
use pybevy_core::{PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pyhandle;
use pyo3::{exceptions::PyTypeError, prelude::*};

#[pyhandle(WorldAssetRoot)]
#[pyclass(name = "WorldAssetRoot", extends = PyComponent, eq, frozen)]
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
    pub fn new(handle: &Bound<'_, PyAny>) -> PyResult<(Self, PyComponent)> {
        let handle = extract_handle_from_any(handle)?;

        if let Some(name) = handle.asset_type_name()
            && name != "WorldAsset"
        {
            return Err(PyTypeError::new_err(format!(
                "AssetType `{}` does not match expected type `WorldAsset`",
                name
            )));
        }

        Ok((Self(handle), PyComponent))
    }

    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
