use bevy::{
    math::{InvalidDirectionError, Isometry2d, Ray2d, Rot2, primitives::Segment2d},
    mesh::Meshable,
};
use pybevy_math::{bounding::PyIsometry2d, dir2::PyDir2, ray::PyRay2d, rot2::PyRot2, vec2::PyVec2};
use pyo3::prelude::*;

use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PySegment2dMeshBuilder,
};

#[pyclass(name = "Segment2d", extends = PyMeshable, eq)]
#[derive(Clone, PartialEq)]
pub struct PySegment2d(pub(crate) Segment2d);

impl From<Segment2d> for PySegment2d {
    fn from(segment: Segment2d) -> Self {
        Self(segment)
    }
}

impl From<PySegment2d> for Segment2d {
    fn from(segment: PySegment2d) -> Self {
        segment.0
    }
}

#[pymethods]
impl PySegment2d {
    #[new]
    #[pyo3(signature = (point1 = PyVec2::ZERO, point2 = PyVec2::ZERO, *, vertices = None))]
    pub fn new(
        point1: PyVec2,
        point2: PyVec2,
        vertices: Option<[PyVec2; 2]>,
    ) -> (Self, PyMeshable) {
        if let Some(v) = vertices {
            return (
                Self(Segment2d {
                    vertices: [v[0].get(), v[1].get()],
                }),
                PyMeshable,
            );
        }
        (Self(Segment2d::new(point1.get(), point2.get())), PyMeshable)
    }

    #[staticmethod]
    pub fn from_direction_and_length(
        py: Python<'_>,
        direction: PyDir2,
        length: f32,
    ) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self(Segment2d::from_direction_and_length(
                    direction.into_dir2(),
                    length,
                )),
                PyMeshable,
            ),
        )
    }

    #[staticmethod]
    pub fn from_scaled_direction(py: Python<'_>, scaled_direction: PyVec2) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self(Segment2d::from_scaled_direction(scaled_direction.get())),
                PyMeshable,
            ),
        )
    }

    #[staticmethod]
    pub fn from_ray_and_length(py: Python<'_>, ray: PyRay2d, length: f32) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (
                Self(Segment2d::from_ray_and_length(Ray2d::from(ray), length)),
                PyMeshable,
            ),
        )
    }

    #[getter]
    pub fn vertices(&self) -> [PyVec2; 2] {
        [
            PyVec2::from_vec2(self.0.vertices[0]),
            PyVec2::from_vec2(self.0.vertices[1]),
        ]
    }

    #[setter]
    pub fn set_vertices(&mut self, vertices: [PyVec2; 2]) {
        self.0.vertices = [vertices[0].get(), vertices[1].get()];
    }

    pub fn point1(&self) -> PyVec2 {
        PyVec2::from_vec2(self.0.point1())
    }

    pub fn point2(&self) -> PyVec2 {
        PyVec2::from_vec2(self.0.point2())
    }

    pub fn center(&self) -> PyVec2 {
        PyVec2::from_vec2(self.0.center())
    }

    pub fn length(&self) -> f32 {
        self.0.length()
    }

    pub fn length_squared(&self) -> f32 {
        self.0.length_squared()
    }

    pub fn direction(&self) -> PyResult<PyDir2> {
        Ok(PyDir2::from_dir2(self.0.direction()))
    }

    pub fn try_direction(&self) -> PyResult<PyDir2> {
        self.0
            .try_direction()
            .map(PyDir2::from_dir2)
            .map_err(|e: InvalidDirectionError| {
                pyo3::exceptions::PyValueError::new_err(format!("{}", e))
            })
    }

    pub fn scaled_direction(&self) -> PyVec2 {
        PyVec2::from_vec2(self.0.scaled_direction())
    }

    pub fn left_normal(&self) -> PyResult<PyDir2> {
        Ok(PyDir2::from_dir2(self.0.left_normal()))
    }

    pub fn try_left_normal(&self) -> PyResult<PyDir2> {
        self.0
            .try_left_normal()
            .map(PyDir2::from_dir2)
            .map_err(|e: InvalidDirectionError| {
                pyo3::exceptions::PyValueError::new_err(format!("{}", e))
            })
    }

    pub fn right_normal(&self) -> PyResult<PyDir2> {
        Ok(PyDir2::from_dir2(self.0.right_normal()))
    }

    pub fn try_right_normal(&self) -> PyResult<PyDir2> {
        self.0
            .try_right_normal()
            .map(PyDir2::from_dir2)
            .map_err(|e: InvalidDirectionError| {
                pyo3::exceptions::PyValueError::new_err(format!("{}", e))
            })
    }

    pub fn scaled_left_normal(&self) -> PyVec2 {
        PyVec2::from_vec2(self.0.scaled_left_normal())
    }

    pub fn scaled_right_normal(&self) -> PyVec2 {
        PyVec2::from_vec2(self.0.scaled_right_normal())
    }

    pub fn transformed(&self, py: Python<'_>, isometry: &PyIsometry2d) -> PyResult<Py<Self>> {
        let iso: Isometry2d = isometry.clone().into();
        Py::new(py, (Self(self.0.transformed(iso)), PyMeshable))
    }

    pub fn translated(&self, py: Python<'_>, translation: PyVec2) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.translated(translation.get())), PyMeshable))
    }

    pub fn rotated(&self, py: Python<'_>, rotation: PyRot2) -> PyResult<Py<Self>> {
        let rot: Rot2 = rotation.into();
        Py::new(py, (Self(self.0.rotated(rot)), PyMeshable))
    }

    pub fn rotated_around(
        &self,
        py: Python<'_>,
        rotation: PyRot2,
        point: PyVec2,
    ) -> PyResult<Py<Self>> {
        let rot: Rot2 = rotation.into();
        Py::new(
            py,
            (Self(self.0.rotated_around(rot, point.get())), PyMeshable),
        )
    }

    pub fn rotated_around_center(&self, py: Python<'_>, rotation: PyRot2) -> PyResult<Py<Self>> {
        let rot: Rot2 = rotation.into();
        Py::new(py, (Self(self.0.rotated_around_center(rot)), PyMeshable))
    }

    pub fn centered(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.centered()), PyMeshable))
    }

    pub fn resized(&self, py: Python<'_>, length: f32) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.resized(length)), PyMeshable))
    }

    pub fn reverse(&mut self) {
        self.0.reverse();
    }

    pub fn reversed(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, (Self(self.0.reversed()), PyMeshable))
    }

    pub fn closest_point(&self, point: PyVec2) -> PyVec2 {
        PyVec2::from_vec2(self.0.closest_point(point.get()))
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PySegment2dMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "Segment2d(point1={}, point2={})",
            self.0.point1(),
            self.0.point2()
        )
    }
}
