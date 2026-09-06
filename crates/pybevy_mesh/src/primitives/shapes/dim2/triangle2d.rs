use bevy::{
    math::primitives::{Measured2d, Triangle2d},
    mesh::Meshable,
};
use pybevy_math::{vec2::PyVec2, winding_order::PyWindingOrder};
use pyo3::prelude::*;

use super::circle::PyCircle;
use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyTriangle2dMeshBuilder,
};

#[pyclass(name = "Triangle2d", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
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
    ) -> PyResult<PyClassInitializer<Self>> {
        if let Some(v) = vertices {
            let verts = [
                (&v[0]).try_into()?,
                (&v[1]).try_into()?,
                (&v[2]).try_into()?,
            ];
            return Ok((Self(Triangle2d { vertices: verts }), PyMeshable).into());
        }
        let verts = [a.try_into()?, b.try_into()?, c.try_into()?];
        Ok((Self(Triangle2d { vertices: verts }), PyMeshable).into())
    }

    #[getter]
    pub fn vertices(&self) -> PyResult<[PyVec2; 3]> {
        Ok([
            PyVec2::from_vec2(self.0.vertices[0]),
            PyVec2::from_vec2(self.0.vertices[1]),
            PyVec2::from_vec2(self.0.vertices[2]),
        ])
    }

    #[setter]
    pub fn set_vertices(&mut self, vertices: [PyVec2; 3]) -> PyResult<()> {
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
