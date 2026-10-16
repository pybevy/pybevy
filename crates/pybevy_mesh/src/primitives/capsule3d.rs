use bevy::mesh::{Capsule3dMeshBuilder, CapsuleUvProfile, MeshBuilder};
use pybevy_core::{PyAsset, domain::IntegerInput};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::{
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::validation::{
        AT_LEAST_FOUR, AT_LEAST_ONE, minimum_count, stored_count, vertex_budget,
    },
};

#[pyenum(CapsuleUvProfile)]
#[pyclass(
    name = "CapsuleUvProfile",
    module = "pybevy.mesh",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyCapsuleUvProfile {
    Aspect,
    Uniform,
    Fixed,
}

#[pyclass(name = "Capsule3dMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyCapsule3dMeshBuilder(pub(crate) Capsule3dMeshBuilder);

impl From<Capsule3dMeshBuilder> for PyCapsule3dMeshBuilder {
    fn from(builder: Capsule3dMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCapsule3dMeshBuilder {
    pub fn rings(&self, py: Python<'_>, rings: u32) -> PyResult<Py<Self>> {
        let mut builder = self.0;
        builder.rings = rings;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn longitudes(&self, py: Python<'_>, longitudes: IntegerInput<u32>) -> PyResult<Py<Self>> {
        let longitudes = minimum_count(
            "Capsule3dMeshBuilder",
            "longitudes",
            longitudes,
            AT_LEAST_ONE,
        )?;
        let mut builder = self.0;
        builder.longitudes = longitudes;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn latitudes(&self, py: Python<'_>, latitudes: IntegerInput<u32>) -> PyResult<Py<Self>> {
        let latitudes = minimum_count(
            "Capsule3dMeshBuilder",
            "latitudes",
            latitudes,
            AT_LEAST_FOUR,
        )?;
        let mut builder = self.0;
        builder.latitudes = latitudes;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn uv_profile(&self, py: Python<'_>, profile: PyCapsuleUvProfile) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.uv_profile(profile.into())), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        stored_count(
            "Capsule3dMeshBuilder",
            "longitudes",
            self.0.longitudes,
            AT_LEAST_ONE,
        )?;
        stored_count(
            "Capsule3dMeshBuilder",
            "latitudes",
            self.0.latitudes,
            AT_LEAST_FOUR,
        )?;
        let longitudes = u128::from(self.0.longitudes);
        let latitude_rings = 2 * u128::from(self.0.latitudes / 2);
        let vertices =
            2 * longitudes + (longitudes + 1) * (latitude_rings + u128::from(self.0.rings));
        vertex_budget("Capsule3dMeshBuilder", vertices)?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
