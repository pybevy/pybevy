use bevy::{
    math::{
        Vec2,
        primitives::{Annulus, Circle, Measured2d},
    },
    mesh::Meshable,
};
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use super::circle::PyCircle;
use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyAnnulusMeshBuilder};

#[pyclass(name = "Annulus", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyAnnulus(pub(crate) Annulus);

impl From<PyAnnulus> for Annulus {
    fn from(py_annulus: PyAnnulus) -> Self {
        py_annulus.0
    }
}

impl From<Annulus> for PyAnnulus {
    fn from(annulus: Annulus) -> Self {
        PyAnnulus(annulus)
    }
}

#[pymethods]
impl PyAnnulus {
    #[new]
    #[pyo3(signature = (inner_radius = 0.5, outer_radius = 1.0, *, inner_circle = None, outer_circle = None))]
    pub fn new(
        inner_radius: f32,
        outer_radius: f32,
        inner_circle: Option<&PyCircle>,
        outer_circle: Option<&PyCircle>,
    ) -> PyClassInitializer<Self> {
        if let (Some(inner), Some(outer)) = (inner_circle, outer_circle) {
            return (
                Self(Annulus {
                    inner_circle: Circle::new(inner.radius()),
                    outer_circle: Circle::new(outer.radius()),
                }),
                PyMeshable,
            )
                .into();
        }
        (Self(Annulus::new(inner_radius, outer_radius)), PyMeshable).into()
    }

    #[getter]
    pub fn inner_circle(&self, py: Python<'_>) -> PyResult<Py<PyCircle>> {
        Py::new(py, (self.0.inner_circle.into(), PyMeshable))
    }

    #[setter]
    pub fn set_inner_circle(&mut self, value: &PyCircle) {
        self.0.inner_circle = Circle::new(value.radius());
    }

    #[getter]
    pub fn outer_circle(&self, py: Python<'_>) -> PyResult<Py<PyCircle>> {
        Py::new(py, (self.0.outer_circle.into(), PyMeshable))
    }

    #[setter]
    pub fn set_outer_circle(&mut self, value: &PyCircle) {
        self.0.outer_circle = Circle::new(value.radius());
    }

    pub fn diameter(&self) -> f32 {
        self.0.diameter()
    }

    pub fn thickness(&self) -> f32 {
        self.0.thickness()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn perimeter(&self) -> f32 {
        self.0.perimeter()
    }

    pub fn closest_point(&self, point: PyVec2) -> PyVec2 {
        let bevy_point: Vec2 = point.into();
        PyVec2::from_vec2(self.0.closest_point(bevy_point))
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyAnnulusMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "Annulus(inner_radius={}, outer_radius={})",
            self.0.inner_circle.radius, self.0.outer_circle.radius
        )
    }
}
