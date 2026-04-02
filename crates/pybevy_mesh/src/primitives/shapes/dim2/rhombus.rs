use bevy::{
    math::{prelude::Measured2d, primitives::Rhombus},
    mesh::Meshable,
};
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyRhombusMeshBuilder};

#[pyclass(name = "Rhombus", extends = PyMeshable, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyRhombus(pub(crate) Rhombus);

impl From<PyRhombus> for Rhombus {
    fn from(py_rhombus: PyRhombus) -> Self {
        py_rhombus.0
    }
}

impl From<Rhombus> for PyRhombus {
    fn from(rhombus: Rhombus) -> Self {
        PyRhombus(rhombus)
    }
}

#[pymethods]
impl PyRhombus {
    #[new]
    #[pyo3(signature = (horizontal_diagonal = 1.0, vertical_diagonal = 1.0, *, half_diagonals = None))]
    pub fn new(
        horizontal_diagonal: f32,
        vertical_diagonal: f32,
        half_diagonals: Option<PyVec2>,
    ) -> (Self, PyMeshable) {
        if let Some(hd) = half_diagonals {
            return (
                Self(Rhombus {
                    half_diagonals: hd.into(),
                }),
                PyMeshable,
            );
        }
        (
            Self(Rhombus::new(horizontal_diagonal, vertical_diagonal)),
            PyMeshable,
        )
    }

    #[staticmethod]
    pub fn from_side(py: Python, side: f32) -> PyResult<Py<Self>> {
        Py::new(py, (Self(Rhombus::from_side(side)), PyMeshable))
    }

    #[staticmethod]
    pub fn from_inradius(py: Python, inradius: f32) -> PyResult<Py<Self>> {
        Py::new(py, (Self(Rhombus::from_inradius(inradius)), PyMeshable))
    }

    #[getter]
    pub fn half_diagonals(&self) -> PyVec2 {
        PyVec2::from_vec2(self.0.half_diagonals)
    }

    #[setter]
    pub fn set_half_diagonals(&mut self, value: PyVec2) {
        self.0.half_diagonals = value.into();
    }

    pub fn side(&self) -> f32 {
        self.0.side()
    }

    pub fn circumradius(&self) -> f32 {
        self.0.circumradius()
    }

    pub fn inradius(&self) -> f32 {
        self.0.inradius()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn closest_point(&self, point: PyVec2) -> PyVec2 {
        PyVec2::from_vec2(self.0.closest_point(point.into()))
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyRhombusMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "Rhombus(half_diagonals=Vec2({}, {}))",
            self.0.half_diagonals.x, self.0.half_diagonals.y
        )
    }
}
