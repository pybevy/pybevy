use bevy::{
    math::primitives::{Measured2d, Triangle3d},
    mesh::Meshable,
};
use pybevy_macros::pyconstructor;
use pybevy_math::{dir3::PyDir3, vec3::PyVec3};
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyTriangle3dMeshBuilder,
};

#[pyclass(name = "Triangle3d", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
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

#[pyconstructor("Triangle3d", keyword_only(vertices), conflicts((a, b, c), (vertices)))]
#[pymethods]
impl PyTriangle3d {
    #[new]
    pub fn new(
        #[expected("Vec3")] a: Option<PyVec3>,
        #[expected("Vec3")] b: Option<PyVec3>,
        #[expected("Vec3")] c: Option<PyVec3>,
        #[expected("sequence of 3 Vec3 values")] vertices: Option<[PyVec3; 3]>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let a = a.unwrap_or_else(|| PyVec3::new(0.0, 0.5, 0.0));
        let b = b.unwrap_or_else(|| PyVec3::new(-0.5, -0.5, 0.0));
        let c = c.unwrap_or_else(|| PyVec3::new(0.5, -0.5, 0.0));
        if let Some(v) = vertices {
            let verts = [
                (&v[0]).try_into()?,
                (&v[1]).try_into()?,
                (&v[2]).try_into()?,
            ];
            return Ok((Self(Triangle3d { vertices: verts }), PyMeshable).into());
        }
        let verts = [a.try_into()?, b.try_into()?, c.try_into()?];
        Ok((Self(Triangle3d { vertices: verts }), PyMeshable).into())
    }

    #[getter]
    pub fn vertices(&self) -> PyResult<[PyVec3; 3]> {
        Ok([
            PyVec3::from_vec3(self.0.vertices[0]),
            PyVec3::from_vec3(self.0.vertices[1]),
            PyVec3::from_vec3(self.0.vertices[2]),
        ])
    }

    #[setter]
    pub fn set_vertices(&mut self, vertices: [PyVec3; 3]) -> PyResult<()> {
        self.0.vertices = [
            (&vertices[0]).try_into()?,
            (&vertices[1]).try_into()?,
            (&vertices[2]).try_into()?,
        ];
        Ok(())
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
