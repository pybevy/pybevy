use bevy::mesh::{MeshBuilder, SphereKind, SphereMeshBuilder};
use pybevy_core::{
    PyAsset,
    domain::{DomainValues, FieldDomain, IntegerInput, check},
};
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{
    mesh::PyMesh, mesh_builder::PyMeshBuilder, primitives::validation::vertex_budget,
    sphere_kind::PySphereKind,
};

fn validate_uv(sectors: u32, stacks: u32) -> PyResult<()> {
    let vertices = (u128::from(sectors) + 1) * (u128::from(stacks) + 1);
    vertex_budget("SphereMeshBuilder", vertices)
}

/// Bevy's icosphere indices are `u16`, which 79 subdivisions still fit.
const SUBDIVISIONS_DOMAIN: &[FieldDomain] = &[FieldDomain::Range { lo: 0.0, hi: 80.0 }];

fn validate_ico<T: DomainValues>(subdivisions: T) -> PyResult<T> {
    Ok(check(
        "SphereMeshBuilder",
        "subdivisions",
        subdivisions,
        SUBDIVISIONS_DOMAIN,
    )?)
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

    pub fn ico(&self, py: Python, subdivisions: IntegerInput<u32>) -> PyResult<Py<PyMesh>> {
        let subdivisions = validate_ico(subdivisions)?.stored();
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
