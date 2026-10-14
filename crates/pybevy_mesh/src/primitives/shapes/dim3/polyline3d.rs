use bevy::{math::Vec3, shape::Polyline3d};
use pybevy_core::{
    domain::{IntegerInput, NON_NEGATIVE, check_for},
    public_error::polyline_mesh_too_short,
};
use pybevy_math::vec3::PyVec3;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyList};

use crate::{
    mesh_builder::PyMeshBuilder,
    meshable::PyMeshable,
    primitives::{PyPolyline3dMeshBuilder, validation::vertex_budget},
};

#[pyclass(name = "Polyline3d", module = "pybevy.shape", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyPolyline3d(pub(crate) Polyline3d);

#[pymethods]
impl PyPolyline3d {
    #[new]
    #[pyo3(signature = (
        vertices = Polyline3d::default().vertices.into_iter().map(PyVec3::from).collect()
    ))]
    pub fn new(vertices: Vec<PyVec3>) -> PyResult<PyClassInitializer<Self>> {
        let vertices: Vec<Vec3> = vertices
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;
        Ok((Self(Polyline3d::new(vertices)), PyMeshable).into())
    }

    #[staticmethod]
    pub fn with_subdivisions(
        py: Python<'_>,
        start: PyVec3,
        end: PyVec3,
        subdivisions: IntegerInput<u64>,
    ) -> PyResult<Py<Self>> {
        let subdivisions =
            check_for::<Self, _>("subdivisions", subdivisions, NON_NEGATIVE)?.stored();
        // Bevy reserves subdivisions + 2 vertices up front.
        vertex_budget("Polyline3d", u128::from(subdivisions) + 2)?;
        let subdivisions = subdivisions as usize;
        let start: Vec3 = start.try_into()?;
        let end: Vec3 = end.try_into()?;
        Py::new(
            py,
            (
                Self(Polyline3d::with_subdivisions(start, end, subdivisions)),
                PyMeshable,
            ),
        )
    }

    #[getter]
    pub fn vertices(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let list = PyList::empty(py);
        for vertex in &self.0.vertices {
            list.append(PyVec3::from_vec3(*vertex))?;
        }
        Ok(list.into())
    }

    #[setter]
    pub fn set_vertices(&mut self, vertices: Vec<PyVec3>) -> PyResult<()> {
        let bevy_vertices: Vec<Vec3> = vertices
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;
        self.0 = Polyline3d::new(bevy_vertices);
        Ok(())
    }

    pub fn mesh(&self, py: Python<'_>) -> PyResult<Py<PyPolyline3dMeshBuilder>> {
        if self.0.vertices.len() < 2 {
            return Err(PyValueError::new_err(polyline_mesh_too_short("Polyline3d")));
        }
        Py::new(
            py,
            (PyPolyline3dMeshBuilder::from(self.0.clone()), PyMeshBuilder),
        )
    }

    fn __repr__(&self) -> String {
        format!("Polyline3d(vertices=[{}])", self.0.vertices.len())
    }
}
