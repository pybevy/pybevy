use bevy::{math::primitives::Plane3d, mesh::Meshable};
use pybevy_math::{PyDir3, PyVec2, PyVec3};
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyPlaneMeshBuilder};

#[pyclass(name = "Plane3d", extends = PyMeshable, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyPlane3d(pub Plane3d);

#[pymethods]
impl PyPlane3d {
    #[new]
    #[pyo3(signature = (
        normal = PyDir3::Y.as_vec3(),
        half_size = PyVec2::splat(0.5)
    ))]
    pub fn new(normal: PyVec3, half_size: PyVec2) -> (Self, PyMeshable) {
        (
            Self(Plane3d::new(normal.into(), half_size.into())),
            PyMeshable,
        )
    }

    #[staticmethod]
    pub fn from_points(
        py: Python<'_>,
        a: PyVec3,
        b: PyVec3,
        c: PyVec3,
    ) -> PyResult<(Py<PyPlane3d>, PyVec3)> {
        let (plane, translation) = Plane3d::from_points(a.into(), b.into(), c.into());
        let py_plane = Py::new(py, (PyPlane3d(plane), PyMeshable))?;
        Ok((py_plane, PyVec3::from_vec3(translation)))
    }

    #[getter]
    pub fn half_size(&self) -> PyVec2 {
        self.0.half_size.into()
    }

    #[setter]
    pub fn set_half_size(&mut self, value: PyVec2) {
        self.0.half_size = value.into();
    }

    pub fn __repr__(&self) -> String {
        format!(
            "Plane3d(normal={:?}, half_size={:?})",
            self.0.normal, self.0.half_size
        )
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyPlaneMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }
}

impl From<Plane3d> for PyPlane3d {
    fn from(plane: Plane3d) -> Self {
        PyPlane3d(plane)
    }
}
