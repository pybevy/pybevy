use bevy::{
    mesh::Meshable,
    shape::{Cone, Measured3d},
};
use pybevy_core::repr::float_repr;
use pybevy_shape::primitives::dimension::dimension;
use pyo3::prelude::*;

use crate::{
    mesh_builder::PyMeshBuilder,
    meshable::PyMeshable,
    primitives::{PyConeMeshBuilder, shapes::dim2::circle::PyCircle},
};

#[pyclass(name = "Cone", module = "pybevy.shape", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyCone(pub(crate) Cone);

#[pymethods]
impl PyCone {
    #[new]
    #[pyo3(signature = (radius=0.5, height=1.0))]
    pub fn new(radius: f32, height: f32) -> PyResult<PyClassInitializer<Self>> {
        let cone = Cone::new(
            dimension::<Self, _>("radius", radius)?,
            dimension::<Self, _>("height", height)?,
        );
        Ok((Self(cone), PyMeshable).into())
    }

    #[getter]
    pub fn radius(&self) -> f32 {
        self.0.radius
    }

    #[setter]
    pub fn set_radius(&mut self, value: f32) -> PyResult<()> {
        self.0.radius = dimension::<Self, _>("radius", value)?;
        Ok(())
    }

    #[getter]
    pub fn height(&self) -> f32 {
        self.0.height
    }

    #[setter]
    pub fn set_height(&mut self, value: f32) -> PyResult<()> {
        self.0.height = dimension::<Self, _>("height", value)?;
        Ok(())
    }

    pub fn base(&self, py: Python) -> PyResult<Py<PyCircle>> {
        Py::new(py, (PyCircle::from(self.0.base()), PyMeshable))
    }

    pub fn base_area(&self) -> f32 {
        self.0.base_area()
    }

    pub fn lateral_area(&self) -> f32 {
        self.0.lateral_area()
    }

    pub fn slant_height(&self) -> f32 {
        self.0.slant_height()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn volume(&self) -> f32 {
        self.0.volume()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyConeMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    pub fn __repr__(&self) -> String {
        format!(
            "Cone(radius={}, height={})",
            float_repr(self.0.radius),
            float_repr(self.0.height)
        )
    }
}

impl From<Cone> for PyCone {
    fn from(cone: Cone) -> Self {
        PyCone(cone)
    }
}
