use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use pybevy_core::{PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::handle_storage;
use pyo3::{exceptions::PyTypeError, prelude::*};

#[handle_storage(MeshMaterial2d::<ColorMaterial>, "MeshMaterial2d")]
#[pyclass(name = "MeshMaterial2d", extends = PyComponent, eq, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMeshMaterial2d(pub(crate) PyHandle);

impl TryFrom<&PyMeshMaterial2d> for MeshMaterial2d<ColorMaterial> {
    type Error = PyErr;

    fn try_from(value: &PyMeshMaterial2d) -> Result<Self, Self::Error> {
        Ok(MeshMaterial2d((&value.0).try_into()?))
    }
}

impl From<&MeshMaterial2d<ColorMaterial>> for PyMeshMaterial2d {
    fn from(value: &MeshMaterial2d<ColorMaterial>) -> Self {
        PyMeshMaterial2d((&value.0).into())
    }
}

#[pymethods]
impl PyMeshMaterial2d {
    #[new]
    pub fn new(handle: &Bound<'_, PyAny>) -> PyResult<(Self, PyComponent)> {
        let handle = extract_handle_from_any(handle)?;

        // Validate asset type
        if let Some(name) = handle.asset_type_name()
            && name != "ColorMaterial"
        {
            return Err(PyTypeError::new_err(format!(
                "AssetType `{}` does not match expected type `ColorMaterial`",
                name
            )));
        }

        Ok((Self(handle), PyComponent))
    }
    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
