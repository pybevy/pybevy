use bevy::{
    math::primitives::{Measured2d, Rectangle},
    mesh::Meshable,
};
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyRectangleMeshBuilder,
};

#[pyclass(name = "Rectangle", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyRectangle(pub(crate) Rectangle);

#[pymethods]
impl PyRectangle {
    #[new]
    #[pyo3(signature = (width=1.0, height=1.0, *, half_size=None))]
    pub fn new(width: f32, height: f32, half_size: Option<PyVec2>) -> PyClassInitializer<Self> {
        if let Some(hs) = half_size {
            return (
                Self(Rectangle {
                    half_size: hs.into(),
                }),
                PyMeshable,
            ).into();
        }
        (Self(Rectangle::new(width, height)), PyMeshable).into()
    }

    #[staticmethod]
    pub fn from_size(py: Python, size: &PyVec2) -> PyResult<Py<Self>> {
        let rect = Rectangle::from_size(size.into());
        Py::new(py, (Self(rect), PyMeshable))
    }

    #[staticmethod]
    pub fn from_corners(py: Python, point1: &PyVec2, point2: &PyVec2) -> PyResult<Py<Self>> {
        let rect = Rectangle::from_corners(point1.into(), point2.into());
        Py::new(py, (Self(rect), PyMeshable))
    }

    #[staticmethod]
    pub fn from_length(py: Python, length: f32) -> PyResult<Py<Self>> {
        let rect = Rectangle::from_length(length);
        Py::new(py, (Self(rect), PyMeshable))
    }

    #[getter]
    pub fn half_size(&self) -> PyVec2 {
        self.0.half_size.into()
    }

    #[setter]
    pub fn set_half_size(&mut self, value: PyVec2) {
        self.0.half_size = value.into();
    }

    pub fn size(&self) -> PyVec2 {
        self.0.size().into()
    }

    pub fn closest_point(&self, point: &PyVec2) -> PyVec2 {
        let cp = self.0.closest_point(point.into());
        cp.into()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyRectangleMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    pub fn __repr__(&self) -> String {
        format!("Rectangle(size={})", self.0.size())
    }
}

impl From<Rectangle> for PyRectangle {
    fn from(rect: Rectangle) -> Self {
        PyRectangle(rect)
    }
}
