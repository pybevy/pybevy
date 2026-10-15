use bevy::mesh::{MeshBuilder, SphereKind, SphereMeshBuilder};
use pybevy_core::{PyAsset, public_error};
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{
    mesh::PyMesh, mesh_builder::PyMeshBuilder, primitives::validation::vertex_budget,
    sphere_kind::PySphereKind,
};

fn validate_uv(sectors: u32, stacks: u32) -> PyResult<()> {
    let vertices = (u128::from(sectors) + 1) * (u128::from(stacks) + 1);
    vertex_budget("SphereMeshBuilder", vertices)
}

fn validate_ico(subdivisions: u32) -> PyResult<()> {
    if subdivisions >= 80 {
        let side = u128::from(subdivisions) + 1;
        let vertices = 10 * side * side + 2;
        return Err(PyValueError::new_err(
            public_error::icosphere_subdivisions_too_many(subdivisions, vertices),
        ));
    }
    Ok(())
}

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
    pub fn kind(&self, py: Python<'_>, kind: PySphereKind) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.kind(kind.into())), PyMeshBuilder))
    }

    pub fn ico(&self, py: Python, subdivisions: u32) -> PyResult<Py<PyMesh>> {
        validate_ico(subdivisions)?;
        let builder = self
            .0
            .ico(subdivisions)
            .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?
            .into();

        Py::new(py, (builder, PyAsset))
    }

    pub fn uv(&self, py: Python, sectors: u32, stacks: u32) -> PyResult<Py<PyMesh>> {
        validate_uv(sectors, stacks)?;
        Py::new(py, (self.0.uv(sectors, stacks).into(), PyAsset))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        let mesh = match self.0.kind {
            SphereKind::Ico { subdivisions } => {
                validate_ico(subdivisions)?;
                self.0
                    .ico(subdivisions)
                    .map_err(|error| PyValueError::new_err(error.to_string()))?
            }
            SphereKind::Uv { sectors, stacks } => {
                validate_uv(sectors, stacks)?;
                self.0.build()
            }
        };
        Py::new(py, (mesh.into(), PyAsset))
    }
}
