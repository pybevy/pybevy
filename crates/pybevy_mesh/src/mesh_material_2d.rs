use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use pybevy_core::{PyComponent, PyHandle, ensure_asset_type, extract_handle_from_any};
use pybevy_macros::pyhandle;
use pyo3::prelude::*;

#[pyhandle(MeshMaterial2d::<ColorMaterial>, "MeshMaterial2d")]
#[pyclass(name = "MeshMaterial2d", module = "pybevy.mesh", extends = PyComponent, eq, frozen, skip_from_py_object)]
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
    pub fn new(material: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        let handle = extract_handle_from_any(material)?;

        ensure_asset_type::<ColorMaterial>(&handle)?;

        Ok((Self(handle), PyComponent).into())
    }
    #[getter]
    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
