use bevy::{math::Vec2, mesh::Meshable, shape::Polyline2d};
use pybevy_core::{
    domain::{IntegerInput, NON_NEGATIVE, check_for},
    public_error::polyline_mesh_too_short,
};
use pybevy_math::vec2::PyVec2;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyList};

use crate::{
    mesh_builder::PyMeshBuilder,
    meshable::PyMeshable,
    primitives::{PyPolyline2dMeshBuilder, validation::vertex_budget},
};

#[pyclass(name = "Polyline2d", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyPolyline2d(pub(crate) Polyline2d);

#[pymethods]
impl PyPolyline2d {
    #[new]
    #[pyo3(signature = (
        vertices = Polyline2d::default().vertices.into_iter().map(PyVec2::from).collect()
    ))]
    pub fn new(vertices: Vec<PyVec2>) -> PyResult<PyClassInitializer<Self>> {
        let vertices: Vec<Vec2> = vertices
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;
        Ok((Self(Polyline2d::new(vertices)), PyMeshable).into())
    }

    #[staticmethod]
    #[pyo3(signature = (start, end, subdivisions))]
    pub fn with_subdivisions(
        py: Python<'_>,
        start: PyVec2,
        end: PyVec2,
        subdivisions: IntegerInput<u64>,
    ) -> PyResult<Py<Self>> {
        let subdivisions =
            check_for::<Self, _>("subdivisions", subdivisions, NON_NEGATIVE)?.stored();
        // Bevy reserves subdivisions + 2 vertices up front.
        vertex_budget("Polyline2d", u128::from(subdivisions) + 2)?;
        let subdivisions = subdivisions as usize;
        let start: Vec2 = start.try_into()?;
        let end: Vec2 = end.try_into()?;
        Py::new(
            py,
            (
                Self(Polyline2d::with_subdivisions(start, end, subdivisions)),
                PyMeshable,
            ),
        )
    }

    #[getter]
    pub fn vertices(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let list = PyList::empty(py);
        for vertex in &self.0.vertices {
            list.append(PyVec2::from_vec2(*vertex))?;
        }
        Ok(list.into())
    }

    #[setter]
    pub fn set_vertices(&mut self, vertices: Vec<PyVec2>) -> PyResult<()> {
        let bevy_vertices: Vec<Vec2> = vertices
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;
        self.0 = Polyline2d::new(bevy_vertices);
        Ok(())
    }

    pub fn mesh(&self, py: Python<'_>) -> PyResult<Py<PyPolyline2dMeshBuilder>> {
        if self.0.vertices.len() < 2 {
            // Avoid Bevy's index-range underflow below two vertices.
            return Err(PyValueError::new_err(polyline_mesh_too_short("Polyline2d")));
        }
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!("Polyline2d(vertices=[{}])", self.0.vertices.len())
    }
}
