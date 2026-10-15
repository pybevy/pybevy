use bevy::math::{Vec2, primitives::Polygon};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::{prelude::*, types::PyList};

use crate::vec2::PyVec2;

#[pyclass(name = "Polygon", module = "pybevy.math", eq, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub struct PyPolygon {
    pub(crate) inner: Polygon,
}

#[pymethods]
impl PyPolygon {
    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(self.clone())
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    #[new]
    pub fn new(vertices: Vec<PyVec2>) -> PyResult<Self> {
        let bevy_vertices: Vec<Vec2> = vertices
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            inner: Polygon::new(bevy_vertices),
        })
    }

    #[getter]
    pub fn vertices(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let list = PyList::empty(py);
        for vertex in &self.inner.vertices {
            list.append(PyVec2::from_borrowed(ValueStorage::read_only_snapshot(
                *vertex,
            )))?;
        }
        Ok(list.into())
    }

    #[setter]
    pub fn set_vertices(&mut self, vertices: Vec<PyVec2>) -> PyResult<()> {
        let bevy_vertices: Vec<Vec2> = vertices
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;
        self.inner = Polygon::new(bevy_vertices);
        Ok(())
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
