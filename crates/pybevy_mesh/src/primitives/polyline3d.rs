use bevy::{
    asset::RenderAssetUsages,
    math::primitives::Polyline3d,
    mesh::{Indices, Mesh, MeshBuilder, PrimitiveTopology},
};
use pybevy_core::PyAsset;
use pyo3::prelude::*;

use crate::{mesh::PyMesh, mesh_builder::PyMeshBuilder};

// Bevy keeps its builder private, so reproduce the LineList conversion here.
#[pyclass(name = "Polyline3dMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder, frozen)]
#[derive(Debug)]
pub struct PyPolyline3dMeshBuilder {
    pub(crate) polyline: Polyline3d,
}

impl From<Polyline3d> for PyPolyline3dMeshBuilder {
    fn from(polyline: Polyline3d) -> Self {
        Self { polyline }
    }
}

impl MeshBuilder for PyPolyline3dMeshBuilder {
    fn build(&self) -> Mesh {
        if self.polyline.vertices.len() < 2 {
            // Avoid Bevy's index-range underflow for fewer than two vertices.
            return Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default());
        }
        let positions: Vec<[f32; 3]> = self
            .polyline
            .vertices
            .iter()
            .map(|v| [v.x, v.y, v.z])
            .collect();

        let indices = Indices::U32(
            (0..self.polyline.vertices.len() as u32 - 1)
                .flat_map(|i| [i, i + 1])
                .collect(),
        );

        Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default())
            .with_inserted_indices(indices)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    }
}

#[pymethods]
impl PyPolyline3dMeshBuilder {
    pub fn build(&self, py: Python<'_>) -> PyResult<Py<PyMesh>> {
        Py::new(py, (MeshBuilder::build(self).into(), PyAsset))
    }
}
