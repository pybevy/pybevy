use bevy::{
    math::primitives::{Measured2d, Triangle3d},
    mesh::Meshable,
};
use pybevy_math::{PyDir3, PyVec3};
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyTriangle3dMeshBuilder,
};

#[pyclass(name = "Triangle3d", extends = PyMeshable, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyTriangle3d(pub(crate) Triangle3d);

impl From<PyTriangle3d> for Triangle3d {
    fn from(py_triangle: PyTriangle3d) -> Self {
        py_triangle.0
    }
}

impl From<Triangle3d> for PyTriangle3d {
    fn from(triangle: Triangle3d) -> Self {
        PyTriangle3d(triangle)
    }
}

#[pymethods]
impl PyTriangle3d {
    #[new]
    #[pyo3(signature = (
        a = PyVec3::from_vec3(bevy::math::Vec3::new(0.0, 0.5, 0.0)),
        b = PyVec3::from_vec3(bevy::math::Vec3::new(-0.5, -0.5, 0.0)),
        c = PyVec3::from_vec3(bevy::math::Vec3::new(0.5, -0.5, 0.0)),
        *,
        vertices = None
    ))]
    pub fn new(
        a: PyVec3,
        b: PyVec3,
        c: PyVec3,
        vertices: Option<[PyVec3; 3]>,
    ) -> (Self, PyMeshable) {
        if let Some(v) = vertices {
            let verts = [(&v[0]).into(), (&v[1]).into(), (&v[2]).into()];
            return (Self(Triangle3d { vertices: verts }), PyMeshable);
        }
        let verts = [a.into(), b.into(), c.into()];
        (Self(Triangle3d { vertices: verts }), PyMeshable)
    }

    #[getter]
    pub fn vertices(&self) -> [PyVec3; 3] {
        [
            PyVec3::from_vec3(self.0.vertices[0]),
            PyVec3::from_vec3(self.0.vertices[1]),
            PyVec3::from_vec3(self.0.vertices[2]),
        ]
    }

    #[setter]
    pub fn set_vertices(&mut self, vertices: [PyVec3; 3]) {
        self.0.vertices = [
            (&vertices[0]).into(),
            (&vertices[1]).into(),
            (&vertices[2]).into(),
        ];
    }

    pub fn is_acute(&self) -> bool {
        self.0.is_acute()
    }

    pub fn centroid(&self) -> PyVec3 {
        PyVec3::from_vec3(self.0.centroid())
    }

    pub fn circumcenter(&self) -> PyVec3 {
        PyVec3::from_vec3(self.0.circumcenter())
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn is_degenerate(&self) -> bool {
        self.0.is_degenerate()
    }

    pub fn is_obtuse(&self) -> bool {
        self.0.is_obtuse()
    }

    pub fn normal(&self) -> PyResult<PyDir3> {
        self.0
            .normal()
            .map(PyDir3::from)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    pub fn reverse(&mut self) {
        self.0.reverse();
    }

    pub fn reversed(&self, py: Python) -> PyResult<Py<PyTriangle3d>> {
        Py::new(py, (Self(self.0.reversed()), PyMeshable))
    }

    pub fn largest_side(&self) -> (PyVec3, PyVec3) {
        let (a, b) = self.0.largest_side();
        (PyVec3::from_vec3(a), PyVec3::from_vec3(b))
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyTriangle3dMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        let v = &self.0.vertices;
        format!(
            "Triangle3d(Vec3({}, {}, {}), Vec3({}, {}, {}), Vec3({}, {}, {}))",
            v[0].x, v[0].y, v[0].z, v[1].x, v[1].y, v[1].z, v[2].x, v[2].y, v[2].z
        )
    }
}
