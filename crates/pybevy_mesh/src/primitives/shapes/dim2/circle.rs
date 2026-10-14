use bevy::{
    math::Vec2,
    mesh::Meshable,
    shape::{Circle, Measured2d},
};
use pybevy_core::{FromBorrowedStorage, ValueStorage, repr::float_repr};
use pybevy_macros::pyvalue;
use pybevy_math::vec2::PyVec2;
use pybevy_shape::primitives::dimension::Dimension;
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyCircleMeshBuilder};

#[pyvalue(domains = [radius => [finite, non_negative]])]
#[pyclass(name = "Circle", module = "pybevy.shape", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyCircle {
    pub(crate) storage: ValueStorage<Circle>,
}

impl PyCircle {
    pub(crate) fn from_read_only(circle: &Circle) -> Self {
        Self {
            storage: ValueStorage::read_only_snapshot(*circle),
        }
    }

    pub(crate) fn try_get(&self) -> PyResult<Circle> {
        self.to_bevy()
    }
}

#[pymethods]
impl PyCircle {
    #[new]
    #[pyo3(signature = (radius=0.5))]
    pub fn new(radius: f32) -> PyResult<PyClassInitializer<Self>> {
        let circle = Circle::new(Self::check_radius(radius)?.without_negative_zero());
        Ok((Self::from_owned(circle), PyMeshable).into())
    }

    #[getter]
    pub fn radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.radius)
    }

    #[setter]
    pub fn set_radius(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.radius = Self::check_radius(value)?.without_negative_zero();
        Ok(())
    }

    pub fn diameter(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.diameter())
    }

    pub fn area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.area())
    }

    pub fn perimeter(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.perimeter())
    }

    pub fn closest_point(&self, point: PyVec2) -> PyResult<PyVec2> {
        let bevy_point: Vec2 = point.try_into()?;
        Ok(PyVec2::from_vec2(self.as_ref()?.closest_point(bevy_point)))
    }

    pub fn mesh(&self, py: Python<'_>) -> PyResult<Py<PyCircleMeshBuilder>> {
        Py::new(py, (self.as_ref()?.mesh().into(), PyMeshBuilder))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Circle(radius={})",
            float_repr(self.as_ref()?.radius)
        ))
    }
}

impl From<Circle> for PyCircle {
    fn from(circle: Circle) -> Self {
        Self::from_owned(circle)
    }
}
