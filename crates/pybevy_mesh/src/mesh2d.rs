use bevy::mesh::Mesh2d;
use pybevy_core::{PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pyhandle;
use pyo3::{exceptions::PyTypeError, prelude::*};

#[pyhandle(Mesh2d)]
#[pyclass(name = "Mesh2d", extends = PyComponent, eq, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMesh2d(pub(crate) PyHandle);

impl TryFrom<&PyMesh2d> for Mesh2d {
    type Error = PyErr;

    fn try_from(value: &PyMesh2d) -> Result<Self, Self::Error> {
        Ok(Mesh2d((&value.0).try_into()?))
    }
}

impl From<&Mesh2d> for PyMesh2d {
    fn from(value: &Mesh2d) -> Self {
        PyMesh2d((&value.0).into())
    }
}

#[pymethods]
impl PyMesh2d {
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
