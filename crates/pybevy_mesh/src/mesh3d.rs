use bevy::mesh::{Mesh, Mesh3d};
use pybevy_core::{HandleArg, PyComponent, PyHandle};
use pybevy_macros::pyhandle;
use pyo3::prelude::*;

#[pyhandle(Mesh3d)]
#[pyclass(name = "Mesh3d", module = "pybevy.mesh", extends = PyComponent, eq, frozen, skip_from_py_object)]
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
    #[pyo3(signature = (mesh = HandleArg::default()))]
    pub fn new(mesh: HandleArg) -> PyResult<PyClassInitializer<Self>> {
        Ok((Self(mesh.into_py_handle::<Mesh>()?), PyComponent).into())
    }
    #[getter]
    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
