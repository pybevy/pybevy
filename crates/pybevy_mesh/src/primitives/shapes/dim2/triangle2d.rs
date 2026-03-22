use bevy::{
    math::primitives::{Measured2d, Triangle2d},
    mesh::Meshable,
};
use pybevy_math::{PyVec2, PyWindingOrder};
use pyo3::prelude::*;

use super::circle::PyCircle;
use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyTriangle2dMeshBuilder,
};

#[pyclass(name = "Triangle2d", extends = PyMeshable, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyTriangle2d(pub(crate) Triangle2d);

impl From<PyTriangle2d> for Triangle2d {
    fn from(py_triangle: PyTriangle2d) -> Self {
        py_triangle.0
    }
}

impl From<Triangle2d> for PyTriangle2d {
    fn from(triangle: Triangle2d) -> Self {
        PyTriangle2d(triangle)
    }
}

#[pymethods]
impl PyTriangle2d {
    #[new]
    #[pyo3(signature = (
        a = PyVec2::from_vec2(bevy::math::Vec2::new(0.0, 0.5)),
        b = PyVec2::from_vec2(bevy::math::Vec2::new(-0.5, -0.5)),
        c = PyVec2::from_vec2(bevy::math::Vec2::new(0.5, -0.5)),
        *,
        vertices = None
    ))]
    pub fn new(
        a: PyVec2,
        b: PyVec2,
        c: PyVec2,
        vertices: Option<[PyVec2; 3]>,
    ) -> (Self, PyMeshable) {
        if let Some(v) = vertices {
            let verts = [(&v[0]).into(), (&v[1]).into(), (&v[2]).into()];
            return (Self(Triangle2d { vertices: verts }), PyMeshable);
        }
        let verts = [a.into(), b.into(), c.into()];
        (Self(Triangle2d { vertices: verts }), PyMeshable)
    }

    #[getter]
    pub fn vertices(&self) -> [PyVec2; 3] {
        [
            PyVec2::from_vec2(self.0.vertices[0]),
            PyVec2::from_vec2(self.0.vertices[1]),
            PyVec2::from_vec2(self.0.vertices[2]),
        ]
    }

    #[setter]
    pub fn set_vertices(&mut self, vertices: [PyVec2; 3]) {
        self.0.vertices = [
            (&vertices[0]).into(),
            (&vertices[1]).into(),
            (&vertices[2]).into(),
        ];
    }

    pub fn is_acute(&self) -> bool {
        self.0.is_acute()
    }

    pub fn is_degenerate(&self) -> bool {
        self.0.is_degenerate()
    }

    pub fn circumcircle(&self, py: Python<'_>) -> PyResult<(Py<PyCircle>, PyVec2)> {
        let (circle, center) = self.0.circumcircle();
        let py_circle = Py::new(py, (circle.into(), PyMeshable))?;
        Ok((py_circle, PyVec2::from_vec2(center)))
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn winding_order(&self) -> PyWindingOrder {
        self.0.winding_order().into()
    }

    pub fn is_obtuse(&self) -> bool {
        self.0.is_obtuse()
    }

    pub fn reverse(&mut self) {
        self.0.reverse();
    }

    pub fn reversed(&self, py: Python<'_>) -> PyResult<Py<PyTriangle2d>> {
        Py::new(py, (Self(self.0.reversed()), PyMeshable))
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyTriangle2dMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        let v = &self.0.vertices;
        format!(
            "Triangle2d(Vec2({}, {}), Vec2({}, {}), Vec2({}, {}))",
            v[0].x, v[0].y, v[1].x, v[1].y, v[2].x, v[2].y
        )
    }
}
