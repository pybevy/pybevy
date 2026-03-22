use bevy::math::{Vec2, primitives::Polygon};
use pyo3::{prelude::*, types::PyList};

use crate::vec2::PyVec2;

#[pyclass(name = "Polygon", eq)]
#[derive(Clone, PartialEq)]
pub struct PyPolygon {
    pub(crate) inner: Polygon,
}

#[pymethods]
impl PyPolygon {
    #[new]
    pub fn new(vertices: Vec<PyVec2>) -> Self {
        let bevy_vertices: Vec<Vec2> = vertices.into_iter().map(|v| v.into()).collect();
        Self {
            inner: Polygon::new(bevy_vertices),
        }
    }

    #[getter]
    pub fn vertices(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let list = PyList::empty(py);
        for vertex in &self.inner.vertices {
            list.append(PyVec2::from(*vertex))?;
        }
        Ok(list.into())
    }

    #[setter]
    pub fn set_vertices(&mut self, vertices: Vec<PyVec2>) {
        let bevy_vertices: Vec<Vec2> = vertices.into_iter().map(|v| v.into()).collect();
        self.inner = Polygon::new(bevy_vertices);
    }

    pub fn is_simple(&self) -> bool {
        self.inner.is_simple()
    }

    fn __repr__(&self) -> String {
        format!("Polygon(vertices=[{}])", self.inner.vertices.len())
    }
}

impl From<Polygon> for PyPolygon {
    fn from(polygon: Polygon) -> Self {
        Self { inner: polygon }
    }
}

impl From<PyPolygon> for Polygon {
    fn from(polygon: PyPolygon) -> Self {
        polygon.inner
    }
}
