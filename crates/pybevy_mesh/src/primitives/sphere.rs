use bevy::mesh::{MeshBuilder, SphereMeshBuilder};
use pybevy_core::PyAsset;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder, sphere_kind::PySphereKind};

#[pyclass(name = "SphereMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PySphereMeshBuilder(SphereMeshBuilder);

impl From<SphereMeshBuilder> for PySphereMeshBuilder {
    fn from(builder: SphereMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PySphereMeshBuilder {
    /// Bevy parity (#97): with a `kind` argument this behaves like Bevy's
    /// chaining builder — it returns a NEW `SphereMeshBuilder` with the kind
    /// replaced (this object is frozen). With no argument it keeps the
    /// existing getter behavior and returns the current kind.
    #[pyo3(signature = (kind=None))]
    pub fn kind(&self, py: Python, kind: Option<PySphereKind>) -> PyResult<Py<PyAny>> {
        match kind {
            Some(kind) => {
                let builder = SphereMeshBuilder {
                    sphere: self.0.sphere,
                    kind: kind.into(),
                };
                Ok(Py::new(py, (PySphereMeshBuilder(builder), PyMeshBuilder))?.into_any())
            }
            None => {
                let current: PySphereKind = self.0.kind.into();
                Ok(current.into_pyobject(py)?.into_any().unbind())
            }
        }
    }

    pub fn ico(&self, py: Python, subdivisions: u32) -> PyResult<Py<PyMesh>> {
        let builder = self
            .0
            .ico(subdivisions)
            .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?
            .into();

        Py::new(py, (builder, PyAsset))
    }

    pub fn uv(&self, py: Python, sectors: u32, stacks: u32) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.uv(sectors, stacks).into(), PyAsset))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
