use bevy::mesh::{CircularSectorMeshBuilder, MeshBuilder};
use pybevy_core::{
    PyAsset,
    domain::{IntegerInput, check_for},
};
use pyo3::prelude::*;

use crate::{
    circular_mesh_uv_mode::PyCircularMeshUvMode,
    mesh::PyMesh,
    mesh_builder::PyMeshBuilder,
    primitives::validation::{AT_LEAST_TWO, vertex_budget},
};

#[pyclass(name = "CircularSectorMeshBuilder", module = "pybevy.mesh", extends = PyMeshBuilder)]
#[derive(Debug)]
pub struct PyCircularSectorMeshBuilder(pub(crate) CircularSectorMeshBuilder);

impl From<CircularSectorMeshBuilder> for PyCircularSectorMeshBuilder {
    fn from(builder: CircularSectorMeshBuilder) -> Self {
        Self(builder)
    }
}

#[pymethods]
impl PyCircularSectorMeshBuilder {
    pub fn resolution(&self, py: Python<'_>, resolution: IntegerInput<u32>) -> PyResult<Py<Self>> {
        let resolution = check_for::<Self, _>("resolution", resolution, AT_LEAST_TWO)?.stored();
        let mut builder = self.0.clone();
        builder.resolution = resolution;
        Py::new(py, (Self(builder), PyMeshBuilder))
    }

    pub fn uv_mode(&self, py: Python<'_>, mode: PyCircularMeshUvMode) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (Self(self.0.clone().uv_mode(mode.into())), PyMeshBuilder),
        )
    }

    pub fn build(&self, py: Python) -> PyResult<Py<PyMesh>> {
        check_for::<Self, _>("resolution", self.0.resolution, AT_LEAST_TWO)?;
        vertex_budget(
            "CircularSectorMeshBuilder",
            u128::from(self.0.resolution) + 1,
        )?;
        Py::new(py, (self.0.build().into(), PyAsset))
    }
}
