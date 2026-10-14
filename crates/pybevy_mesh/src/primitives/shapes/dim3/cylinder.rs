use bevy::{
    mesh::Meshable,
    shape::{Cylinder, Measured3d},
};
use pybevy_core::repr::float_repr;
use pybevy_macros::pyconstructor;
use pybevy_shape::primitives::dimension::dimension;
use pyo3::prelude::*;

use crate::{
    mesh_builder::PyMeshBuilder,
    meshable::PyMeshable,
    primitives::{PyCylinderMeshBuilder, shapes::dim2::circle::PyCircle},
};

#[pyclass(name = "Cylinder", module = "pybevy.shape", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyCylinder(pub(crate) Cylinder);

#[pyconstructor("Cylinder", keyword_only(half_height), conflicts((height), (half_height)))]
#[pymethods]
impl PyCylinder {
    #[new]
    pub fn new(
        #[default(0.5)] radius: f32,
        #[default(1.0)] height: f32,
        half_height: Option<f32>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let radius = dimension::<Self, _>("radius", radius)?;
        if let Some(half_height) = half_height {
            let half_height = dimension::<Self, _>("half_height", half_height)?;
            return Ok((
                Self(Cylinder {
                    radius,
                    half_height,
                }),
                PyMeshable,
            )
                .into());
        }
        let cylinder = Cylinder::new(radius, dimension::<Self, _>("height", height)?);
        Ok((Self(cylinder), PyMeshable).into())
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
    pub fn half_height(&self) -> f32 {
        self.0.half_height
    }

    #[setter]
    pub fn set_half_height(&mut self, value: f32) -> PyResult<()> {
        self.0.half_height = dimension::<Self, _>("half_height", value)?;
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

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn volume(&self) -> f32 {
        self.0.volume()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyCylinderMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    pub fn __repr__(&self) -> String {
        format!(
            "Cylinder(radius={}, half_height={})",
            float_repr(self.0.radius),
            float_repr(self.0.half_height)
        )
    }
}

impl From<Cylinder> for PyCylinder {
    fn from(cylinder: Cylinder) -> Self {
        PyCylinder(cylinder)
    }
}
