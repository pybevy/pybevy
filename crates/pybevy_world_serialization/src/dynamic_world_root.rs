use bevy::world_serialization::DynamicWorldRoot;
use pybevy_core::{PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pyhandle;
use pyo3::{exceptions::PyTypeError, prelude::*};

#[pyhandle(DynamicWorldRoot)]
#[pyclass(name = "DynamicWorldRoot", extends = PyComponent, eq, frozen)]
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
    pub fn new(handle: &Bound<'_, PyAny>) -> PyResult<(Self, PyComponent)> {
        let handle = extract_handle_from_any(handle)?;

        if let Some(name) = handle.asset_type_name()
            && name != "DynamicWorld"
        {
            return Err(PyTypeError::new_err(format!(
                "AssetType `{}` does not match expected type `DynamicWorld`",
                name
            )));
        }

        Ok((Self(handle), PyComponent))
    }

    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
