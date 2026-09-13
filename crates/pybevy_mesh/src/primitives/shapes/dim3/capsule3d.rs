use bevy::{
    math::primitives::{Capsule3d, Measured3d},
    mesh::Meshable,
};
use pybevy_macros::pyconstructor;
use pyo3::prelude::*;

use super::cylinder::PyCylinder;
use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyCapsule3dMeshBuilder,
};

#[pyclass(name = "Capsule3d", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyCapsule3d(pub(crate) Capsule3d);

impl From<PyCapsule3d> for Capsule3d {
    fn from(py_capsule: PyCapsule3d) -> Self {
        py_capsule.0
    }
}

impl From<Capsule3d> for PyCapsule3d {
    fn from(capsule: Capsule3d) -> Self {
        PyCapsule3d(capsule)
    }
}

#[pyconstructor("Capsule3d", keyword_only(half_length), conflicts((length), (half_length)))]
#[pymethods]
impl PyCapsule3d {
    #[new]
    pub fn new(
        #[default(0.5)] radius: f32,
        #[default(1.0)] length: f32,
        half_length: Option<f32>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let capsule = match half_length {
            Some(half_length) => Capsule3d {
                radius,
                half_length,
            },
            None => Capsule3d::new(radius, length),
        };
        Ok((Self(capsule), PyMeshable).into())
    }

    #[getter]
    pub fn radius(&self) -> f32 {
        self.0.radius
    }

    #[setter]
    pub fn set_radius(&mut self, value: f32) {
        self.0.radius = value;
    }

    #[getter]
    pub fn half_length(&self) -> f32 {
        self.0.half_length
    }

    #[setter]
    pub fn set_half_length(&mut self, value: f32) {
        self.0.half_length = value;
    }

    pub fn to_cylinder(&self, py: Python<'_>) -> PyResult<Py<PyCylinder>> {
        Py::new(py, (self.0.to_cylinder().into(), PyMeshable))
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn volume(&self) -> f32 {
        self.0.volume()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyCapsule3dMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "Capsule3d(radius={}, length={})",
            self.0.radius,
            self.0.half_length * 2.0
        )
    }
}
