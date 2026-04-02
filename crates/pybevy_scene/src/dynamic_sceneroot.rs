use bevy::scene::DynamicSceneRoot;
use pybevy_core::{PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::handle_storage;
use pyo3::{exceptions::PyTypeError, prelude::*};

#[handle_storage(DynamicSceneRoot)]
#[pyclass(name = "DynamicSceneRoot", extends = PyComponent, eq, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyDynamicSceneRoot(pub(crate) PyHandle);

impl TryFrom<&PyDynamicSceneRoot> for DynamicSceneRoot {
    type Error = PyErr;

    fn try_from(value: &PyDynamicSceneRoot) -> Result<Self, Self::Error> {
        Ok(DynamicSceneRoot((&value.0).try_into()?))
    }
}

impl From<&DynamicSceneRoot> for PyDynamicSceneRoot {
    fn from(value: &DynamicSceneRoot) -> Self {
        PyDynamicSceneRoot((&value.0).into())
    }
}

#[pymethods]
impl PyDynamicSceneRoot {
    #[new]
    pub fn new(handle: &Bound<'_, PyAny>) -> PyResult<(Self, PyComponent)> {
        let handle = extract_handle_from_any(handle)?;

        if let Some(name) = handle.asset_type_name()
            && name != "DynamicScene"
        {
            return Err(PyTypeError::new_err(format!(
                "AssetType `{}` does not match expected type `DynamicScene`",
                name
            )));
        }

        Ok((Self(handle), PyComponent))
    }

    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
