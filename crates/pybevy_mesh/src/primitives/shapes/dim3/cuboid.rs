use bevy::{
    math::primitives::{Cuboid, Measured3d},
    mesh::Meshable,
};
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyCuboidMeshBuilder};

#[pyclass(name = "Cuboid", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyCuboid(pub(crate) Cuboid);

#[pymethods]
impl PyCuboid {
    #[new]
    #[pyo3(signature = (x_length=1.0, y_length=1.0, z_length=1.0, *, half_size=None))]
    pub fn new(
        x_length: f32,
        y_length: f32,
        z_length: f32,
        half_size: Option<PyVec3>,
    ) -> PyClassInitializer<Self> {
        if let Some(hs) = half_size {
            return (
                Self(Cuboid {
                    half_size: hs.into(),
                }),
                PyMeshable,
            )
                .into();
        }
        (Self(Cuboid::new(x_length, y_length, z_length)), PyMeshable).into()
    }

    #[staticmethod]
    pub fn from_size(py: Python, size: PyVec3) -> PyResult<Py<Self>> {
        Py::new(py, (Self(Cuboid::from_size(size.into())), PyMeshable))
    }

    #[staticmethod]
    pub fn from_corners(py: Python, point1: PyVec3, point2: PyVec3) -> PyResult<Py<Self>> {
        let cuboid = Cuboid::from_corners(point1.into(), point2.into());
        Py::new(py, (Self(cuboid), PyMeshable))
    }

    #[staticmethod]
    pub fn from_length(py: Python, length: f32) -> PyResult<Py<Self>> {
        Py::new(py, (Self(Cuboid::from_length(length)), PyMeshable))
    }

    #[getter]
    pub fn half_size(&self) -> PyVec3 {
        self.0.half_size.into()
    }

    #[setter]
    pub fn set_half_size(&mut self, value: PyVec3) {
        self.0.half_size = value.into();
    }

    pub fn size(&self) -> PyVec3 {
        self.0.size().into()
    }

    pub fn closest_point(&self, point: PyVec3) -> PyVec3 {
        self.0.closest_point(point.into()).into()
    }

    pub fn area(&self) -> f32 {
        self.0.area()
    }

    pub fn volume(&self) -> f32 {
        self.0.volume()
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyCuboidMeshBuilder>> {
        Py::new(py, (self.0.mesh().into(), PyMeshBuilder))
    }

    pub fn __repr__(&self) -> String {
        format!("Cuboid(half_size={})", self.0.half_size)
    }
}

impl From<Cuboid> for PyCuboid {
    fn from(cuboid: Cuboid) -> Self {
        PyCuboid(cuboid)
    }
}
