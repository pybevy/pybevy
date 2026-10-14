use bevy::mesh::{Mesh, Mesh2d};
use pybevy_core::{HandleArg, PyComponent, PyHandle};
use pybevy_macros::pyhandle;
use pyo3::prelude::*;

#[pyhandle(Mesh2d)]
#[pyclass(name = "Mesh2d", module = "pybevy.mesh", extends = PyComponent, eq, frozen, skip_from_py_object)]
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
    #[pyo3(signature = (mesh = HandleArg::default()))]
    pub fn new(mesh: HandleArg) -> PyResult<PyClassInitializer<Self>> {
        Ok((Self(mesh.into_py_handle::<Mesh>()?), PyComponent).into())
    }
    #[getter]
    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
