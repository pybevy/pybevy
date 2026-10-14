use bevy::{
    math::{
        Vec2,
        primitives::{Measured2d, Rectangle},
    },
    mesh::Meshable,
};
use pybevy_core::{FromBorrowedStorage, StorageError, ValueStorage, repr::float_list_repr};
use pybevy_macros::{pyconstructor, pyvalue};
use pybevy_math::{primitives::dimension::dimension, vec2::PyVec2};
use pyo3::prelude::*;

use crate::{
    mesh_builder::PyMeshBuilder, meshable::PyMeshable, primitives::PyRectangleMeshBuilder,
};

fn check_half_size(value: &Vec2) -> Result<(), StorageError> {
    dimension("Rectangle", "half_size", *value).map(drop)
}

#[pyvalue]
#[pyclass(name = "Rectangle", module = "pybevy.math", extends = PyMeshable, eq, skip_from_py_object)]
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

#[pyconstructor("Rectangle", keyword_only(half_size), conflicts((width, height), (half_size)))]
#[pymethods]
impl PyRectangle {
    #[new]
    pub fn new(
        #[default(1.0)] width: f32,
        #[default(1.0)] height: f32,
        #[expected("Vec2")] half_size: Option<PyVec2>,
    ) -> PyResult<PyClassInitializer<Self>> {
        if let Some(hs) = half_size {
            let half_size = dimension("Rectangle", "half_size", hs.try_into()?)?;
            return Ok((Self::from_owned(Rectangle { half_size }), PyMeshable).into());
        }
        let rect = Rectangle::new(
            dimension("Rectangle", "width", width)?,
            dimension("Rectangle", "height", height)?,
        );
        Ok((Self::from_owned(rect), PyMeshable).into())
    }

    #[staticmethod]
    pub fn from_size(py: Python, size: &PyVec2) -> PyResult<Py<Self>> {
        let rect = Rectangle::from_size(dimension("Rectangle", "size", size.try_into()?)?);
        Py::new(py, (Self::from_owned(rect), PyMeshable))
    }

    #[staticmethod]
    pub fn from_corners(py: Python, point1: &PyVec2, point2: &PyVec2) -> PyResult<Py<Self>> {
        let rect = Rectangle::from_corners(point1.try_into()?, point2.try_into()?);
        dimension("Rectangle", "half_size", rect.half_size)?;
        Py::new(py, (Self::from_owned(rect), PyMeshable))
    }

    #[staticmethod]
    pub fn from_length(py: Python, length: f32) -> PyResult<Py<Self>> {
        let rect = Rectangle::from_length(dimension("Rectangle", "length", length)?);
        Py::new(py, (Self::from_owned(rect), PyMeshable))
    }

    #[getter]
    pub fn half_size(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |s| &s.half_size,
            |s| &mut s.half_size,
            check_half_size,
        )?)
    }

    #[setter]
    pub fn set_half_size(&mut self, value: PyVec2) -> PyResult<()> {
        let half_size = value.try_into()?;
        let mut shape = self.as_mut()?;
        shape.half_size = dimension("Rectangle", "half_size", half_size)?;
        Ok(())
    }

    pub fn size(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.size().into())
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
        Ok(format!(
            "Rectangle(size={})",
            float_list_repr(&self.as_ref()?.size().to_array())
        ))
    }
}

impl From<Rectangle> for PyRectangle {
    fn from(value: Rectangle) -> Self {
        Self::from_owned(value)
    }
}
