use bevy::{
    math::{
        Vec2,
        primitives::{Ellipse, Measured2d},
    },
    mesh::Meshable,
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyEllipseMeshBuilder};

#[pyvalue]
#[pyclass(name = "Ellipse", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyEllipse {
    pub(crate) storage: ValueStorage<Ellipse>,
}

impl PartialEq for PyEllipse {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl TryFrom<PyEllipse> for Ellipse {
    type Error = PyErr;

    fn try_from(value: PyEllipse) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyEllipse> for Ellipse {
    type Error = PyErr;

    fn try_from(value: &PyEllipse) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl From<Ellipse> for PyEllipse {
    fn from(value: Ellipse) -> Self {
        Self::from_owned(value)
    }
}

#[pymethods]
impl PyEllipse {
    #[new]
    #[pyo3(signature = (half_size = PyVec2::vec2(Vec2::new(1.0, 0.5))))]
    pub fn new(half_size: PyVec2) -> PyResult<PyClassInitializer<Self>> {
        let half_size: Vec2 = half_size.try_into()?;
        Ok((
            Self::from_owned(Ellipse::new(half_size.x, half_size.y)),
            PyMeshable,
        )
            .into())
    }

    #[getter]
    pub fn half_size(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.half_size)?)
    }

    #[setter]
    pub fn set_half_size(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.half_size = value.try_into()?;
        Ok(())
    }

    pub fn eccentricity(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.eccentricity())
    }

    pub fn focal_length(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.focal_length())
    }

    pub fn semi_major(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.semi_major())
    }

    pub fn semi_minor(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.semi_minor())
    }

    #[staticmethod]
    pub fn from_size(py: Python, size: PyVec2) -> PyResult<Py<Self>> {
        let bevy_size: Vec2 = size.try_into()?;
        Py::new(
            py,
            (Self::from_owned(Ellipse::from_size(bevy_size)), PyMeshable),
        )
    }

    pub fn area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.area())
    }

    pub fn perimeter(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.perimeter())
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyEllipseMeshBuilder>> {
        Py::new(py, (self.as_ref()?.mesh().into(), PyMeshBuilder))
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Ellipse(half_size=Vec2({}, {}))",
            self.as_ref()?.half_size.x,
            self.as_ref()?.half_size.y
        ))
    }
}
