use bevy::{
    math::{
        Vec3,
        primitives::{Cuboid, Measured3d},
    },
    mesh::Meshable,
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::{pyconstructor, pyvalue};
use pybevy_math::{
    primitives::dimension::{check_half_size3, dimension},
    vec3::PyVec3,
};
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyCuboidMeshBuilder};

#[pyvalue]
#[pyclass(name = "Cuboid", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyCuboid {
    pub(crate) storage: ValueStorage<Cuboid>,
}

impl PartialEq for PyCuboid {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

#[pyconstructor(
    "Cuboid",
    keyword_only(half_size),
    conflicts((x_length, y_length, z_length), (half_size)),
)]
#[pymethods]
impl PyCuboid {
    #[new]
    pub fn new(
        #[default(1.0)] x_length: f32,
        #[default(1.0)] y_length: f32,
        #[default(1.0)] z_length: f32,
        #[expected("Vec3")] half_size: Option<PyVec3>,
    ) -> PyResult<PyClassInitializer<Self>> {
        if let Some(hs) = half_size {
            let half_size: Vec3 = dimension("half_size", hs.try_into()?)?;
            return Ok((Self::from_owned(Cuboid { half_size }), PyMeshable).into());
        }
        let cuboid = Cuboid::new(
            dimension("x_length", x_length)?,
            dimension("y_length", y_length)?,
            dimension("z_length", z_length)?,
        );
        Ok((Self::from_owned(cuboid), PyMeshable).into())
    }

    #[staticmethod]
    pub fn from_size(py: Python, size: PyVec3) -> PyResult<Py<Self>> {
        let size: Vec3 = dimension("size", size.try_into()?)?;
        Py::new(py, (Self::from_owned(Cuboid::from_size(size)), PyMeshable))
    }

    #[staticmethod]
    pub fn from_corners(py: Python, point1: PyVec3, point2: PyVec3) -> PyResult<Py<Self>> {
        let cuboid = Cuboid::from_corners(point1.try_into()?, point2.try_into()?);
        dimension("half_size", cuboid.half_size)?;
        Py::new(py, (Self::from_owned(cuboid), PyMeshable))
    }

    #[staticmethod]
    pub fn from_length(py: Python, length: f32) -> PyResult<Py<Self>> {
        let cuboid = Cuboid::from_length(dimension("length", length)?);
        Py::new(py, (Self::from_owned(cuboid), PyMeshable))
    }

    #[getter]
    pub fn half_size(&self) -> PyResult<PyVec3> {
        Ok(self.storage.borrow_validated_field_as(
            |s| &s.half_size,
            |s| &mut s.half_size,
            check_half_size3,
        )?)
    }

    #[setter]
    pub fn set_half_size(&mut self, value: PyVec3) -> PyResult<()> {
        let half_size: Vec3 = value.try_into()?;
        let mut cuboid = self.as_mut()?;
        cuboid.half_size = dimension("half_size", half_size)?;
        Ok(())
    }

    pub fn size(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.size().into())
    }

    pub fn closest_point(&self, point: PyVec3) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.closest_point(point.try_into()?).into())
    }

    pub fn area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.area())
    }

    pub fn volume(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.volume())
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyCuboidMeshBuilder>> {
        Py::new(py, (self.as_ref()?.mesh().into(), PyMeshBuilder))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("Cuboid(half_size={})", self.as_ref()?.half_size))
    }
}

impl From<Cuboid> for PyCuboid {
    fn from(value: Cuboid) -> Self {
        Self::from_owned(value)
    }
}
