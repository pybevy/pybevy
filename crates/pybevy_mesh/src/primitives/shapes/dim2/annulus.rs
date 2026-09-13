use bevy::{
    math::{
        Vec2,
        primitives::{Annulus, Circle, Measured2d},
    },
    mesh::Meshable,
};
use pybevy_macros::pyconstructor;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use super::circle::PyCircle;
use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyAnnulusMeshBuilder};

#[pyclass(name = "Annulus", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
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

#[pyconstructor(
    "Annulus",
    keyword_only(inner_circle, outer_circle),
    conflicts((inner_radius), (inner_circle)),
    conflicts((outer_radius), (outer_circle)),
)]
#[pymethods]
impl PyAnnulus {
    #[new]
    pub fn new(
        #[default(0.5)] inner_radius: f32,
        #[default(1.0)] outer_radius: f32,
        #[expected("Circle")] inner_circle: Option<PyRef<'_, PyCircle>>,
        #[expected("Circle")] outer_circle: Option<PyRef<'_, PyCircle>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok((
            Self(Annulus {
                inner_circle: inner_circle
                    .map(|circle| circle.0)
                    .unwrap_or_else(|| Circle::new(inner_radius)),
                outer_circle: outer_circle
                    .map(|circle| circle.0)
                    .unwrap_or_else(|| Circle::new(outer_radius)),
            }),
            PyMeshable,
        )
            .into())
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

    pub fn closest_point(&self, point: PyVec2) -> PyResult<PyVec2> {
        let bevy_point: Vec2 = point.try_into()?;
        Ok(PyVec2::from_vec2(self.0.closest_point(bevy_point)))
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
