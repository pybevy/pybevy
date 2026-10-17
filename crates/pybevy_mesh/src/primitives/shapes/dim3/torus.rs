use bevy::{
    mesh::Meshable,
    shape::{Measured3d, Torus},
};
use pybevy_core::repr::float_repr;
use pybevy_macros::pyconstructor;
use pybevy_math::{primitives::dimension::dimension, torus_kind::PyTorusKind};
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyTorusMeshBuilder};

#[pyclass(name = "Torus", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyTorus(pub(crate) Torus);

impl From<PyTorus> for Torus {
    fn from(py_torus: PyTorus) -> Self {
        py_torus.0
    }
}

impl From<Torus> for PyTorus {
    fn from(torus: Torus) -> Self {
        PyTorus(torus)
    }
}

#[pyconstructor(
    "Torus",
    keyword_only(minor_radius, major_radius),
    conflicts((inner_radius, outer_radius), (minor_radius, major_radius)),
    complete(minor_radius, major_radius),
)]
#[pymethods]
impl PyTorus {
    #[new]
    pub fn new(
        #[default(0.5)] inner_radius: f32,
        #[default(1.0)] outer_radius: f32,
        minor_radius: Option<f32>,
        major_radius: Option<f32>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let torus = match (minor_radius, major_radius) {
            (Some(minor_radius), Some(major_radius)) => Torus {
                minor_radius: dimension::<Self, _>("minor_radius", minor_radius)?,
                major_radius: dimension::<Self, _>("major_radius", major_radius)?,
            },
            _ => {
                let torus = Torus::new(
                    dimension::<Self, _>("inner_radius", inner_radius)?,
                    dimension::<Self, _>("outer_radius", outer_radius)?,
                );
                dimension::<Self, _>("minor_radius", torus.minor_radius)?;
                torus
            }
        };
        Ok((Self(torus), PyMeshable).into())
    }

    #[getter]
    pub fn minor_radius(&self) -> f32 {
        self.0.minor_radius
    }

    #[setter]
    pub fn set_minor_radius(&mut self, value: f32) -> PyResult<()> {
        self.0.minor_radius = dimension::<Self, _>("minor_radius", value)?;
        Ok(())
    }

    #[getter]
    pub fn major_radius(&self) -> f32 {
        self.0.major_radius
    }

    #[setter]
    pub fn set_major_radius(&mut self, value: f32) -> PyResult<()> {
        self.0.major_radius = dimension::<Self, _>("major_radius", value)?;
        Ok(())
    }

    pub fn inner_radius(&self) -> f32 {
        self.0.inner_radius()
    }

    pub fn outer_radius(&self) -> f32 {
        self.0.outer_radius()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn volume(&self) -> f32 {
        self.0.volume()
    }

    pub fn kind(&self) -> PyTorusKind {
        self.0.kind().into()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyTorusMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> String {
        format!(
            "Torus(minor_radius={}, major_radius={})",
            float_repr(self.0.minor_radius),
            float_repr(self.0.major_radius)
        )
    }
}
