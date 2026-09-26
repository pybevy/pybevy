use bevy::{
    math::{
        Vec2,
        primitives::{Circle, Measured2d},
    },
    mesh::Meshable,
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::{mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyCircleMeshBuilder};

#[pyclass(name = "Circle", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyCircle(pub(crate) ValueStorage<Circle>);

impl PyCircle {
    pub(crate) fn from_read_only(circle: &Circle) -> Self {
        Self(ValueStorage::read_only_snapshot(*circle))
    }

    pub(crate) fn try_get(&self) -> PyResult<Circle> {
        Ok(self.0.get()?)
    }
}

impl FromBorrowedStorage<ValueStorage<Circle>> for PyCircle {
    fn from_borrowed(storage: ValueStorage<Circle>) -> Self {
        Self(storage)
    }
}

#[pymethods]
impl PyCircle {
    #[new]
    #[pyo3(signature = (radius=0.5))]
    pub fn new(radius: f32) -> PyClassInitializer<Self> {
        (Self(ValueStorage::owned(Circle::new(radius))), PyMeshable).into()
    }

    #[getter]
    pub fn radius(&self) -> PyResult<f32> {
        Ok(self.0.as_ref()?.radius)
    }

    #[setter]
    pub fn set_radius(&mut self, value: f32) -> PyResult<()> {
        self.0.as_mut()?.radius = value;
        Ok(())
    }

    pub fn diameter(&self) -> PyResult<f32> {
        Ok(self.0.as_ref()?.diameter())
    }

    pub fn area(&self) -> PyResult<f32> {
        Ok(self.0.as_ref()?.area())
    }

    pub fn perimeter(&self) -> PyResult<f32> {
        Ok(self.0.as_ref()?.perimeter())
    }

    pub fn closest_point(&self, point: PyVec2) -> PyResult<PyVec2> {
        let bevy_point: Vec2 = point.try_into()?;
        Ok(PyVec2::from_vec2(
            self.0.as_ref()?.closest_point(bevy_point),
        ))
    }

    pub fn mesh(&self, py: Python<'_>) -> PyResult<Py<PyCircleMeshBuilder>> {
        Py::new(py, (self.0.as_ref()?.mesh().into(), PyMeshBuilder))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("Circle(radius={})", self.0.as_ref()?.radius))
    }
}

impl From<Circle> for PyCircle {
    fn from(circle: Circle) -> Self {
        PyCircle(ValueStorage::owned(circle))
    }
}
