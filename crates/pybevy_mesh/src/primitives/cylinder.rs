use bevy::mesh::{CylinderAnchor, CylinderMeshBuilder, MeshBuilder};
use pybevy_core::PyAsset;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::{
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::validation::{minimum_count, vertex_budget},
};

#[pyenum(CylinderAnchor)]
#[pyclass(
    name = "CylinderAnchor",
    module = "pybevy.mesh",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyCylinderAnchor {
    MidPoint,
    Top,
    Bottom,
}

#[pyclass(name = "CylinderMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyCylinderMeshBuilder(CylinderMeshBuilder);

impl From<CylinderMeshBuilder> for PyCylinderMeshBuilder {
    fn from(builder: CylinderMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCylinderMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: u32) -> PyResult<Py<Self>> {
        minimum_count("CylinderMeshBuilder.resolution", resolution, 3)?;
        Py::new(py, (Self(self.0.resolution(resolution)), PyMeshBuilder))
    }

    pub fn segments(&self, py: Python<'_>, segments: u32) -> PyResult<Py<Self>> {
        minimum_count("CylinderMeshBuilder.segments", segments, 1)?;
        Py::new(py, (Self(self.0.segments(segments)), PyMeshBuilder))
    }

    pub fn without_caps(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.without_caps()), PyMeshBuilder))
    }

    pub fn anchor(&self, py: Python<'_>, anchor: PyCylinderAnchor) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.anchor(anchor.into())), PyMeshBuilder))
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        minimum_count("CylinderMeshBuilder.resolution", self.0.resolution, 3)?;
        minimum_count("CylinderMeshBuilder.segments", self.0.segments, 1)?;
        let resolution = u128::from(self.0.resolution);
        let segments = u128::from(self.0.segments);
        let caps = if self.0.caps { 2 * resolution } else { 0 };
        vertex_budget(
            "CylinderMeshBuilder",
            caps + (segments + 1) * (resolution + 1),
        )?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
