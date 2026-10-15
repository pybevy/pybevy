use bevy::pbr::wireframe::{Mesh3dWireframe, WireframeMaterial};
use pybevy_core::{HandleArg, PyComponent, PyHandle};
use pybevy_macros::pyhandle;
use pyo3::prelude::*;

#[pyhandle(Mesh3dWireframe)]
#[pyclass(
    name = "Mesh3dWireframe", module = "pybevy.pbr",
    extends = PyComponent,
    eq,
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyMesh3dWireframe(pub(crate) PyHandle);

impl TryFrom<&PyMesh3dWireframe> for Mesh3dWireframe {
    type Error = PyErr;

    fn try_from(value: &PyMesh3dWireframe) -> Result<Self, Self::Error> {
        Ok(Mesh3dWireframe((&value.0).try_into()?))
    }
}

impl From<&Mesh3dWireframe> for PyMesh3dWireframe {
    fn from(value: &Mesh3dWireframe) -> Self {
        Self((&value.0).into())
    }
}

#[pymethods]
impl PyMesh3dWireframe {
    #[new]
    #[pyo3(signature = (material = HandleArg::default()))]
    pub fn new(material: HandleArg) -> PyResult<PyClassInitializer<Self>> {
        Ok((
            Self(material.into_py_handle::<WireframeMaterial>()?),
            PyComponent,
        )
            .into())
    }

    #[getter]
    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
