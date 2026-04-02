use bevy::mesh::Mesh3d;
use pybevy_core::{PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::handle_storage;
use pyo3::{exceptions::PyTypeError, prelude::*};

#[handle_storage(Mesh3d)]
#[pyclass(name = "Mesh3d", extends = PyComponent, eq, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMesh3d(pub(crate) PyHandle);

impl TryFrom<&PyMesh3d> for Mesh3d {
    type Error = PyErr;

    fn try_from(value: &PyMesh3d) -> Result<Self, Self::Error> {
        Ok(Mesh3d((&value.0).try_into()?))
    }
}

impl From<&Mesh3d> for PyMesh3d {
    fn from(value: &Mesh3d) -> Self {
        PyMesh3d((&value.0).into())
    }
}

#[pymethods]
impl PyMesh3d {
    #[new]
    pub fn new(handle: &Bound<'_, PyAny>) -> PyResult<(Self, PyComponent)> {
        let handle = extract_handle_from_any(handle)?;

        // Validate asset type
        if let Some(name) = handle.asset_type_name()
            && name != "Mesh"
        {
            return Err(PyTypeError::new_err(format!(
                "AssetType `{}` does not match expected type `Mesh`",
                name
            )));
        }

        Ok((Self(handle), PyComponent))
    }
    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
