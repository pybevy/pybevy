use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use pybevy_core::{PyComponent, PyHandle, extract_handle_from_any};
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyType};
#[pyclass(name = "MeshMaterial3d", extends = PyComponent, eq, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMeshMaterial3d(pub(crate) PyHandle);

impl TryFrom<&PyMeshMaterial3d> for MeshMaterial3d<StandardMaterial> {
    type Error = PyErr;

    fn try_from(value: &PyMeshMaterial3d) -> Result<Self, Self::Error> {
        Ok(MeshMaterial3d((&value.0).try_into()?))
    }
}

impl From<&MeshMaterial3d<StandardMaterial>> for PyMeshMaterial3d {
    fn from(value: &MeshMaterial3d<StandardMaterial>) -> Self {
        PyMeshMaterial3d((&value.0).into())
    }
}

#[pymethods]
impl PyMeshMaterial3d {
    #[new]
    pub fn new(handle: &Bound<'_, PyAny>) -> PyResult<(Self, PyComponent)> {
        let handle = extract_handle_from_any(handle)?;

        // Validate asset type
        if let Some(name) = handle.asset_type_name() {
            if name != "StandardMaterial" {
                return Err(PyTypeError::new_err(format!(
                    "AssetType `{}` does not match expected type `StandardMaterial`",
                    name
                )));
            }
        }

        Ok((Self(handle), PyComponent))
    }

    /// Support `MeshMaterial3d[HologramMaterial]` subscript notation.
    ///
    /// If the key has `__pybevy_material_component__`, returns that component type
    /// (e.g. MeshMaterial3dShader). Otherwise returns MeshMaterial3d itself.
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        // Check for @material redirect
        if let Ok(component) = key.getattr("__pybevy_material_component__") {
            return Ok(component.unbind());
        }
        // Default: return MeshMaterial3d itself
        Ok(cls.clone().into_any().unbind())
    }

    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
