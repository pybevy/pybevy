use bevy::{
    math::{
        Vec3,
        primitives::{Measured3d, Tetrahedron},
    },
    mesh::Meshable,
};
use pybevy_math::PyVec3;
use pyo3::prelude::*;

use super::triangle3d::PyTriangle3d;
use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyTetrahedronMeshBuilder,
};

#[pyclass(name = "Tetrahedron", extends = PyMeshable, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyTetrahedron(pub(crate) Tetrahedron);

impl From<PyTetrahedron> for Tetrahedron {
    fn from(py_tetrahedron: PyTetrahedron) -> Self {
        py_tetrahedron.0
    }
}

impl From<Tetrahedron> for PyTetrahedron {
    fn from(tetrahedron: Tetrahedron) -> Self {
        PyTetrahedron(tetrahedron)
    }
}

#[pymethods]
impl PyTetrahedron {
    #[new]
    #[pyo3(signature = (
        a = PyVec3::new(0.5, 0.5, 0.5),
        b = PyVec3::new(-0.5, 0.5, -0.5),
        c = PyVec3::new(-0.5, -0.5, 0.5),
        d = PyVec3::new(0.5, -0.5, -0.5),
        *,
        vertices = None
    ))]
    pub fn new(
        a: PyVec3,
        b: PyVec3,
        c: PyVec3,
        d: PyVec3,
        vertices: Option<[PyVec3; 4]>,
    ) -> (Self, PyMeshable) {
        if let Some(v) = vertices {
            let verts = [
                (&v[0]).into(),
                (&v[1]).into(),
                (&v[2]).into(),
                (&v[3]).into(),
            ];
            return (Self(Tetrahedron { vertices: verts }), PyMeshable);
        }
        let a_vec: Vec3 = a.into();
        let b_vec: Vec3 = b.into();
        let c_vec: Vec3 = c.into();
        let d_vec: Vec3 = d.into();

        (
            Self(Tetrahedron::new(a_vec, b_vec, c_vec, d_vec)),
            PyMeshable,
        )
    }

    #[getter]
    pub fn vertices(&self) -> [PyVec3; 4] {
        [
            PyVec3::from_vec3(self.0.vertices[0]),
            PyVec3::from_vec3(self.0.vertices[1]),
            PyVec3::from_vec3(self.0.vertices[2]),
            PyVec3::from_vec3(self.0.vertices[3]),
        ]
    }

    #[setter]
    pub fn set_vertices(&mut self, vertices: [PyVec3; 4]) {
        self.0.vertices = [
            (&vertices[0]).into(),
            (&vertices[1]).into(),
            (&vertices[2]).into(),
            (&vertices[3]).into(),
        ];
    }

    pub fn signed_volume(&self) -> f32 {
        self.0.signed_volume()
    }

    pub fn centroid(&self) -> PyVec3 {
        PyVec3::from_vec3(self.0.centroid())
    }

    pub fn faces(&self, py: Python<'_>) -> PyResult<[Py<PyTriangle3d>; 4]> {
        let faces = self.0.faces();
        Ok([
            Py::new(py, (faces[0].into(), PyMeshable))?,
            Py::new(py, (faces[1].into(), PyMeshable))?,
            Py::new(py, (faces[2].into(), PyMeshable))?,
            Py::new(py, (faces[3].into(), PyMeshable))?,
        ])
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn volume(&self) -> f32 {
        self.0.volume()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyTetrahedronMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        let v = &self.0.vertices;
        format!(
            "Tetrahedron(Vec3({}, {}, {}), Vec3({}, {}, {}), Vec3({}, {}, {}), Vec3({}, {}, {}))",
            v[0].x,
            v[0].y,
            v[0].z,
            v[1].x,
            v[1].y,
            v[1].z,
            v[2].x,
            v[2].y,
            v[2].z,
            v[3].x,
            v[3].y,
            v[3].z
        )
    }
}
