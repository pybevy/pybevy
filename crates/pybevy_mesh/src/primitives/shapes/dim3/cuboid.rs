use bevy::{
    math::{
        Vec3,
        primitives::{Cuboid, Measured3d},
    },
    mesh::Meshable,
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pybevy_math::vec3::PyVec3;
use pyo3::{exceptions::PyValueError, prelude::*};

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

fn validate_dimension(value: f32, parameter: &str) -> PyResult<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(PyValueError::new_err(format!(
            "{parameter} must be finite and greater than zero"
        )));
    }
    Ok(())
}

fn validate_dimensions(value: Vec3, parameter: &str) -> PyResult<Vec3> {
    if !value.is_finite() || value.cmple(Vec3::ZERO).any() {
        return Err(PyValueError::new_err(format!(
            "{parameter} components must be finite and greater than zero"
        )));
    }
    Ok(value)
}

#[pymethods]
impl PyCuboid {
    #[new]
    #[pyo3(signature = (x_length=1.0, y_length=1.0, z_length=1.0, *, half_size=None))]
    pub fn new(
        x_length: f32,
        y_length: f32,
        z_length: f32,
        half_size: Option<PyVec3>,
    ) -> PyResult<PyClassInitializer<Self>> {
        if let Some(hs) = half_size {
            let half_size = validate_dimensions(hs.try_into()?, "half_size")?;
            return Ok((Self::from_owned(Cuboid { half_size }), PyMeshable).into());
        }
        validate_dimension(x_length, "x_length")?;
        validate_dimension(y_length, "y_length")?;
        validate_dimension(z_length, "z_length")?;
        Ok((
            Self::from_owned(Cuboid::new(x_length, y_length, z_length)),
            PyMeshable,
        )
            .into())
    }

    #[staticmethod]
    pub fn from_size(py: Python, size: PyVec3) -> PyResult<Py<Self>> {
        let size = validate_dimensions(size.try_into()?, "size")?;
        Py::new(py, (Self::from_owned(Cuboid::from_size(size)), PyMeshable))
    }

    #[staticmethod]
    pub fn from_corners(py: Python, point1: PyVec3, point2: PyVec3) -> PyResult<Py<Self>> {
        let cuboid = Cuboid::from_corners(point1.try_into()?, point2.try_into()?);
        validate_dimensions(cuboid.half_size, "corner-derived half_size")?;
        Py::new(py, (Self::from_owned(cuboid), PyMeshable))
    }

    #[staticmethod]
    pub fn from_length(py: Python, length: f32) -> PyResult<Py<Self>> {
        validate_dimension(length, "length")?;
        Py::new(
            py,
            (Self::from_owned(Cuboid::from_length(length)), PyMeshable),
        )
    }

    #[getter]
    pub fn half_size(&self) -> PyResult<PyVec3> {
        Ok(self.storage.borrow_field_as(|s| &s.half_size)?)
    }

    #[setter]
    pub fn set_half_size(&mut self, value: PyVec3) -> PyResult<()> {
        self.as_mut()?.half_size = validate_dimensions(value.try_into()?, "half_size")?;
        Ok(())
    }

    pub fn size(&self) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.size().into())
    }

    pub fn closest_point(&self, point: PyVec3) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.closest_point(point.try_into()?).try_into()?)
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
