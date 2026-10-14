use bevy::mesh::{MeshBuilder, SphereKind, SphereMeshBuilder};
use pybevy_core::{
    PyAsset,
    domain::{FieldDomain, IntegerInput, check_for},
};
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::validation::{AT_LEAST_THREE, AT_LEAST_TWO, vertex_budget},
    sphere_kind::PySphereKind,
};

/// Fewer than 3 sectors or 2 stacks builds a sphere with no visible triangles.
fn validate_uv(sectors: u32, stacks: u32) -> PyResult<()> {
    check_for::<PySphereMeshBuilder, _>("sectors", sectors, AT_LEAST_THREE)?;
    check_for::<PySphereMeshBuilder, _>("stacks", stacks, AT_LEAST_TWO)?;
    let vertices = (u128::from(sectors) + 1) * (u128::from(stacks) + 1);
    vertex_budget("SphereMeshBuilder", vertices)
}

fn validate_kind(kind: SphereKind) -> PyResult<SphereKind> {
    match kind {
        SphereKind::Ico { subdivisions } => {
            check_for::<PySphereMeshBuilder, _>("subdivisions", subdivisions, SUBDIVISIONS_DOMAIN)?;
        }
        SphereKind::Uv { sectors, stacks } => validate_uv(sectors, stacks)?,
    }
    Ok(kind)
}

/// Bevy's icosphere indices are `u16`, which 79 subdivisions still fit.
const SUBDIVISIONS_DOMAIN: &[FieldDomain] = &[FieldDomain::Range { lo: 0.0, hi: 80.0 }];

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
        Py::new(
            py,
            (
                Self(self.0.kind(validate_kind(kind.into())?)),
                PyMeshBuilder,
            ),
        )
    }

    pub fn ico(&self, py: Python, subdivisions: IntegerInput<u32>) -> PyResult<Py<PyMesh>> {
        let subdivisions =
            check_for::<Self, _>("subdivisions", subdivisions, SUBDIVISIONS_DOMAIN)?.stored();
        let builder = self
            .0
            .ico(subdivisions)
            .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?
            .into();

        Py::new(py, (builder, PyAsset))
    }

    pub fn uv(
        &self,
        py: Python,
        sectors: IntegerInput<u32>,
        stacks: IntegerInput<u32>,
    ) -> PyResult<Py<PyMesh>> {
        let sectors = check_for::<Self, _>("sectors", sectors, AT_LEAST_THREE)?.stored();
        let stacks = check_for::<Self, _>("stacks", stacks, AT_LEAST_TWO)?.stored();
        validate_uv(sectors, stacks)?;
        Py::new(py, (self.0.uv(sectors, stacks).into(), PyAsset))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        let mesh = match validate_kind(self.0.kind)? {
            SphereKind::Ico { subdivisions } => self
                .0
                .ico(subdivisions)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
            SphereKind::Uv { .. } => self.0.build(),
        };
        Py::new(py, (mesh.into(), PyAsset))
    }
}
