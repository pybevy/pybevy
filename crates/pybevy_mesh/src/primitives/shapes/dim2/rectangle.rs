use bevy::{
    math::primitives::{Measured2d, Rectangle},
    mesh::Meshable,
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::pyvalue;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyRectangleMeshBuilder,
};

#[pyvalue]
#[pyclass(name = "Rectangle", extends = PyMeshable, eq, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyRectangle {
    pub(crate) storage: ValueStorage<Rectangle>,
}

impl PartialEq for PyRectangle {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

#[pymethods]
impl PyRectangle {
    #[new]
    #[pyo3(signature = (width=1.0, height=1.0, *, half_size=None))]
    pub fn new(
        width: f32,
        height: f32,
        half_size: Option<PyVec2>,
    ) -> PyResult<PyClassInitializer<Self>> {
        if let Some(hs) = half_size {
            return Ok((
                Self::from_owned(Rectangle {
                    half_size: hs.try_into()?,
                }),
                PyMeshable,
            )
                .into());
        }
        Ok((Self::from_owned(Rectangle::new(width, height)), PyMeshable).into())
    }

    #[staticmethod]
    pub fn from_size(py: Python, size: &PyVec2) -> PyResult<Py<Self>> {
        let rect = Rectangle::from_size(size.try_into()?);
        Py::new(py, (Self::from_owned(rect), PyMeshable))
    }

    #[staticmethod]
    pub fn from_corners(py: Python, point1: &PyVec2, point2: &PyVec2) -> PyResult<Py<Self>> {
        let rect = Rectangle::from_corners(point1.try_into()?, point2.try_into()?);
        Py::new(py, (Self::from_owned(rect), PyMeshable))
    }

    #[staticmethod]
    pub fn from_length(py: Python, length: f32) -> PyResult<Py<Self>> {
        let rect = Rectangle::from_length(length);
        Py::new(py, (Self::from_owned(rect), PyMeshable))
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

    pub fn size(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.size().into())
    }

    pub fn width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.half_size.x * 2.0)
    }

    pub fn height(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.half_size.y * 2.0)
    }

    pub fn closest_point(&self, point: &PyVec2) -> PyResult<PyVec2> {
        let cp = self.as_ref()?.closest_point(point.try_into()?);
        Ok(cp.into())
    }

    pub fn area(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.area())
    }

    pub fn perimeter(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.perimeter())
    }

    pub fn mesh(&self, py: Python) -> PyResult<Py<PyRectangleMeshBuilder>> {
        Py::new(py, (self.as_ref()?.mesh().into(), PyMeshBuilder))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("Rectangle(size={})", self.as_ref()?.size()))
    }
}

impl From<Rectangle> for PyRectangle {
    fn from(value: Rectangle) -> Self {
        Self::from_owned(value)
    }
}
